//! Agent executor - runs agents using serdesAI's Agent API.
//!
//! This module provides the execution layer for SpotAgents, using
//! serdesAI's agent loop with proper tool calling and streaming support.

use crate::agents::{AgentManager, SpotAgent};
use crate::auth;
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{EventBridge, MessageSender};
use crate::models::{resolve_api_key, ModelRegistry, ModelType};
use crate::session::SessionManager;
use crate::tools::agent_tools::InvokeAgentTool;
use crate::tools::registry::SpotToolRegistry;
use tracing::{debug, error, info, warn};

use serdes_ai_agent::{agent, RunOptions};
use serdes_ai_core::messages::ToolCallArgs;
use serdes_ai_core::messages::{ImageMediaType, UserContent, UserContentPart};
use serdes_ai_core::{
    ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart, ModelSettings, TextPart,
    ToolCallPart, ToolReturnPart,
};
use serdes_ai_models::{
    infer_model, openai::OpenAIChatModel, Model, ModelError, ModelProfile, ModelRequestParameters,
    StreamedResponse,
};
use serdes_ai_tools::{RunContext, Tool, ToolDefinition, ToolError, ToolReturn};

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

// Re-export stream event for consumers
pub use serdes_ai_agent::AgentStreamEvent as StreamEvent;

/// Wrapper to make Arc<dyn Model> implement Model.
///
/// This allows us to use dynamically dispatched models with serdesAI's
/// agent builder, which requires a concrete Model type.
struct ArcModel(Arc<dyn Model>);

#[async_trait]
impl Model for ArcModel {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn system(&self) -> &str {
        self.0.system()
    }

    fn identifier(&self) -> String {
        self.0.identifier()
    }

