//! Agent executor - runs agents using serdesAI's Agent API.
//!
//! This module provides the execution layer for SpotAgents, using
//! serdesAI's agent loop with proper tool calling and streaming support.
//!
//! ## Submodules
//! - `adapters`: Model and tool adapters for serdesAI integration
//! - `sub_agents`: Executors for invoke_agent and list_agents tools
//! - `mcp`: MCP tool executor
//! - `types`: Result types and errors
//! - `model_factory`: Model resolution and creation

mod adapters;
mod mcp;
mod model_factory;
mod sub_agents;
mod types;

// Re-export public API
pub use model_factory::get_model;
pub use types::{ExecutorError, ExecutorResult, ExecutorStreamReceiver};

use crate::agents::{AgentManager, SpotAgent};
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{EventBridge, MessageSender};
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;

use adapters::{ArcModel, RecordingToolExecutor, ToolExecutorAdapter};
use mcp::McpToolExecutor;
use sub_agents::{InvokeAgentExecutor, ListAgentsExecutor};

use serdes_ai_agent::{agent, RunOptions};
use serdes_ai_core::messages::ToolCallArgs;
use serdes_ai_core::messages::{ImageMediaType, UserContent, UserContentPart};
use serdes_ai_core::{
    ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart, TextPart, ToolCallPart,
    ToolReturnPart,
};
use serdes_ai_tools::{Tool, ToolDefinition};

use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

// Re-export stream event
pub use serdes_ai_agent::AgentStreamEvent as StreamEvent;

/// Agent executor that bridges SpotAgents with serdesAI.
///
/// This replaces raw model calls with proper agent execution including:
/// - Tool calling and execution loop
/// - Streaming support
/// - Message history management
/// - Retry logic
pub struct AgentExecutor<'a> {
    db: &'a Database,
    registry: &'a ModelRegistry,
    /// Optional message bus for event publishing.
    bus: Option<MessageSender>,
}

impl<'a> AgentExecutor<'a> {
    /// Create a new executor with database access (for OAuth tokens) and model registry.
    pub fn new(db: &'a Database, registry: &'a ModelRegistry) -> Self {
        Self {
            db,
            registry,
            bus: None,
        }
    }

    /// Add message bus for event publishing.
    ///
    /// When a bus is configured, sub-agent invocations will publish their
    /// events to the same bus, making nested agent output visible.
    pub fn with_bus(mut self, sender: MessageSender) -> Self {
        self.bus = Some(sender);
        self
    }

