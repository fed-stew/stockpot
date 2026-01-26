//! Stockpot - The main assistant agent.

use crate::agents::{AgentCapabilities, SpotAgent};

/// Stockpot - Your AI coding companion 🍲
pub struct StockpotAgent;

impl SpotAgent for StockpotAgent {
    fn name(&self) -> &str {
        "stockpot"
    }

    fn display_name(&self) -> &str {
        "Coding Agent"
    }

    fn description(&self) -> &str {
        "Your AI coding companion - helps with all coding tasks"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/stockpot.md").to_string()
    }

    fn available_tools(&self) -> Vec<&str> {
        vec![
            "list_files",
            "read_file",
            "edit_file",
            "delete_file",
            "grep",
            "run_shell_command",
            "invoke_agent",
            "list_agents",
            // Process management tools
            "list_processes",
            "read_process_output",
            "kill_process",
        ]
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentVisibility;

    #[test]
    fn test_stockpot_name() {
        let agent = StockpotAgent;
        assert_eq!(agent.name(), "stockpot");
    }

    #[test]
    fn test_stockpot_display_name() {
        let agent = StockpotAgent;
        assert_eq!(agent.display_name(), "Coding Agent");
    }

    #[test]
    fn test_stockpot_description_not_empty() {
        let agent = StockpotAgent;
        assert!(!agent.description().is_empty());
        assert!(agent.description().contains("coding"));
    }

    #[test]
    fn test_stockpot_system_prompt_not_empty() {
        let agent = StockpotAgent;
        let prompt = agent.system_prompt();
        assert!(!prompt.is_empty());
        // Prompt should contain relevant keywords
        assert!(
            prompt.contains("Stockpot") || prompt.contains("code") || prompt.contains("assistant"),
            "System prompt should mention Stockpot or coding"
        );
    }

    #[test]
    fn test_stockpot_has_core_tools() {
        let agent = StockpotAgent;
        let tools = agent.available_tools();

        // Must have file operations
        assert!(tools.contains(&"list_files"), "Should have list_files");
        assert!(tools.contains(&"read_file"), "Should have read_file");
        assert!(tools.contains(&"edit_file"), "Should have edit_file");
        assert!(tools.contains(&"delete_file"), "Should have delete_file");

        // Must have search
        assert!(tools.contains(&"grep"), "Should have grep");

        // Must have shell
        assert!(
            tools.contains(&"run_shell_command"),
            "Should have run_shell_command"
        );

        // Must have agent collaboration
        assert!(tools.contains(&"invoke_agent"), "Should have invoke_agent");
        assert!(tools.contains(&"list_agents"), "Should have list_agents");
    }

    #[test]
    fn test_stockpot_has_full_capabilities() {
        let agent = StockpotAgent;
        let caps = agent.capabilities();

        assert!(caps.shell, "Stockpot should have shell capability");
        assert!(
            caps.file_write,
            "Stockpot should have file_write capability"
        );
        assert!(caps.file_read, "Stockpot should have file_read capability");
        assert!(
            caps.sub_agents,
            "Stockpot should have sub_agents capability"
        );
        assert!(caps.mcp, "Stockpot should have mcp capability");
    }

    #[test]
    fn test_stockpot_default_visibility() {
        let agent = StockpotAgent;
        // Default visibility should be Main (primary agent)
        assert_eq!(agent.visibility(), AgentVisibility::Main);
    }

    #[test]
    fn test_stockpot_no_model_override() {
        let agent = StockpotAgent;
        // Stockpot should not force a specific model
        assert!(
            agent.model_override().is_none(),
            "Stockpot should not have a model override"
        );
    }

    #[test]
    fn test_stockpot_tool_count() {
        let agent = StockpotAgent;
        let tools = agent.available_tools();
        // Should have a reasonable number of tools
        assert!(tools.len() >= 8, "Stockpot should have at least 8 tools");
        assert!(tools.len() <= 20, "Stockpot shouldn't have too many tools");
    }

    #[test]
    fn test_stockpot_exact_tool_count() {
        let agent = StockpotAgent;
        let tools = agent.available_tools();
        assert_eq!(
            tools.len(),
            11,
            "Stockpot should have exactly 11 tools: {:?}",
            tools
        );
    }

    #[test]
    fn test_stockpot_capabilities_match_tools() {
        let agent = StockpotAgent;
        let caps = agent.capabilities();
        let tools = agent.available_tools();

        // file_read capability should match having read tools
        assert!(caps.file_read);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"grep"));

        // file_write capability should match having write tools
        assert!(caps.file_write);
        assert!(tools.contains(&"edit_file"));
        assert!(tools.contains(&"delete_file"));

        // shell capability should match having shell tool
        assert!(caps.shell);
        assert!(tools.contains(&"run_shell_command"));

        // sub_agents capability should match having agent tools
        assert!(caps.sub_agents);
        assert!(tools.contains(&"invoke_agent"));
        assert!(tools.contains(&"list_agents"));
    }

    #[test]
    fn test_stockpot_name_is_lowercase() {
        let agent = StockpotAgent;
        let name = agent.name();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase()),
            "Agent name should be lowercase: {}",
            name
        );
    }

    #[test]
    fn test_stockpot_is_main_agent() {
        let agent = StockpotAgent;
        // Stockpot is the primary agent
        assert_eq!(agent.visibility(), AgentVisibility::Main);
    }

    #[test]
    fn test_stockpot_full_capabilities() {
        let caps = AgentCapabilities::full();

        // Full capabilities should have everything enabled
        assert!(caps.shell, "full() should have shell");
        assert!(caps.file_write, "full() should have file_write");
        assert!(caps.file_read, "full() should have file_read");
        assert!(caps.sub_agents, "full() should have sub_agents");
        assert!(caps.mcp, "full() should have mcp");
    }

    #[test]
    fn test_stockpot_has_more_tools_than_planning() {
        use crate::agents::builtin::PlanningAgent;

        let stockpot = StockpotAgent;
        let planning = PlanningAgent;

        let stockpot_tools = stockpot.available_tools();
        let planning_tools = planning.available_tools();

        assert!(
            stockpot_tools.len() > planning_tools.len(),
            "Stockpot ({}) should have more tools than Planning ({})",
            stockpot_tools.len(),
            planning_tools.len()
        );
    }

    #[test]
    fn test_stockpot_has_more_capabilities_than_planning() {
        use crate::agents::builtin::PlanningAgent;

        let stockpot_caps = StockpotAgent.capabilities();
        let planning_caps = PlanningAgent.capabilities();

        // Count enabled capabilities
        let stockpot_count = [
            stockpot_caps.shell,
            stockpot_caps.file_write,
            stockpot_caps.file_read,
            stockpot_caps.sub_agents,
            stockpot_caps.mcp,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        let planning_count = [
            planning_caps.shell,
            planning_caps.file_write,
            planning_caps.file_read,
            planning_caps.sub_agents,
            planning_caps.mcp,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        assert!(
            stockpot_count > planning_count,
            "Stockpot ({}) should have more capabilities than Planning ({})",
            stockpot_count,
            planning_count
        );
    }
}