    async fn request(
        &self,
        messages: &[ModelRequest],
        settings: &ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<ModelResponse, ModelError> {
        self.0.request(messages, settings, params).await
    }

    async fn request_stream(
        &self,
        messages: &[ModelRequest],
        settings: &ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<StreamedResponse, ModelError> {
        self.0.request_stream(messages, settings, params).await
    }

    fn profile(&self) -> &ModelProfile {
        self.0.profile()
    }

    async fn count_tokens(&self, messages: &[ModelRequest]) -> Result<u64, ModelError> {
        self.0.count_tokens(messages).await
    }
}

/// Wrapper that adapts an `Arc<dyn Tool>` to work as a `ToolExecutor<()>`.
///
/// This bridges our Tool implementations (which use `call()`) to
/// serdesAI's executor interface (which uses `execute()`).
struct ToolExecutorAdapter {
    tool: Arc<dyn Tool + Send + Sync>,
}

impl ToolExecutorAdapter {
    fn new(tool: Arc<dyn Tool + Send + Sync>) -> Self {
        Self { tool }
    }
}

#[async_trait]
impl serdes_ai_agent::ToolExecutor<()> for ToolExecutorAdapter {
    async fn execute(
        &self,
        args: JsonValue,
        ctx: &serdes_ai_agent::RunContext<()>,
    ) -> Result<ToolReturn, ToolError> {
        // Convert serdes_ai_agent::RunContext to serdes_ai_tools::RunContext
        let tool_ctx = RunContext::minimal(&ctx.model_name);
        self.tool.call(&tool_ctx, args).await
    }
}

/// Wraps a tool executor and records tool returns during streaming.
///
/// `serdes_ai_agent::AgentStreamEvent` does not include tool return payloads, but we
/// need them for accurate `message_history` reconstruction.
struct RecordingToolExecutor<E> {
    inner: E,
    recorder: Arc<Mutex<Vec<ToolReturnPart>>>,
}

impl<E> RecordingToolExecutor<E> {
    fn new(inner: E, recorder: Arc<Mutex<Vec<ToolReturnPart>>>) -> Self {
        Self { inner, recorder }
    }
}

#[async_trait]
impl<E> serdes_ai_agent::ToolExecutor<()> for RecordingToolExecutor<E>
where
    E: serdes_ai_agent::ToolExecutor<()> + Send + Sync,
{
    async fn execute(
        &self,
        args: JsonValue,
        ctx: &serdes_ai_agent::RunContext<()>,
    ) -> Result<ToolReturn, ToolError> {
        let result = self.inner.execute(args, ctx).await;

        // Best-effort tool name/id capture; used to reconstruct `ToolReturnPart`s.
        let tool_name = ctx
            .tool_name
            .clone()
            .unwrap_or_else(|| "unknown_tool".to_string());

        let mut part = match &result {
            Ok(ret) => ToolReturnPart::new(&tool_name, ret.content.clone()),
            Err(e) => ToolReturnPart::error(&tool_name, format!("Tool error: {}", e)),
        };

        if let Some(tool_call_id) = ctx.tool_call_id.clone() {
            part = part.with_tool_call_id(tool_call_id);
        }

        self.recorder.lock().await.push(part);
        result
    }
}

/// Executor for invoke_agent that has access to all required dependencies.
struct InvokeAgentExecutor {
    db_path: std::path::PathBuf,
    current_model: String,
    /// Optional message bus for sub-agent event publishing.
    bus: Option<MessageSender>,
}

impl InvokeAgentExecutor {
    /// Create executor with message bus (preferred - enables visible sub-agent output).
    fn new(db: &Database, current_model: &str, bus: MessageSender) -> Self {
        Self {
            db_path: db.path().to_path_buf(),
            current_model: current_model.to_string(),
            bus: Some(bus),
        }
    }

    /// Create executor without message bus (legacy - sub-agent output not visible).
    fn new_legacy(db: &Database, current_model: &str) -> Self {
        Self {
            db_path: db.path().to_path_buf(),
            current_model: current_model.to_string(),
            bus: None,
        }
    }

    fn definition() -> ToolDefinition {
        InvokeAgentTool.definition()
    }
}

#[async_trait]
impl serdes_ai_agent::ToolExecutor<()> for InvokeAgentExecutor {
    async fn execute(
        &self,
        args: JsonValue,
        _ctx: &serdes_ai_agent::RunContext<()>,
    ) -> Result<ToolReturn, ToolError> {
        #[derive(serde::Deserialize)]
        struct Args {
            agent_name: String,
            prompt: String,
            #[serde(default)]
            session_id: Option<String>,
        }

        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::execution_failed(format!("Invalid arguments: {}", e)))?;

        debug!(agent = %args.agent_name, "Invoking sub-agent");

        // Clone the data we need for the blocking task
        let db_path = self.db_path.clone();
        let current_model = self.current_model.clone();
        let agent_name = args.agent_name.clone();
        let prompt = args.prompt.clone();
        let session_id = args.session_id.clone();
        let bus = self.bus.clone(); // Clone bus for the spawned task

        // Run the agent in a blocking context to handle the non-Send Database
        let result = tokio::task::spawn_blocking(move || {
            // Create a new runtime for the blocking task
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            rt.block_on(async {
                // Open a fresh database connection
                let db = Database::open_at(db_path)
                    .map_err(|e| format!("Failed to open database: {}", e))?;

                // Load fresh registries
                let model_registry = ModelRegistry::load_from_db(&db).unwrap_or_default();
                let agent_manager = AgentManager::new();
                let tool_registry = SpotToolRegistry::new();
                let mcp_manager = McpManager::new();

                // Find the agent
                let agent = agent_manager
                    .get(&agent_name)
                    .ok_or_else(|| format!("Agent not found: {}", agent_name))?;

                // Get the effective model for this agent (pinned or current)
                let effective_model = {
                    let settings = Settings::new(&db);
                    settings
                        .get_agent_pinned_model(&agent_name)
                        .unwrap_or_else(|| current_model.clone())
                };

                // Load session history if session_id provided
                let session_manager = SessionManager::new();
                let message_history = session_id.as_ref().and_then(|sid| {
                    match session_manager.load(sid) {
                        Ok(data) => {
                            debug!(session_id = %sid, messages = data.messages.len(), "Loaded session history");
                            Some(data.messages)
                        }
                        Err(e) => {
                            debug!(session_id = %sid, error = %e, "No existing session, starting fresh");
                            None
                        }
                    }
                });

                // Create executor - with bus if available for visible sub-agent output
                let executor = AgentExecutor::new(&db, &model_registry);

                let result = if let Some(bus) = bus {
                    // Use execute_with_bus - events flow to the same bus!
                    executor
                        .with_bus(bus)
                        .execute_with_bus(
                            agent,
                            &effective_model,
                            &prompt,
                            message_history,
                            &tool_registry,
                            &mcp_manager,
                        )
                        .await
                        .map_err(|e| format!("Agent execution failed: {}", e))?
                } else {
                    // Legacy: no bus, sub-agent output only in response
                    executor
                        .execute(
                            agent,
                            &effective_model,
                            &prompt,
                            message_history,
                            &tool_registry,
                            &mcp_manager,
                        )
                        .await
                        .map_err(|e| format!("Agent execution failed: {}", e))?
                };

                // Save session for future continuation
                let final_session_id = session_id.clone().unwrap_or_else(|| {
                    session_manager.generate_name(&agent_name)
                });

                // Only save if we have messages (non-streaming mode returns them)
                if !result.messages.is_empty() {
                    if let Err(e) = session_manager.save(
                        &final_session_id,
                        &result.messages,
                        &agent_name,
                        &effective_model,
                    ) {
                        warn!(error = %e, "Failed to save session");
                    } else {
                        debug!(session_id = %final_session_id, messages = result.messages.len(), "Saved session");
                    }
                }

                Ok::<_, String>((result.output, final_session_id))
            })
        })
        .await
        .map_err(|e| ToolError::execution_failed(format!("Task join error: {}", e)))?
        .map_err(ToolError::execution_failed)?;

        Ok(ToolReturn::json(serde_json::json!({
            "agent": args.agent_name,
            "response": result.0,
            "session_id": result.1,  // Now returns the actual session_id used
            "success": true
        })))
    }
}

/// Executor for list_agents that returns available agents.
struct ListAgentsExecutor {
    db_path: std::path::PathBuf,
}

impl ListAgentsExecutor {
    fn new(db: &Database) -> Self {
        Self {
            db_path: db.path().to_path_buf(),
        }
    }

