//! Planning Agent - Strategic task breakdown.

use crate::agents::{AgentCapabilities, SpotAgent};

/// Planning Agent - Breaks down complex tasks into actionable steps
pub struct PlanningAgent;

impl SpotAgent for PlanningAgent {
    fn name(&self) -> &str {
        "planning-agent"
    }

    fn display_name(&self) -> &str {
        "Planning Agent"
    }

    fn description(&self) -> &str {
        "Breaks down complex coding tasks into clear, actionable steps"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/planning.md").to_string()
    }

    fn available_tools(&self) -> Vec<&str> {
        vec![
            "list_files",
            "read_file",
            "grep",
            "share_your_reasoning",
            "invoke_agent",
            "list_agents",
        ]
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::planning()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentVisibility;

    #[test]
    fn test_planning_name() {
        let agent = PlanningAgent;
        assert_eq!(agent.name(), "planning-agent");
    }

    #[test]
    fn test_planning_display_name() {
        let agent = PlanningAgent;
        assert_eq!(agent.display_name(), "Planning Agent");
    }

    #[test]
    fn test_planning_description_not_empty() {
        let agent = PlanningAgent;
        assert!(!agent.description().is_empty());
        assert!(
            agent.description().to_lowercase().contains("task")
                || agent.description().to_lowercase().contains("plan")
        );
    }

    #[test]
    fn test_planning_system_prompt_not_empty() {
        let agent = PlanningAgent;
        let prompt = agent.system_prompt();
        assert!(!prompt.is_empty());
    }

    #[test]
    fn test_planning_has_read_tools() {
        let agent = PlanningAgent;
        let tools = agent.available_tools();

        // Planning agent should have read-only file access
        assert!(tools.contains(&"list_files"), "Should have list_files");
        assert!(tools.contains(&"read_file"), "Should have read_file");
        assert!(tools.contains(&"grep"), "Should have grep");
    }

    #[test]
    fn test_planning_no_write_tools() {
        let agent = PlanningAgent;
        let tools = agent.available_tools();

        // Planning agent should NOT have write capabilities
        assert!(
            !tools.contains(&"edit_file"),
            "Planning should not have edit_file"
        );
        assert!(
            !tools.contains(&"delete_file"),
            "Planning should not have delete_file"
        );
        assert!(
            !tools.contains(&"run_shell_command"),
            "Planning should not have shell"
        );
    }

    #[test]
    fn test_planning_has_agent_collaboration() {
        let agent = PlanningAgent;
        let tools = agent.available_tools();

        // Planning agent can delegate to other agents
        assert!(tools.contains(&"invoke_agent"), "Should have invoke_agent");
        assert!(tools.contains(&"list_agents"), "Should have list_agents");
    }

    #[test]
    fn test_planning_capabilities() {
        let agent = PlanningAgent;
        let caps = agent.capabilities();

        // Planning has limited capabilities
        assert!(!caps.shell, "Planning should not have shell capability");
        assert!(
            !caps.file_write,
            "Planning should not have file_write capability"
        );
        assert!(caps.file_read, "Planning should have file_read capability");
        assert!(
            caps.sub_agents,
            "Planning should have sub_agents capability"
        );
        assert!(!caps.mcp, "Planning should not have mcp capability");
    }

    #[test]
    fn test_planning_default_visibility() {
        let agent = PlanningAgent;
        // Planning is a main agent, should be visible
        assert_eq!(agent.visibility(), AgentVisibility::Main);
    }

    #[test]
    fn test_planning_no_model_override() {
        let agent = PlanningAgent;
        assert!(
            agent.model_override().is_none(),
            "Planning should not force a specific model"
        );
    }

    #[test]
    fn test_planning_tool_count() {
        let agent = PlanningAgent;
        let tools = agent.available_tools();
        // Planning agent should have fewer tools than Stockpot
        assert!(tools.len() >= 4, "Planning should have at least 4 tools");
        assert!(tools.len() <= 10, "Planning shouldn't have too many tools");
    }

    #[test]
    fn test_planning_exact_tool_count() {
        let agent = PlanningAgent;
        let tools = agent.available_tools();
        assert_eq!(
            tools.len(),
            6,
            "Planning should have exactly 6 tools: {:?}",
            tools
        );
    }

    #[test]
    fn test_planning_has_share_reasoning() {
        let agent = PlanningAgent;
        let tools = agent.available_tools();
        assert!(
            tools.contains(&"share_your_reasoning"),
            "Planning should have share_your_reasoning"
        );
    }

    #[test]
    fn test_planning_capabilities_match_tools() {
        let agent = PlanningAgent;
        let caps = agent.capabilities();
        let tools = agent.available_tools();

        // file_read capability should match having read tools
        assert!(caps.file_read);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"list_files"));

        // sub_agents capability should match having agent tools
        assert!(caps.sub_agents);
        assert!(tools.contains(&"invoke_agent"));
        assert!(tools.contains(&"list_agents"));

        // no shell capability = no shell tool
        assert!(!caps.shell);
        assert!(!tools.contains(&"run_shell_command"));

        // no file_write capability = no write tools
        assert!(!caps.file_write);
        assert!(!tools.contains(&"edit_file"));
        assert!(!tools.contains(&"delete_file"));
    }

    #[test]
    fn test_planning_system_prompt_contains_planning_keywords() {
        let agent = PlanningAgent;
        let prompt = agent.system_prompt();
        // Prompt should contain planning-related content
        let lower = prompt.to_lowercase();
        assert!(
            lower.contains("plan") || lower.contains("task") || lower.contains("step"),
            "System prompt should mention planning concepts"
        );
    }

    #[test]
    fn test_planning_name_is_kebab_case() {
        let agent = PlanningAgent;
        let name = agent.name();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "Agent name should be kebab-case: {}",
            name
        );
    }
}
