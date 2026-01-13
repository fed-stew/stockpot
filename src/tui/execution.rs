//! Agent execution for TUI mode

use crate::agents::{AgentExecutor, AgentManager};
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::MessageSender;
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;
use serdes_ai_core::ModelRequest;
use std::sync::Arc;

/// Execute an agent with the given parameters
#[allow(clippy::too_many_arguments)]
pub async fn execute_agent(
    agent_name: String,
    prompt: String,
    history: Vec<ModelRequest>,
    model_name: String,
    db: Arc<Database>,
    agent_manager: Arc<AgentManager>,
    model_registry: Arc<ModelRegistry>,
    tool_registry: Arc<SpotToolRegistry>,
    mcp_manager: Arc<McpManager>,
    sender: MessageSender,
) {
    // Get the agent
    let agent = match agent_manager.get(&agent_name) {
        Some(agent) => agent,
        None => {
            let _ = sender.error(format!("Agent not found: {}", agent_name));
            return;
        }
    };

    // Create executor with references
    let mut executor = AgentExecutor::new(&db, &model_registry).with_bus(sender.clone());

    // Execute
    // Note: execute_with_bus signature:
    // agent: &dyn SpotAgent
    // model_name: &str
    // prompt: &str
    // history: Option<Vec<ModelRequest>>
    // tool_registry: &SpotToolRegistry
    // mcp_manager: &McpManager

    // We pass references where needed
    let result = executor
        .execute_with_bus(
            agent,
            &model_name,
            &prompt,
            Some(history),
            &tool_registry,
            &mcp_manager,
        )
        .await;

    if let Err(e) = result {
        let _ = sender.error(format!("Execution failed: {}", e));
    }
}