    fn definition() -> ToolDefinition {
        ToolDefinition::new(
            "list_agents",
            "List all available agents that can be invoked.",
        )
    }
}

#[async_trait]
impl serdes_ai_agent::ToolExecutor<()> for ListAgentsExecutor {
    async fn execute(
        &self,
        _args: JsonValue,
        _ctx: &serdes_ai_agent::RunContext<()>,
    ) -> Result<ToolReturn, ToolError> {
        // Open database and list agents (we don't actually need db for listing)
        let agent_manager = AgentManager::new();
        let agents: Vec<_> = agent_manager
            .list()
            .iter()
            .map(|info| {
                serde_json::json!({
                    "name": info.name,
                    "display_name": info.display_name,
                    "description": info.description
                })
            })
            .collect();

        Ok(ToolReturn::json(serde_json::json!({
            "agents": agents,
            "count": agents.len()
        })))
    }
}

/// Executor for running SpotAgents with serdesAI's agent loop.
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
        _mcp_manager: &McpManager,
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
            .max_tokens(4096);

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
            .collect_mcp_tools(_mcp_manager, Some(spot_agent.name()))
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

        struct RawToolCall {
            tool_name: String,
            tool_call_id: Option<String>,
            args_buffer: String,
        }

        // Accumulate text for the final output (since RunComplete only has run_id)
        let mut accumulated_text = String::new();
        let mut final_run_id: Option<String> = None;

        // Track per-response state so we can rebuild `ModelResponse` parts.
        let mut current_response_text = String::new();
        let mut completed_tool_calls: Vec<RawToolCall> = Vec::new();
        let mut in_progress_tool_call: Option<RawToolCall> = None;

        // Track tool return parts emitted by tool executors.
        let mut expected_tool_returns: usize = 0;
        let mut tool_return_index: usize = 0;
        let mut pending_tool_returns: Vec<ToolReturnPart> = Vec::new();
        let mut pending_tool_calls: std::collections::VecDeque<(String, Option<String>)> =
            std::collections::VecDeque::new();

        // Process all events through the bridge
        while let Some(event_result) = stream.recv().await {
            match event_result {
                Ok(event) => {
                    let tool_executed_info = match &event {
                        StreamEvent::ToolExecuted {
                            tool_name,
                            success,
                            error,
                        } => Some((tool_name.clone(), *success, error.clone())),
                        _ => None,
                    };

                    match &event {
                        StreamEvent::RequestStart { .. } => {
                            current_response_text.clear();
                            completed_tool_calls.clear();
                            in_progress_tool_call = None;
                        }
                        StreamEvent::TextDelta { text } => {
                            accumulated_text.push_str(text);
                            current_response_text.push_str(text);
                        }
                        StreamEvent::ToolCallStart {
                            tool_name,
                            tool_call_id,
                        } => {
                            if let Some(tc) = in_progress_tool_call.take() {
                                completed_tool_calls.push(tc);
                            }
                            in_progress_tool_call = Some(RawToolCall {
                                tool_name: tool_name.clone(),
                                tool_call_id: tool_call_id.clone(),
                                args_buffer: String::new(),
                            });
                        }
                        StreamEvent::ToolCallDelta { delta } => {
                            if let Some(tc) = in_progress_tool_call.as_mut() {
                                tc.args_buffer.push_str(delta);
                            }
                        }
                        StreamEvent::ResponseComplete { .. } => {
                            if let Some(tc) = in_progress_tool_call.take() {
                                completed_tool_calls.push(tc);
                            }

                            pending_tool_calls = completed_tool_calls
                                .iter()
                                .map(|tc| (tc.tool_name.clone(), tc.tool_call_id.clone()))
                                .collect();

                            let mut response_parts: Vec<ModelResponsePart> = Vec::new();

                            if !current_response_text.is_empty() {
                                response_parts.push(ModelResponsePart::Text(TextPart::new(
                                    current_response_text.clone(),
                                )));
                            }

                            for tc in completed_tool_calls.drain(..) {
                                let mut part = ToolCallPart::new(
                                    tc.tool_name,
                                    ToolCallArgs::from(tc.args_buffer),
                                );
                                if let Some(id) = tc.tool_call_id {
                                    part = part.with_tool_call_id(id);
                                }
                                response_parts.push(ModelResponsePart::ToolCall(part));
                            }

                            if !response_parts.is_empty() {
                                let response = ModelResponse::with_parts(response_parts)
                                    .with_model_name(model_name.to_string());

                                let mut response_req = ModelRequest::new();
                                response_req
                                    .parts
                                    .push(ModelRequestPart::ModelResponse(Box::new(response)));
                                messages.push(response_req);
                            }

                            // If tool calls were present, tool execution events follow and we can
                            // append tool returns once they complete.
                            expected_tool_returns = pending_tool_calls.len();
                            pending_tool_returns.clear();
                            current_response_text.clear();
                        }
                        StreamEvent::RunComplete { run_id } => {
                            final_run_id = Some(run_id.clone());
                        }
                        _ => {}
                    }

                    // Tool return payloads aren't present in stream events, so we stitch them
                    // in from a recorder wrapped around tool executors.
                    //
                    // Note: some tool failures (e.g., unknown tool) never call an executor; in
                    // those cases we synthesize an error ToolReturnPart from the ToolExecuted event.
                    if let Some((tool_name, success, error)) = tool_executed_info {
                        if expected_tool_returns > 0 {
                            let tool_call_id =
                                pending_tool_calls.pop_front().and_then(|(_, id)| id);

                            let mut part = {
                                let next_part = {
                                    let recorded = tool_return_recorder.lock().await;
                                    recorded.get(tool_return_index).cloned()
                                };

                                if let Some(part) = next_part {
                                    tool_return_index += 1;
                                    part
                                } else {
                                    let msg = error.unwrap_or_else(|| {
                                        if success {
                                            "Tool executed but no tool return was recorded"
                                                .to_string()
                                        } else {
                                            "Tool failed".to_string()
                                        }
                                    });
                                    ToolReturnPart::error(&tool_name, msg)
                                }
                            };

                            if part.tool_call_id.is_none() {
                                if let Some(id) = tool_call_id {
                                    part = part.with_tool_call_id(id);
                                }
                            }

                            pending_tool_returns.push(part);

                            if pending_tool_returns.len() == expected_tool_returns {
                                let mut tool_req = ModelRequest::new();
                                for part in pending_tool_returns.drain(..) {
                                    tool_req.parts.push(ModelRequestPart::ToolReturn(part));
                                }
                                messages.push(tool_req);
                                expected_tool_returns = 0;
                            }
                        }
                    }

                    bridge.process(event);
                }
                Err(e) => {
                    bridge.agent_error(&e.to_string());
                    return Err(e);
                }
            }
        }

