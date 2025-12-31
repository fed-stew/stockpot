//! Stockpot - The main assistant agent.

use crate::agents::{AgentCapabilities, SpotAgent};

/// Stockpot - Your AI coding companion 🍲
pub struct StockpotAgent;

impl SpotAgent for StockpotAgent {
    fn name(&self) -> &str {
        "stockpot"
    }

    fn display_name(&self) -> &str {
        "stockpot 🍲"
    }

    fn description(&self) -> &str {
        "Your AI coding companion - helps with all coding tasks"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/stockpot.md").to_string()
    }

    fn available_tools(&self) -> Vec<&str> {
        vec![
            "read_file",
            "write_file",
            "list_files",
            "grep",
            "run_command",
            "apply_diff",
            "invoke_agent",
            "list_agents",
            "share_reasoning",
        ]
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::full()
    }
}