    /// Filter tool names based on settings.
    ///
    /// Filters out:
    /// - `share_your_reasoning` unless `show_reasoning` is enabled
    /// - `invoke_agent` and `list_agents` (these use custom executors)
    fn filter_tools<'b>(&self, tool_names: Vec<&'b str>) -> Vec<&'b str> {
        let settings = Settings::new(self.db);
        let show_reasoning = settings.get_bool("show_reasoning").unwrap_or(false);

        tool_names
            .into_iter()
            .filter(|name| {
                match *name {
                    "share_your_reasoning" => show_reasoning,
                    // These are handled by custom executors, not the registry
                    "invoke_agent" | "list_agents" => false,
                    _ => true,
                }
            })
            .collect()
    }

    /// Check if agent wants invoke_agent tool.
    fn wants_invoke_agent(&self, tool_names: &[&str]) -> bool {
        tool_names.contains(&"invoke_agent")
    }

    /// Check if agent wants list_agents tool.
    fn wants_list_agents(&self, tool_names: &[&str]) -> bool {
        tool_names.contains(&"list_agents")
    }

    /// Execute an agent with a prompt (blocking mode).
    ///
    /// This runs the full agent loop including tool calls until completion.
    /// Returns the final output and message history for context.
    pub async fn execute(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: &str,
        message_history: Option<Vec<ModelRequest>>,
        tool_registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
    ) -> Result<ExecutorResult, ExecutorError> {
        // Get the model (handles OAuth models and custom endpoints)
        let model = get_model(self.db, model_name, self.registry).await?;
        let wrapped_model = ArcModel(model);

        // Get original tool list (before filtering) to check for special tools
        let original_tools = spot_agent.available_tools();
        let wants_invoke = self.wants_invoke_agent(&original_tools);
        let wants_list = self.wants_list_agents(&original_tools);

        // Get the tools this agent should have access to (filtered by settings)
        let tool_names = self.filter_tools(original_tools);
        let tools = tool_registry.tools_by_name(&tool_names);

        // Build the serdesAI agent
        let mut builder = agent(wrapped_model)
            .system_prompt(spot_agent.system_prompt())
            .temperature(0.7)
            .max_tokens(16384);

        // Register built-in tools with real executors
        for tool in tools {
            let def = tool.definition();
            builder = builder.tool_with_executor(def, ToolExecutorAdapter::new(Arc::clone(&tool)));
        }

        // Add invoke_agent with custom executor (has database access)
        if wants_invoke {
            let invoke_executor = if let Some(ref bus) = self.bus {
                InvokeAgentExecutor::new(self.db, model_name, bus.clone())
            } else {
                InvokeAgentExecutor::new_legacy(self.db, model_name)
            };
            builder =
                builder.tool_with_executor(InvokeAgentExecutor::definition(), invoke_executor);
        }

        // Add list_agents with custom executor
        if wants_list {
            builder = builder.tool_with_executor(
                ListAgentsExecutor::definition(),
                ListAgentsExecutor::new(self.db),
            );
        }

        // Add MCP tools (filtered by agent attachments)
        let mcp_tools = self
            .collect_mcp_tools(mcp_manager, Some(spot_agent.name()))
            .await;
        for (def, tool) in mcp_tools {
            builder = builder.tool_with_executor(def, ToolExecutorAdapter::new(tool));
        }

        let serdes_agent = builder.build();

        // Set up run options with message history if provided
        let options = match message_history {
            Some(history) => RunOptions::new().message_history(history),
            None => RunOptions::new(),
        };

        // Run the agent
        let result = serdes_agent
            .run_with_options(prompt, (), options)
            .await
            .map_err(|e| ExecutorError::Execution(e.to_string()))?;

        Ok(ExecutorResult {
            output: result.output.clone(),
            messages: result.messages,
            run_id: result.run_id,
        })
    }

    /// Execute agent with events published to message bus.
    ///
    /// This is the preferred method when you have a UI that subscribes
    /// to the message bus. All streaming events are converted to Messages
    /// and published, allowing sub-agents to also be visible.
    ///
    /// # Errors
    ///
    /// Returns an error if no message bus is configured (use `with_bus()` first).
    pub async fn execute_with_bus(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: &str,
        message_history: Option<Vec<ModelRequest>>,
        tool_registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
    ) -> Result<ExecutorResult, ExecutorError> {
        let bus = self.bus.as_ref().ok_or(ExecutorError::Config(
            "No message bus configured. Use with_bus() first.".into(),
        ))?;

        // Create event bridge for this agent
        let mut bridge =
            EventBridge::new(bus.clone(), spot_agent.name(), spot_agent.display_name());

        bridge.agent_started();

        // Track tool returns during streaming so we can reconstruct message history.
        let tool_return_recorder: Arc<Mutex<Vec<ToolReturnPart>>> =
            Arc::new(Mutex::new(Vec::new()));

        // Start with any provided history, then add the current user prompt.
        let mut messages = message_history.clone().unwrap_or_default();
        let mut user_req = ModelRequest::new();
        user_req.add_user_prompt(prompt.to_string());
        messages.push(user_req);

        // Use internal streaming execution
        let mut stream = self
            .execute_stream_internal(
                spot_agent,
                model_name,
                UserContent::text(prompt),
                message_history,
                tool_registry,
                mcp_manager,
                Some(Arc::clone(&tool_return_recorder)),
            )
            .await?;

        // Process stream and accumulate results
        let (accumulated_text, final_run_id, messages) = self
            .process_stream(
                &mut stream,
                &mut bridge,
                messages,
                model_name,
                &tool_return_recorder,
            )
            .await?;

        // Get the run_id (from RunComplete event)
        let run_id = final_run_id.ok_or_else(|| {
            ExecutorError::Execution("Stream ended without RunComplete event".into())
        })?;

        bridge.agent_completed(&run_id);

        Ok(ExecutorResult {
            output: accumulated_text,
            messages,
            run_id,
        })
    }

    /// Execute agent with images (multimodal content).
    ///
    /// Similar to `execute_with_bus` but accepts image data alongside text.
    /// Images are sent as base64-encoded PNG data to vision-capable models.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_with_images(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: &str,
        images: &[(Vec<u8>, ImageMediaType)],
        message_history: Option<Vec<ModelRequest>>,
        tool_registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
    ) -> Result<ExecutorResult, ExecutorError> {
        let bus = self.bus.as_ref().ok_or(ExecutorError::Config(
            "No message bus configured. Use with_bus() first.".into(),
        ))?;

        // Create event bridge for this agent
        let mut bridge =
            EventBridge::new(bus.clone(), spot_agent.name(), spot_agent.display_name());

        bridge.agent_started();

        // Track tool returns during streaming so we can reconstruct message history.
        let tool_return_recorder: Arc<Mutex<Vec<ToolReturnPart>>> =
            Arc::new(Mutex::new(Vec::new()));

        // Build the user content (text + images)
        let user_content = if images.is_empty() {
            UserContent::text(prompt)
        } else {
            let mut parts = Vec::new();
            if !prompt.is_empty() {
                parts.push(UserContentPart::text(prompt));
            }
            for (image_data, media_type) in images {
                parts.push(UserContentPart::image_binary(
                    image_data.clone(),
                    *media_type,
                ));
            }
            UserContent::parts(parts)
        };

        // Log what we built
        match &user_content {
            UserContent::Text(t) => {
                info!(text_len = t.len(), "Built text-only content")
            }
            UserContent::Parts(parts) => {
                let image_parts = parts
                    .iter()
                    .filter(|p| matches!(p, UserContentPart::Image { .. }))
                    .count();
                let text_parts = parts
                    .iter()
                    .filter(|p| matches!(p, UserContentPart::Text { .. }))
                    .count();
                info!(
                    text_parts,
                    image_parts,
                    total_parts = parts.len(),
                    "Built multimodal content with images"
                );
            }
        }

        // Start with any provided history, then add the current user prompt.
        let mut messages = message_history.clone().unwrap_or_default();
        let mut user_req = ModelRequest::new();
        user_req.add_user_prompt(user_content.clone());
        messages.push(user_req);

        // Use internal streaming execution - pass user_content which includes images!
        let mut stream = self
            .execute_stream_internal(
                spot_agent,
                model_name,
                user_content,
                message_history,
                tool_registry,
                mcp_manager,
                Some(Arc::clone(&tool_return_recorder)),
            )
            .await?;

        // Process stream and accumulate results
        let (accumulated_text, final_run_id, messages) = self
            .process_stream(
                &mut stream,
                &mut bridge,
                messages,
                model_name,
                &tool_return_recorder,
            )
            .await?;

        let run_id = final_run_id.ok_or_else(|| {
            ExecutorError::Execution("Stream ended without RunComplete event".into())
        })?;

        bridge.agent_completed(&run_id);

        Ok(ExecutorResult {
            output: accumulated_text,
            messages,
            run_id,
        })
    }

    /// Execute an agent with streaming output.
    ///
    /// **Note**: For new code, prefer [`execute_with_bus`] which automatically
    /// publishes events to a message bus that renderers can subscribe to.
    /// This method is useful when you need direct control over event handling.
    ///
    /// Returns a stream receiver for consuming events in real-time.
    pub async fn execute_stream(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: &str,
        message_history: Option<Vec<ModelRequest>>,
        tool_registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
    ) -> Result<ExecutorStreamReceiver, ExecutorError> {
        self.execute_stream_internal(
            spot_agent,
            model_name,
            UserContent::text(prompt),
            message_history,
            tool_registry,
            mcp_manager,
            None,
        )
        .await
    }

    /// Collect MCP tools from running servers, filtered by agent attachments.
    ///
    /// Only returns tools from MCP servers that are attached to the given agent.
    /// If no agent_name is provided or the agent has no attachments, returns tools
    /// from ALL running servers (for backwards compatibility).
    async fn collect_mcp_tools(
        &self,
        mcp_manager: &McpManager,
        agent_name: Option<&str>,
    ) -> Vec<(ToolDefinition, Arc<dyn Tool + Send + Sync>)> {
        let mut tools = Vec::new();

        // Get agent's MCP attachments from settings
        let attached_mcps: Option<Vec<String>> = agent_name.and_then(|name| {
            let settings = Settings::new(self.db);
            let mcps = settings.get_agent_mcps(name);
            if mcps.is_empty() {
                None // No attachments = use all MCPs
            } else {
                Some(mcps)
            }
        });

        // Get all tools from running MCP servers
        let all_mcp_tools = mcp_manager.list_all_tools().await;

        for (server_name, server_tools) in all_mcp_tools {
            // Filter by agent attachments if specified
            if let Some(ref attached) = attached_mcps {
                if !attached.contains(&server_name) {
                    debug!(
                        agent = agent_name.unwrap_or("unknown"),
                        server = %server_name,
                        "Skipping MCP server - not attached to agent"
                    );
                    continue;
                }
            }

            debug!(
                agent = agent_name.unwrap_or("unknown"),
                server = %server_name,
                tool_count = server_tools.len(),
                "Including MCP tools from server"
            );

            for mcp_tool in server_tools {
                // Create a tool definition from MCP tool
                let def = ToolDefinition::new(
                    mcp_tool.name.clone(),
                    mcp_tool.description.clone().unwrap_or_default(),
                )
                .with_parameters(mcp_tool.input_schema.clone());

                // Create an MCP tool executor
                let executor = McpToolExecutor {
                    server_name: server_name.clone(),
                    tool_name: mcp_tool.name.clone(),
                    mcp_manager_ptr: mcp_manager as *const McpManager,
                };

                tools.push((def, Arc::new(executor) as Arc<dyn Tool + Send + Sync>));
            }
        }

        if tools.is_empty() && attached_mcps.is_some() {
            debug!(
                agent = agent_name.unwrap_or("unknown"),
                "No MCP tools available - attached servers may not be running"
            );
        }

        tools
    }
}