        // Flush any tool returns we managed to capture, even if the stream ended unexpectedly.
        if !pending_tool_returns.is_empty() {
            let mut tool_req = ModelRequest::new();
            for part in pending_tool_returns {
                tool_req.parts.push(ModelRequestPart::ToolReturn(part));
            }
            messages.push(tool_req);
        }

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
    ///
    /// # Arguments
    /// * `spot_agent` - The agent to execute
    /// * `model_name` - The model to use (should be vision-capable for images)
    /// * `prompt` - Text prompt
    /// * `images` - Vector of (PNG bytes, media type) tuples
    /// * `message_history` - Optional conversation history
    /// * `tool_registry` - Available tools
    /// * `mcp_manager` - MCP server manager
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

        struct RawToolCall {
            tool_name: String,
            tool_call_id: Option<String>,
            args_buffer: String,
        }

        // Accumulate text for the final output
        let mut accumulated_text = String::new();
        let mut final_run_id: Option<String> = None;

        // Track per-response state so we can rebuild `ModelResponse` parts.
        let mut current_response_text = String::new();
        let mut completed_tool_calls: Vec<RawToolCall> = Vec::new();
        let mut in_progress_tool_call: Option<RawToolCall> = None;

        // Track tool return parts emitted by tool executors.
        let mut expected_tool_returns: usize = 0;
        let mut tool_return_index: usize = 0;
        let mut pending_tool_returns: Vec<ToolReturnPart> = Vec::new();
        let mut pending_tool_calls: std::collections::VecDeque<(String, Option<String>)> =
            std::collections::VecDeque::new();

