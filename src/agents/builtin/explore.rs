//! Explore Agent - Fast codebase navigation and file search specialist.

use crate::agents::{AgentCapabilities, AgentVisibility, SpotAgent};

/// Explore Agent - Fast, read-only codebase exploration and search
pub struct ExploreAgent;

impl SpotAgent for ExploreAgent {
    fn name(&self) -> &str {
        "explore"
    }

    fn display_name(&self) -> &str {
        "Explore Agent"
    }

    fn description(&self) -> &str {
        "Fast, read-only codebase exploration and search - finds code quickly without modifications"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/explore.md").to_string()
    }

    fn available_tools(&self) -> Vec<&str> {
        vec!["list_files", "read_file", "grep", "share_your_reasoning"]
    }

    fn visibility(&self) -> AgentVisibility {
        AgentVisibility::Sub
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::read_only()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explore_agent_basics() {
        let agent = ExploreAgent;

        assert_eq!(agent.name(), "explore");
        assert_eq!(agent.display_name(), "Explore Agent");
        assert!(!agent.description().is_empty());
    }

    #[test]
    fn test_explore_agent_is_read_only() {
        let agent = ExploreAgent;
        let caps = agent.capabilities();

        // Should have read access
        assert!(caps.file_read);

        // Should NOT have write/execute access
        assert!(!caps.file_write);
        assert!(!caps.shell);
        assert!(!caps.sub_agents);
        assert!(!caps.mcp);
    }

    #[test]
    fn test_explore_agent_minimal_tools() {
        let agent = ExploreAgent;
        let tools = agent.available_tools();

        // Should have exactly these read-only tools
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"grep"));
        assert!(tools.contains(&"share_your_reasoning"));
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn test_explore_system_prompt() {
        let agent = ExploreAgent;
        let prompt = agent.system_prompt();

        assert!(prompt.contains("READ-ONLY"));
        assert!(prompt.contains("exploration"));
        assert!(prompt.contains("Output Format"));
    }
}
