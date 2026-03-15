//! Spot - The main orchestrator agent.

use crate::agents::{AgentCapabilities, SpotAgent};

/// Spot - Main orchestrator that triages, plans, and delegates
pub struct SpotMainAgent;

impl SpotAgent for SpotMainAgent {
    fn name(&self) -> &str {
        "spot"
    }

    fn display_name(&self) -> &str {
        "Spot"
    }

    fn description(&self) -> &str {
        "Main orchestrator - triages tasks, plans, and delegates to specialized agents"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/spot.md").to_string()
    }

    fn available_tools(&self) -> Vec<&str> {
        vec![
            "list_files",
            "read_file",
            "grep",
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
    fn test_spot_name() {
        let agent = SpotMainAgent;
        assert_eq!(agent.name(), "spot");
    }

    #[test]
    fn test_spot_display_name() {
        let agent = SpotMainAgent;
        assert_eq!(agent.display_name(), "Spot");
    }

    #[test]
    fn test_spot_description_not_empty() {
        let agent = SpotMainAgent;
        assert!(!agent.description().is_empty());
        assert!(agent.description().contains("orchestrator"));
    }

    #[test]
    fn test_spot_system_prompt_not_empty() {
        let agent = SpotMainAgent;
        let prompt = agent.system_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.contains("Spot") || prompt.contains("orchestrator") || prompt.contains("triage"),
            "System prompt should mention Spot or orchestration"
        );
    }

    #[test]
    fn test_spot_has_read_and_delegation_tools() {
        let agent = SpotMainAgent;
        let tools = agent.available_tools();

        assert!(tools.contains(&"list_files"), "Should have list_files");
        assert!(tools.contains(&"read_file"), "Should have read_file");
        assert!(tools.contains(&"grep"), "Should have grep");
        assert!(tools.contains(&"invoke_agent"), "Should have invoke_agent");
        assert!(tools.contains(&"list_agents"), "Should have list_agents");
    }

    #[test]
    fn test_spot_no_write_tools() {
        let agent = SpotMainAgent;
        let tools = agent.available_tools();

        assert!(
            !tools.contains(&"edit_file"),
            "Spot should not have edit_file"
        );
        assert!(
            !tools.contains(&"delete_file"),
            "Spot should not have delete_file"
        );
        assert!(
            !tools.contains(&"run_shell_command"),
            "Spot should not have shell"
        );
    }

    #[test]
    fn test_spot_exact_tool_count() {
        let agent = SpotMainAgent;
        let tools = agent.available_tools();
        assert_eq!(
            tools.len(),
            5,
            "Spot should have exactly 5 tools: {:?}",
            tools
        );
    }

    #[test]
    fn test_spot_planning_capabilities() {
        let agent = SpotMainAgent;
        let caps = agent.capabilities();

        assert!(!caps.shell, "Spot should not have shell capability");
        assert!(
            !caps.file_write,
            "Spot should not have file_write capability"
        );
        assert!(caps.file_read, "Spot should have file_read capability");
        assert!(caps.sub_agents, "Spot should have sub_agents capability");
        assert!(!caps.mcp, "Spot should not have mcp capability");
    }

    #[test]
    fn test_spot_default_visibility() {
        let agent = SpotMainAgent;
        assert_eq!(agent.visibility(), AgentVisibility::Main);
    }

    #[test]
    fn test_spot_no_model_override() {
        let agent = SpotMainAgent;
        assert!(
            agent.model_override().is_none(),
            "Spot should not have a model override"
        );
    }

    #[test]
    fn test_spot_name_is_lowercase() {
        let agent = SpotMainAgent;
        let name = agent.name();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase()),
            "Agent name should be lowercase: {}",
            name
        );
    }

    #[test]
    fn test_spot_is_main_agent() {
        let agent = SpotMainAgent;
        assert_eq!(agent.visibility(), AgentVisibility::Main);
    }

    #[test]
    fn test_spot_has_fewer_tools_than_code_agent() {
        use crate::agents::builtin::CodeAgent;

        let spot = SpotMainAgent;
        let code = CodeAgent;

        let spot_tools = spot.available_tools();
        let code_tools = code.available_tools();

        assert!(
            spot_tools.len() < code_tools.len(),
            "Spot ({}) should have fewer tools than Code Agent ({})",
            spot_tools.len(),
            code_tools.len()
        );
    }

    #[test]
    fn test_code_agent_has_more_capabilities_than_spot() {
        use crate::agents::builtin::CodeAgent;

        let spot_caps = SpotMainAgent.capabilities();
        let code_caps = CodeAgent.capabilities();

        let spot_count = [
            spot_caps.shell,
            spot_caps.file_write,
            spot_caps.file_read,
            spot_caps.sub_agents,
            spot_caps.mcp,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        let code_count = [
            code_caps.shell,
            code_caps.file_write,
            code_caps.file_read,
            code_caps.sub_agents,
            code_caps.mcp,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        assert!(
            code_count > spot_count,
            "Code Agent ({}) should have more capabilities than Spot ({})",
            code_count,
            spot_count
        );
    }
}