        // Process all events through the bridge
        while let Some(event_result) = stream.recv().await {
            match event_result {
                Ok(event) => {
                    let tool_executed_info = match &event {
                        StreamEvent::ToolExecuted {
                            tool_name,
                            success,
                            error,
                        } => Some((tool_name.clone(), *success, error.clone())),
                        _ => None,
                    };

                    match &event {
                        StreamEvent::RequestStart { .. } => {
                            current_response_text.clear();
                            completed_tool_calls.clear();
                            in_progress_tool_call = None;
                        }
                        StreamEvent::TextDelta { text } => {
                            accumulated_text.push_str(text);
                            current_response_text.push_str(text);
                        }
                        StreamEvent::ToolCallStart {
                            tool_name,
                            tool_call_id,
                        } => {
                            if let Some(tc) = in_progress_tool_call.take() {
                                completed_tool_calls.push(tc);
                            }
                            in_progress_tool_call = Some(RawToolCall {
                                tool_name: tool_name.clone(),
                                tool_call_id: tool_call_id.clone(),
                                args_buffer: String::new(),
                            });
                        }
                        StreamEvent::ToolCallDelta { delta } => {
                            if let Some(tc) = in_progress_tool_call.as_mut() {
                                tc.args_buffer.push_str(delta);
                            }
                        }
                        StreamEvent::ResponseComplete { .. } => {
                            if let Some(tc) = in_progress_tool_call.take() {
                                completed_tool_calls.push(tc);
                            }

                            pending_tool_calls = completed_tool_calls
                                .iter()
                                .map(|tc| (tc.tool_name.clone(), tc.tool_call_id.clone()))
                                .collect();

                            let mut response_parts: Vec<ModelResponsePart> = Vec::new();

                            if !current_response_text.is_empty() {
                                response_parts.push(ModelResponsePart::Text(TextPart::new(
                                    current_response_text.clone(),
                                )));
                            }

                            for tc in completed_tool_calls.drain(..) {
                                let mut part = ToolCallPart::new(
                                    tc.tool_name,
                                    ToolCallArgs::from(tc.args_buffer),
                                );
                                if let Some(id) = tc.tool_call_id {
                                    part = part.with_tool_call_id(id);
                                }
                                response_parts.push(ModelResponsePart::ToolCall(part));
                            }

                            if !response_parts.is_empty() {
                                let response = ModelResponse::with_parts(response_parts)
                                    .with_model_name(model_name.to_string());

                                let mut response_req = ModelRequest::new();
                                response_req
                                    .parts
                                    .push(ModelRequestPart::ModelResponse(Box::new(response)));
                                messages.push(response_req);
                            }

                            expected_tool_returns = pending_tool_calls.len();
                            pending_tool_returns.clear();
                            current_response_text.clear();
                        }
                        StreamEvent::RunComplete { run_id } => {
                            final_run_id = Some(run_id.clone());
                        }
                        _ => {}
                    }

                    if let Some((tool_name, success, error)) = tool_executed_info {
                        if expected_tool_returns > 0 {
                            let tool_call_id =
                                pending_tool_calls.pop_front().and_then(|(_, id)| id);

                            let mut part = {
                                let next_part = {
                                    let recorded = tool_return_recorder.lock().await;
                                    recorded.get(tool_return_index).cloned()
                                };

                                if let Some(part) = next_part {
                                    tool_return_index += 1;
                                    part
                                } else {
                                    let msg = error.unwrap_or_else(|| {
                                        if success {
                                            "Tool executed but no tool return was recorded"
                                                .to_string()
                                        } else {
                                            "Tool failed".to_string()
                                        }
                                    });
                                    ToolReturnPart::error(&tool_name, msg)
                                }
                            };

                            if part.tool_call_id.is_none() {
                                if let Some(id) = tool_call_id {
                                    part = part.with_tool_call_id(id);
                                }
                            }

                            pending_tool_returns.push(part);

                            if pending_tool_returns.len() == expected_tool_returns {
                                let mut tool_req = ModelRequest::new();
                                for part in pending_tool_returns.drain(..) {
                                    tool_req.parts.push(ModelRequestPart::ToolReturn(part));
                                }
                                messages.push(tool_req);
                                expected_tool_returns = 0;
                            }
                        }
                    }

                    bridge.process(event);
                }
                Err(e) => {
                    bridge.agent_error(&e.to_string());
                    return Err(e);
                }
            }
        }

        // Flush any tool returns we managed to capture
        if !pending_tool_returns.is_empty() {
            let mut tool_req = ModelRequest::new();
            for part in pending_tool_returns {
                tool_req.parts.push(ModelRequestPart::ToolReturn(part));
            }
            messages.push(tool_req);
        }

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
    /// Uses a channel internally to handle the lifetime issues with agent streams.
    ///
    /// # Example
    /// ```ignore
    /// let registry = SpotToolRegistry::new();
    /// let mut stream = executor.execute_stream(agent, model, prompt, None, &registry).await?;
    /// while let Some(event) = stream.recv().await {
    ///     match event? {
    ///         StreamEvent::TextDelta { text } => print!("{}", text),
    ///         StreamEvent::ToolCallStart { tool_name, .. } => println!("🔧 Calling: {}", tool_name),
    ///         _ => {}
    ///     }
    /// }
    /// ```
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

