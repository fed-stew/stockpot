//! Planning Agent - Strategic task breakdown.

use crate::agents::{AgentCapabilities, SpotAgent};

/// Planning Agent - Breaks down complex tasks into actionable steps 📋
pub struct PlanningAgent;

impl SpotAgent for PlanningAgent {
    fn name(&self) -> &str {
        "planning-agent"
    }

    fn display_name(&self) -> &str {
        "Planning Agent 📋"
    }

    fn description(&self) -> &str {
        "Breaks down complex coding tasks into clear, actionable steps"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/planning.md").to_string()
    }

    fn available_tools(&self) -> Vec<&str> {
        vec![
            "read_file",
            "list_files",
            "grep",
            "invoke_agent",
            "list_agents",
            "share_reasoning",
        ]
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::planning()
    }
}