// Private implementation details in a separate impl block
mod streaming;

/// Legacy execute_agent function for backwards compatibility.
///
/// Prefer using `AgentExecutor` directly for new code.
#[deprecated(since = "0.2.0", note = "Use AgentExecutor::execute() instead")]
pub async fn execute_agent(
    db: &Database,
    agent: &dyn SpotAgent,
    model_name: &str,
    prompt: &str,
    message_history: &mut Vec<ModelRequest>,
) -> Result<String, ExecutorError> {
    let model_registry = ModelRegistry::load_from_db(db).unwrap_or_default();
    let executor = AgentExecutor::new(db, &model_registry);
    let tool_registry = SpotToolRegistry::new();
    let mcp_manager = McpManager::new();

    // Convert mutable history to owned
    let history = if message_history.is_empty() {
        None
    } else {
        Some(message_history.clone())
    };

    let result = executor
        .execute(
            agent,
            model_name,
            prompt,
            history,
            &tool_registry,
            &mcp_manager,
        )
        .await?;

    // Update the caller's history
    *message_history = result.messages;

    Ok(result.output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::MessageBus;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, db)
    }

    #[test]
    fn test_agent_executor_new() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        assert!(executor.bus.is_none());
    }

    #[test]
    fn test_agent_executor_with_bus() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let bus = MessageBus::new();
        let sender = bus.sender();
        let executor = AgentExecutor::new(&db, &registry).with_bus(sender);
        assert!(executor.bus.is_some());
    }

    #[test]
    fn test_agent_executor_with_bus_builder_pattern() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let bus = MessageBus::new();
        let executor = AgentExecutor::new(&db, &registry).with_bus(bus.sender());
        assert!(executor.bus.is_some());
    }

    #[test]
    fn test_filter_tools_removes_share_your_reasoning_when_disabled() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec!["read_file", "share_your_reasoning", "write_file"];
        let filtered = executor.filter_tools(tools);
        assert!(!filtered.contains(&"share_your_reasoning"));
        assert!(filtered.contains(&"read_file"));
        assert!(filtered.contains(&"write_file"));
    }

    #[test]
    fn test_filter_tools_keeps_share_your_reasoning_when_enabled() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let settings = Settings::new(&db);
        settings.set("show_reasoning", "true").unwrap();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec!["read_file", "share_your_reasoning", "write_file"];
        let filtered = executor.filter_tools(tools);
        assert!(filtered.contains(&"share_your_reasoning"));
        assert!(filtered.contains(&"read_file"));
        assert!(filtered.contains(&"write_file"));
    }

    #[test]
    fn test_filter_tools_removes_invoke_agent() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec!["read_file", "invoke_agent", "write_file"];
        let filtered = executor.filter_tools(tools);
        assert!(!filtered.contains(&"invoke_agent"));
        assert!(filtered.contains(&"read_file"));
        assert!(filtered.contains(&"write_file"));
    }

    #[test]
    fn test_filter_tools_removes_list_agents() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec!["read_file", "list_agents", "write_file"];
        let filtered = executor.filter_tools(tools);
        assert!(!filtered.contains(&"list_agents"));
        assert!(filtered.contains(&"read_file"));
        assert!(filtered.contains(&"write_file"));
    }

    #[test]
    fn test_filter_tools_removes_all_special_tools() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec![
            "read_file",
            "invoke_agent",
            "list_agents",
            "share_your_reasoning",
            "write_file",
        ];
        let filtered = executor.filter_tools(tools);
        assert!(!filtered.contains(&"invoke_agent"));
        assert!(!filtered.contains(&"list_agents"));
        assert!(!filtered.contains(&"share_your_reasoning"));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_tools_preserves_other_tools() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec![
            "read_file",
            "write_file",
            "list_files",
            "grep",
            "shell_command",
            "apply_diff",
        ];
        let filtered = executor.filter_tools(tools);
        assert_eq!(filtered.len(), 6);
        for tool in &[
            "read_file",
            "write_file",
            "list_files",
            "grep",
            "shell_command",
            "apply_diff",
        ] {
            assert!(filtered.contains(tool));
        }
    }

    #[test]
    fn test_filter_tools_empty_input() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools: Vec<&str> = vec![];
        let filtered = executor.filter_tools(tools);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_wants_invoke_agent_true() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = ["read_file", "invoke_agent", "write_file"];
        assert!(executor.wants_invoke_agent(&tools));
    }

    #[test]
    fn test_wants_invoke_agent_false() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = ["read_file", "write_file", "grep"];
        assert!(!executor.wants_invoke_agent(&tools));
    }

    #[test]
    fn test_wants_invoke_agent_empty() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools: [&str; 0] = [];
        assert!(!executor.wants_invoke_agent(&tools));
    }

    #[test]
    fn test_wants_list_agents_true() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = ["read_file", "list_agents", "write_file"];
        assert!(executor.wants_list_agents(&tools));
    }

    #[test]
    fn test_wants_list_agents_false() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = ["read_file", "write_file", "grep"];
        assert!(!executor.wants_list_agents(&tools));
    }

    #[test]
    fn test_wants_list_agents_empty() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools: [&str; 0] = [];
        assert!(!executor.wants_list_agents(&tools));
    }

    struct MockAgent {
        name: &'static str,
    }

    impl SpotAgent for MockAgent {
        fn name(&self) -> &str {
            self.name
        }

        fn display_name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "Mock agent for testing"
        }

        fn system_prompt(&self) -> String {
            "You are a test agent.".to_string()
        }

        fn available_tools(&self) -> Vec<&str> {
            vec!["read_file"]
        }
    }

    #[tokio::test]
    async fn test_execute_with_bus_without_bus_returns_config_error() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let agent = MockAgent { name: "test" };
        let tool_registry = SpotToolRegistry::new();
        let mcp_manager = McpManager::new();

        let result = executor
            .execute_with_bus(
                &agent,
                "gpt-4",
                "test prompt",
                None,
                &tool_registry,
                &mcp_manager,
            )
            .await;

        assert!(result.is_err());
        match result {
            Err(ExecutorError::Config(msg)) => {
                assert!(msg.contains("No message bus configured"));
                assert!(msg.contains("with_bus()"));
            }
            Err(e) => panic!("Expected ExecutorError::Config, got error: {}", e),
            Ok(_) => panic!("Expected ExecutorError::Config, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_images_without_bus_returns_config_error() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let agent = MockAgent { name: "test" };
        let tool_registry = SpotToolRegistry::new();
        let mcp_manager = McpManager::new();

        let result = executor
            .execute_with_images(
                &agent,
                "gpt-4",
                "test prompt",
                &[],
                None,
                &tool_registry,
                &mcp_manager,
            )
            .await;

        assert!(result.is_err());
        match result {
            Err(ExecutorError::Config(msg)) => {
                assert!(msg.contains("No message bus configured"));
            }
            Err(e) => panic!("Expected ExecutorError::Config, got error: {}", e),
            Ok(_) => panic!("Expected ExecutorError::Config, got Ok"),
        }
    }

    #[test]
    fn test_filter_tools_with_all_special_tools_and_reasoning_enabled() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let settings = Settings::new(&db);
        settings.set("show_reasoning", "true").unwrap();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec![
            "read_file",
            "invoke_agent",
            "list_agents",
            "share_your_reasoning",
            "write_file",
        ];
        let filtered = executor.filter_tools(tools);
        assert!(filtered.contains(&"share_your_reasoning"));
        assert!(!filtered.contains(&"invoke_agent"));
        assert!(!filtered.contains(&"list_agents"));
        assert!(filtered.contains(&"read_file"));
        assert!(filtered.contains(&"write_file"));
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_wants_both_agent_tools() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = ["invoke_agent", "list_agents", "read_file"];
        assert!(executor.wants_invoke_agent(&tools));
        assert!(executor.wants_list_agents(&tools));
    }

    #[test]
    fn test_filter_tools_only_special_tools() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec!["invoke_agent", "list_agents", "share_your_reasoning"];
        let filtered = executor.filter_tools(tools);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_tools_duplicate_tools() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = vec!["read_file", "read_file", "write_file"];
        let filtered = executor.filter_tools(tools);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_wants_invoke_agent_similar_names() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        let tools = ["invoke_agent_v2", "my_invoke_agent", "invoke"];
        assert!(!executor.wants_invoke_agent(&tools));
        let tools = ["invoke_agent_v2", "invoke_agent"];
        assert!(executor.wants_invoke_agent(&tools));
    }

    #[test]
    fn test_executor_lifetime_with_registry() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let executor = AgentExecutor::new(&db, &registry);
        assert!(registry.is_empty());
        assert!(executor.bus.is_none());
    }
}