    #[allow(clippy::too_many_arguments)]
    async fn execute_stream_internal(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: UserContent,
        message_history: Option<Vec<ModelRequest>>,
        tool_registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
        tool_return_recorder: Option<Arc<Mutex<Vec<ToolReturnPart>>>>,
    ) -> Result<ExecutorStreamReceiver, ExecutorError> {
        // Get the model (handles OAuth models and custom endpoints)
        let model = get_model(self.db, model_name, self.registry).await?;

        // Get original tool list (before filtering) to check for special tools
        let original_tools = spot_agent.available_tools();
        let wants_invoke = self.wants_invoke_agent(&original_tools);
        let wants_list = self.wants_list_agents(&original_tools);

        // Get the tools this agent should have access to (filtered by settings)
        let tool_names = self.filter_tools(original_tools);
        let tools = tool_registry.tools_by_name(&tool_names);

        // Collect tool definitions and Arc references
        // We need to move these into the spawned task
        let mut tool_data: Vec<(ToolDefinition, Arc<dyn Tool + Send + Sync>)> =
            tools.into_iter().map(|t| (t.definition(), t)).collect();

        // Collect MCP tools from running servers (filtered by agent attachments)
        let mcp_tool_calls = self
            .collect_mcp_tools(mcp_manager, Some(spot_agent.name()))
            .await;
        tool_data.extend(mcp_tool_calls);

        // Prepare data for the spawned task
        let system_prompt = spot_agent.system_prompt();
        let model_name_owned = model_name.to_string();
        let db_path = self.db.path().to_path_buf();
        let bus = self.bus.clone(); // Clone bus for sub-agent visibility
        let tool_return_recorder = tool_return_recorder.clone();
        let (tx, rx) = mpsc::channel(32);

        // Log what we're sending to serdesAI
        match &prompt {
            UserContent::Text(t) => {
                debug!(text_len = t.len(), "Sending text prompt to serdesAI")
            }
            UserContent::Parts(parts) => {
                let image_count = parts
                    .iter()
                    .filter(|p| matches!(p, UserContentPart::Image { .. }))
                    .count();
                info!(
                    parts_count = parts.len(),
                    image_count, "Sending multimodal prompt to serdesAI"
                );
            }
        }

        debug!(tool_count = tool_data.len(), "Spawning streaming task");

        // Spawn a task that owns the agent and sends events through the channel
        tokio::spawn(async move {
            debug!("Streaming task started");

            let wrapped_model = ArcModel(model);

            // Build the serdesAI agent
            debug!("Building serdesAI agent");
            let mut builder = agent(wrapped_model)
                .system_prompt(system_prompt)
                .temperature(0.7)
                .max_tokens(4096);

            match tool_return_recorder {
                Some(recorder) => {
                    // Register tools with recording executors
                    for (def, tool) in tool_data {
                        debug!(tool_name = %def.name, "Registering tool");
                        builder = builder.tool_with_executor(
                            def,
                            RecordingToolExecutor::new(
                                ToolExecutorAdapter::new(tool),
                                recorder.clone(),
                            ),
                        );
                    }

                    // Add invoke_agent with custom executor (has database access)
                    if wants_invoke {
                        let invoke_executor = InvokeAgentExecutor {
                            db_path: db_path.clone(),
                            current_model: model_name_owned.clone(),
                            bus: bus.clone(), // Pass bus for sub-agent visibility
                        };
                        builder = builder.tool_with_executor(
                            InvokeAgentExecutor::definition(),
                            RecordingToolExecutor::new(invoke_executor, recorder.clone()),
                        );
                    }

                    // Add list_agents with custom executor
                    if wants_list {
                        builder = builder.tool_with_executor(
                            ListAgentsExecutor::definition(),
                            RecordingToolExecutor::new(
                                ListAgentsExecutor {
                                    db_path: db_path.clone(),
                                },
                                recorder.clone(),
                            ),
                        );
                    }
                }
                None => {
                    // Register tools with real executors
                    for (def, tool) in tool_data {
                        debug!(tool_name = %def.name, "Registering tool");
                        builder = builder.tool_with_executor(def, ToolExecutorAdapter::new(tool));
                    }

                    // Add invoke_agent with custom executor (has database access)
                    if wants_invoke {
                        let invoke_executor = InvokeAgentExecutor {
                            db_path: db_path.clone(),
                            current_model: model_name_owned.clone(),
                            bus: bus.clone(), // Pass bus for sub-agent visibility
                        };
                        builder = builder
                            .tool_with_executor(InvokeAgentExecutor::definition(), invoke_executor);
                    }

                    // Add list_agents with custom executor
                    if wants_list {
                        builder = builder.tool_with_executor(
                            ListAgentsExecutor::definition(),
                            ListAgentsExecutor {
                                db_path: db_path.clone(),
                            },
                        );
                    }
                }
            }

            let serdes_agent = builder.build();
            debug!("Agent built successfully");

            // Set up run options
            let history_len = message_history.as_ref().map(|h| h.len()).unwrap_or(0);
            debug!(history_messages = history_len, "Setting up run options");

            let options = match message_history {
                Some(history) => RunOptions::new().message_history(history),
                None => RunOptions::new(),
            };

            // Use real streaming from serdesAI
            debug!("Calling run_stream_with_options");

            match serdes_agent
                .run_stream_with_options(prompt, (), options)
                .await
            {
                Ok(mut stream) => {
                    debug!("Stream started, forwarding events");
                    let mut event_count = 0u32;

                    // Forward all events from the stream
                    while let Some(event_result) = stream.next().await {
                        event_count += 1;
                        match event_result {
                            Ok(event) => {
                                debug!(event_num = event_count, "Received stream event");
                                if tx.send(Ok(event)).await.is_err() {
                                    warn!("Receiver dropped, stopping stream");
                                    break;
                                }
                            }
                            Err(e) => {
                                let error_str = e.to_string();
                                error!(error = %error_str, "Stream error");

                                // Log common error patterns
                                if error_str.contains("status: 400") {
                                    error!("HTTP 400 Bad Request - likely invalid model name");
                                } else if error_str.contains("status: 401") {
                                    error!("HTTP 401 Unauthorized - token may be expired");
                                } else if error_str.contains("status: 404") {
                                    error!("HTTP 404 Not Found - model name may be invalid");
                                }

                                let _ = tx
                                    .send(Ok(StreamEvent::Error {
                                        message: error_str.clone(),
                                    }))
                                    .await;
                                let _ = tx.send(Err(ExecutorError::Execution(error_str))).await;
                                break;
                            }
                        }
                    }
                    debug!(total_events = event_count, "Stream completed");
                }
                Err(e) => {
                    let error_str = e.to_string();
                    error!(error = %error_str, "Failed to start stream");

                    // Check for common error patterns
                    if error_str.contains("status: 400") {
                        error!(
                            "HTTP 400 Bad Request - likely invalid model name or request format"
                        );
                    } else if error_str.contains("status: 401") {
                        error!("HTTP 401 Unauthorized - token may be expired or invalid");
                    } else if error_str.contains("status: 403") {
                        error!("HTTP 403 Forbidden - token may not have required permissions");
                    } else if error_str.contains("status: 404") {
                        error!("HTTP 404 Not Found - model name may be invalid");
                    } else if error_str.contains("status: 429") {
                        error!("HTTP 429 Rate Limited - too many requests");
                    } else if error_str.contains("status: 5") {
                        error!("HTTP 5xx Server Error - API issue");
                    }

                    // Log the full error body if present
                    if error_str.contains("body:") {
                        error!("API Error Body: {}", error_str);
                    }

                    // Send error event
                    let _ = tx
                        .send(Ok(StreamEvent::Error {
                            message: error_str.clone(),
                        }))
                        .await;
                    let _ = tx.send(Err(ExecutorError::Execution(error_str))).await;
                }
            }
            debug!("Streaming task exiting");
        });

        Ok(ExecutorStreamReceiver { rx })
    }
}

