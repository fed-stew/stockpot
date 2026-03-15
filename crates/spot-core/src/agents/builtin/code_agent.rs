//! Code Agent - General-purpose code generation and modification.

use crate::agents::{AgentCapabilities, AgentVisibility, SpotAgent};

/// Code Agent - Reads, writes, and modifies code
pub struct CodeAgent;

impl SpotAgent for CodeAgent {
    fn name(&self) -> &str {
        "code-agent"
    }

    fn display_name(&self) -> &str {
        "Code Agent"
    }

    fn description(&self) -> &str {
        "General-purpose code agent that reads, writes, and modifies files, searches codebases, and executes shell commands"
    }

    fn system_prompt(&self) -> String {
        include_str!("prompts/code_agent.md").to_string()
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

    fn visibility(&self) -> AgentVisibility {
        AgentVisibility::Sub
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_agent_name() {
        let agent = CodeAgent;
        assert_eq!(agent.name(), "code-agent");
    }

    #[test]
    fn test_code_agent_display_name() {
        let agent = CodeAgent;
        assert_eq!(agent.display_name(), "Code Agent");
    }

    #[test]
    fn test_code_agent_description_not_empty() {
        let agent = CodeAgent;
        assert!(!agent.description().is_empty());
    }

    #[test]
    fn test_code_agent_system_prompt_not_empty() {
        let agent = CodeAgent;
        let prompt = agent.system_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.contains("code") || prompt.contains("tool") || prompt.contains("file"),
            "System prompt should mention coding concepts"
        );
    }

    #[test]
    fn test_code_agent_has_full_capabilities() {
        let agent = CodeAgent;
        let caps = agent.capabilities();

        assert!(caps.shell, "Code Agent should have shell capability");
        assert!(
            caps.file_write,
            "Code Agent should have file_write capability"
        );
        assert!(
            caps.file_read,
            "Code Agent should have file_read capability"
        );
        assert!(
            caps.sub_agents,
            "Code Agent should have sub_agents capability"
        );
        assert!(caps.mcp, "Code Agent should have mcp capability");
    }

    #[test]
    fn test_code_agent_has_core_tools() {
        let agent = CodeAgent;
        let tools = agent.available_tools();

        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"edit_file"));
        assert!(tools.contains(&"delete_file"));
        assert!(tools.contains(&"grep"));
        assert!(tools.contains(&"run_shell_command"));
        assert!(tools.contains(&"invoke_agent"));
        assert!(tools.contains(&"list_agents"));
    }

    #[test]
    fn test_code_agent_has_process_tools() {
        let agent = CodeAgent;
        let tools = agent.available_tools();

        assert!(tools.contains(&"list_processes"));
        assert!(tools.contains(&"read_process_output"));
        assert!(tools.contains(&"kill_process"));
    }

    #[test]
    fn test_code_agent_exact_tool_count() {
        let agent = CodeAgent;
        let tools = agent.available_tools();
        assert_eq!(
            tools.len(),
            11,
            "Code Agent should have exactly 11 tools: {:?}",
            tools
        );
    }

    #[test]
    fn test_code_agent_is_sub_agent() {
        let agent = CodeAgent;
        assert_eq!(agent.visibility(), AgentVisibility::Sub);
    }

    #[test]
    fn test_code_agent_no_model_override() {
        let agent = CodeAgent;
        assert!(agent.model_override().is_none());
    }

    #[test]
    fn test_code_agent_name_is_kebab_case() {
        let agent = CodeAgent;
        let name = agent.name();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "Agent name should be kebab-case: {}",
            name
        );
    }
}
