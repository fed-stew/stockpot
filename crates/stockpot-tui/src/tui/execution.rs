//! Agent execution for TUI mode

use serdes_ai_core::ModelRequest;
use std::sync::Arc;
use stockpot_core::agents::{AgentExecutor, AgentManager, RetryHandler};
use stockpot_core::db::Database;
use stockpot_core::mcp::McpManager;
use stockpot_core::messaging::{HistoryUpdateMessage, Message, MessageSender};
use stockpot_core::models::ModelRegistry;
use stockpot_core::tools::SpotToolRegistry;

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
            sender.error(format!("Agent not found: {}", agent_name));
            return;
        }
    };

    // Create retry handler for automatic key rotation on 429s
    let retry_handler = RetryHandler::new(Arc::clone(&db));

    // Create executor with references and retry handler
    let executor = AgentExecutor::new(&db, &model_registry)
        .with_retry_handler(retry_handler)
        .with_bus(sender.clone());

    // Execute and get result with updated messages
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

    match result {
        Ok(exec_result) => {
            // Send updated history back to TUI
            if !exec_result.messages.is_empty() {
                let _ = sender.send(Message::HistoryUpdate(HistoryUpdateMessage {
                    messages: exec_result.messages,
                }));
            }
        }
        Err(e) => {
            sender.error(format!("Execution failed: {}", e));
        }
    }
}