/// Result from agent execution.
#[derive(Debug, Clone)]
pub struct ExecutorResult {
    /// The agent's final text output.
    pub output: String,
    /// Full message history (for context continuation).
    pub messages: Vec<ModelRequest>,
    /// Unique run ID for tracing.
    pub run_id: String,
}

/// Receiver for streaming events from agent execution.
///
/// This wraps an mpsc receiver and provides a convenient interface
/// for consuming streaming events.
pub struct ExecutorStreamReceiver {
    rx: mpsc::Receiver<Result<StreamEvent, ExecutorError>>,
}

impl ExecutorStreamReceiver {
    /// Receive the next event from the stream.
    ///
    /// Returns `None` when the stream is complete.
    pub async fn recv(&mut self) -> Option<Result<StreamEvent, ExecutorError>> {
        self.rx.recv().await
    }
}

/// Get a model by name, handling custom endpoints, OAuth models, and standard models.
///
/// This function checks the model registry first for custom configurations,
/// then falls back to OAuth detection by prefix, and finally standard inference.
pub async fn get_model(
    db: &Database,
    model_name: &str,
    registry: &ModelRegistry,
) -> Result<Arc<dyn Model>, ExecutorError> {
    debug!(model_name = %model_name, "get_model called");

    // First, check if we have a custom config for this model in the registry
    if let Some(config) = registry.get(model_name) {
        debug!(
            model_name = %model_name,
            model_type = %config.model_type,
            has_custom_endpoint = config.custom_endpoint.is_some(),
            "Found model in registry"
        );

        // Handle custom endpoint models (e.g., from /add-model)
        if let Some(endpoint) = &config.custom_endpoint {
            debug!(
                endpoint_url = %endpoint.url,
                has_api_key = endpoint.api_key.is_some(),
                "Custom endpoint details"
            );
            debug!("Using custom endpoint for model: {}", model_name);

            // Resolve the API key from database or environment
            let api_key = if let Some(ref key_template) = endpoint.api_key {
                if key_template.starts_with('$') {
                    // It's an env var reference like $API_KEY or ${API_KEY}
                    let var_name = key_template
                        .trim_start_matches('$')
                        .trim_matches(|c| c == '{' || c == '}');
                    // Check database first, then environment
                    resolve_api_key(db, var_name).ok_or_else(|| {
                        ExecutorError::Config(format!(
                            "API key {} not found. Run /add_model to configure it, or set the environment variable.",
                            var_name
                        ))
                    })?
                } else {
                    // It's a literal key
                    key_template.clone()
                }
            } else {
                return Err(ExecutorError::Config(format!(
                    "Model {} has custom endpoint but no API key configured",
                    model_name
                )));
            };

            // Get the actual model ID to send to the API
            let model_id = config.model_id.as_deref().unwrap_or(model_name);

            // Create OpenAI-compatible model with custom endpoint
            let model = OpenAIChatModel::new(model_id, api_key).with_base_url(&endpoint.url);

            info!(
                model_name = %model_name,
                endpoint = %endpoint.url,
                "Custom endpoint model ready"
            );
            return Ok(Arc::new(model));
        }

        // Handle based on model type for non-custom-endpoint models
        match config.model_type {
            ModelType::ClaudeCode => {
                debug!("Detected Claude Code OAuth model from config");
                let model = auth::get_claude_code_model(db, model_name)
                    .await
                    .map_err(|e| ExecutorError::Auth(e.to_string()))?;
                return Ok(Arc::new(model));
            }
            ModelType::ChatgptOauth => {
                debug!("Detected ChatGPT OAuth model from config");
                let model = auth::get_chatgpt_model(db, model_name)
                    .await
                    .map_err(|e| ExecutorError::Auth(e.to_string()))?;
                return Ok(Arc::new(model));
            }
            // For other types, fall through to standard handling
            _ => {}
        }
    }

    // Legacy: Check for OAuth models by prefix (backward compatibility)
    if model_name.starts_with("chatgpt-") || model_name.starts_with("chatgpt_") {
        debug!("Detected ChatGPT OAuth model by prefix");
        let model = auth::get_chatgpt_model(db, model_name).await.map_err(|e| {
            error!(error = %e, "Failed to get ChatGPT model");
            ExecutorError::Auth(e.to_string())
        })?;
        info!(model_id = %model.identifier(), "ChatGPT OAuth model ready");
        return Ok(Arc::new(model));
    }

    if model_name.starts_with("claude-code-") || model_name.starts_with("claude_code_") {
        debug!("Detected Claude Code OAuth model by prefix");
        let model = auth::get_claude_code_model(db, model_name)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to get Claude Code model");
                ExecutorError::Auth(e.to_string())
            })?;
        info!(model_id = %model.identifier(), "Claude Code OAuth model ready");
        return Ok(Arc::new(model));
    }

    // Check if this looks like a custom model (provider:model format)
    // If so, it should have been in the registry - error out
    if model_name.contains(':') && !model_name.starts_with("claude-code") {
        warn!(
            model_name = %model_name,
            registry_count = registry.len(),
            "Custom model not found in registry"
        );
        return Err(ExecutorError::Config(format!(
            "Model '{}' not found in registry. Did you add it with /add-model? Try running /add-model again.",
            model_name
        )));
    }

    // Standard model inference (uses API keys from environment)
    debug!("Using API key model inference for: {}", model_name);
    let model = infer_model(model_name).map_err(|e| {
        error!(error = %e, "Failed to infer model");
        ExecutorError::Model(e.to_string())
    })?;

    info!(model_name = %model_name, "Model ready");
    Ok(model)
}

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

