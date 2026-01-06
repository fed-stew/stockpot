//! Executors for sub-agent tools (invoke_agent and list_agents).
//!
//! These executors handle the special agent management tools:
//! - `InvokeAgentExecutor`: Invokes sub-agents with proper session management
//! - `ListAgentsExecutor`: Returns available agents to the caller

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use tracing::{debug, warn};

use serdes_ai_tools::{Tool, ToolDefinition, ToolError, ToolReturn};

use crate::agents::AgentManager;
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::MessageSender;
use crate::models::ModelRegistry;
use crate::session::SessionManager;
use crate::tools::agent_tools::InvokeAgentTool;
use crate::tools::SpotToolRegistry;

use super::AgentExecutor;

/// Executor for invoke_agent that has access to all required dependencies.
pub(super) struct InvokeAgentExecutor {
    db_path: PathBuf,
    current_model: String,
    /// Optional message bus for sub-agent event publishing.
    bus: Option<MessageSender>,
}

impl InvokeAgentExecutor {
    /// Create executor with message bus (preferred - enables visible sub-agent output).
    pub fn new(db: &Database, current_model: &str, bus: MessageSender) -> Self {
        Self {
            db_path: db.path().to_path_buf(),
            current_model: current_model.to_string(),
            bus: Some(bus),
        }
    }

    /// Create executor without message bus (legacy - sub-agent output not visible).
    pub fn new_legacy(db: &Database, current_model: &str) -> Self {
        Self {
            db_path: db.path().to_path_buf(),
            current_model: current_model.to_string(),
            bus: None,
        }
    }

    /// Create executor from a path (used in spawned tasks where Database isn't Send).
    pub fn new_with_path(
        db_path: PathBuf,
        current_model: &str,
        bus: Option<MessageSender>,
    ) -> Self {
        Self {
            db_path,
            current_model: current_model.to_string(),
            bus,
        }
    }

    pub fn definition() -> ToolDefinition {
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
        let bus = self.bus.clone();

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
            "session_id": result.1,
            "success": true
        })))
    }
}

/// Executor for list_agents that returns available agents.
pub(super) struct ListAgentsExecutor {
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl ListAgentsExecutor {
    pub fn new(db: &Database) -> Self {
        Self {
            db_path: db.path().to_path_buf(),
        }
    }

    /// Create executor from a path (used in spawned tasks where Database isn't Send).
    pub fn new_with_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn definition() -> ToolDefinition {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // =========================================================================
    // InvokeAgentExecutor Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_executor_new_with_path() {
        let db_path = PathBuf::from("/tmp/test.db");
        let executor = InvokeAgentExecutor::new_with_path(db_path.clone(), "gpt-4", None);

        assert_eq!(executor.db_path, db_path);
        assert_eq!(executor.current_model, "gpt-4");
        assert!(executor.bus.is_none());
    }

    #[test]
    fn test_invoke_agent_executor_new_with_path_and_bus() {
        use crate::messaging::MessageBus;

        let db_path = PathBuf::from("/tmp/test.db");
        let msg_bus = MessageBus::new();
        let bus = msg_bus.sender();
        let executor =
            InvokeAgentExecutor::new_with_path(db_path.clone(), "claude-3", Some(bus.clone()));

        assert_eq!(executor.db_path, db_path);
        assert_eq!(executor.current_model, "claude-3");
        assert!(executor.bus.is_some());
    }

    #[test]
    fn test_invoke_agent_executor_definition_valid() {
        let def = InvokeAgentExecutor::definition();

        assert_eq!(def.name, "invoke_agent");
        assert!(!def.description.is_empty());
        // Should have parameters defined (not null)
        assert!(!def.parameters().is_null());
    }

    #[test]
    fn test_invoke_agent_executor_definition_has_required_params() {
        let def = InvokeAgentExecutor::definition();

        // Check the parameters JSON has expected structure
        let params = def.parameters();
        if let Some(params_obj) = params.as_object() {
            // Tool definitions typically have "properties" and "required" fields
            assert!(
                params_obj.contains_key("properties") || params_obj.contains_key("type"),
                "parameters should have properties or type"
            );
        }
    }

    // =========================================================================
    // ListAgentsExecutor Tests
    // =========================================================================

    #[test]
    fn test_list_agents_executor_new_with_path() {
        let db_path = PathBuf::from("/tmp/test.db");
        let executor = ListAgentsExecutor::new_with_path(db_path.clone());

        assert_eq!(executor.db_path, db_path);
    }

    #[test]
    fn test_list_agents_executor_definition_valid() {
        let def = ListAgentsExecutor::definition();

        assert_eq!(def.name, "list_agents");
        assert!(!def.description.is_empty());
        assert!(def.description.contains("agents") || def.description.contains("available"));
    }

    #[test]
    fn test_list_agents_executor_definition_no_required_params() {
        let def = ListAgentsExecutor::definition();

        // list_agents doesn't require any parameters
        let params = def.parameters();
        if let Some(obj) = params.as_object() {
            if let Some(required) = obj.get("required") {
                if let Some(arr) = required.as_array() {
                    assert!(arr.is_empty(), "list_agents should have no required params");
                }
            }
        }
    }
}