impl<'a> AgentExecutor<'a> {
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
        use crate::config::Settings;

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

/// Tool executor that calls MCP server tools.
///
/// Note: We use a raw pointer to McpManager because we can't easily
/// share Arc across async boundaries here. The pointer is valid for
/// the duration of the executor run.
struct McpToolExecutor {
    server_name: String,
    tool_name: String,
    mcp_manager_ptr: *const McpManager,
}

// Safety: The pointer is only used during a single executor run
// where the McpManager is guaranteed to outlive the tool executor.
unsafe impl Send for McpToolExecutor {}
unsafe impl Sync for McpToolExecutor {}

#[async_trait]
impl Tool for McpToolExecutor {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            self.tool_name.clone(),
            format!("MCP tool from {}", self.server_name),
        )
    }

    async fn call(&self, _ctx: &RunContext<()>, args: JsonValue) -> Result<ToolReturn, ToolError> {
        // Safety: The McpManager outlives this executor
        let manager = unsafe { &*self.mcp_manager_ptr };

        match manager
            .call_tool(&self.server_name, &self.tool_name, args)
            .await
        {
            Ok(result) => {
                // Convert MCP result to ToolReturn
                if result.is_error {
                    let error_msg = result
                        .content
                        .first()
                        .map(|c| match c {
                            serdes_ai_mcp::ToolResultContent::Text { text } => text.clone(),
                            _ => "MCP tool error".to_string(),
                        })
                        .unwrap_or_else(|| "Unknown error".to_string());
                    Ok(ToolReturn::error(error_msg))
                } else {
                    let text = result
                        .content
                        .into_iter()
                        .filter_map(|c| match c {
                            serdes_ai_mcp::ToolResultContent::Text { text } => Some(text),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(ToolReturn::text(text))
                }
            }
            Err(e) => Err(ToolError::ExecutionFailed {
                message: e.to_string(),
                retryable: false,
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Model error: {0}")]
    Model(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Configuration error: {0}")]
    Config(String),
}
