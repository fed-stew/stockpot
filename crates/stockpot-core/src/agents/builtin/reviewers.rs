//! Code reviewer agents.

use crate::agents::{AgentCapabilities, AgentVisibility, SpotAgent};

macro_rules! define_reviewer {
    ($struct_name:ident, $name:expr, $display:expr, $desc:expr, $prompt_file:expr) => {
        pub struct $struct_name;

        impl SpotAgent for $struct_name {
            fn name(&self) -> &str {
                $name
            }

            fn display_name(&self) -> &str {
                $display
            }

            fn description(&self) -> &str {
                $desc
            }

            fn system_prompt(&self) -> String {
                include_str!($prompt_file).to_string()
            }

            fn available_tools(&self) -> Vec<&str> {
                vec!["list_files", "read_file", "grep"]
            }

            fn capabilities(&self) -> AgentCapabilities {
                AgentCapabilities::read_only()
            }

            fn visibility(&self) -> AgentVisibility {
                AgentVisibility::Sub
            }
        }
    };
}

define_reviewer!(
    CodeReviewerAgent,
    "code-reviewer",
    "Code Reviewer",
    "Thorough code reviewer for any language - focusing on quality, security, and best practices",
    "prompts/code_reviewer.md"
);

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // CodeReviewerAgent Tests
    // =========================================================================

    #[test]
    fn test_code_reviewer_name() {
        let agent = CodeReviewerAgent;
        assert_eq!(agent.name(), "code-reviewer");
    }

    #[test]
    fn test_code_reviewer_display_name() {
        let agent = CodeReviewerAgent;
        assert_eq!(agent.display_name(), "Code Reviewer");
    }

    #[test]
    fn test_code_reviewer_description_not_empty() {
        let agent = CodeReviewerAgent;
        assert!(!agent.description().is_empty());
        assert!(
            agent.description().to_lowercase().contains("review")
                || agent.description().to_lowercase().contains("code")
        );
    }

    #[test]
    fn test_code_reviewer_system_prompt_not_empty() {
        let agent = CodeReviewerAgent;
        let prompt = agent.system_prompt();
        assert!(!prompt.is_empty());
    }

    #[test]
    fn test_code_reviewer_has_read_tools() {
        let agent = CodeReviewerAgent;
        let tools = agent.available_tools();

        // Reviewers should have read-only access
        assert!(tools.contains(&"list_files"), "Should have list_files");
        assert!(tools.contains(&"read_file"), "Should have read_file");
        assert!(tools.contains(&"grep"), "Should have grep");
    }

    #[test]
    fn test_code_reviewer_no_write_tools() {
        let agent = CodeReviewerAgent;
        let tools = agent.available_tools();

        // Reviewers should NOT have write capabilities
        assert!(
            !tools.contains(&"edit_file"),
            "Reviewer should not have edit_file"
        );
        assert!(
            !tools.contains(&"delete_file"),
            "Reviewer should not have delete_file"
        );
        assert!(
            !tools.contains(&"run_shell_command"),
            "Reviewer should not have shell"
        );
    }

    #[test]
    fn test_code_reviewer_no_agent_tools() {
        let agent = CodeReviewerAgent;
        let tools = agent.available_tools();

        // Reviewers should not invoke other agents
        assert!(
            !tools.contains(&"invoke_agent"),
            "Reviewer should not have invoke_agent"
        );
        assert!(
            !tools.contains(&"list_agents"),
            "Reviewer should not have list_agents"
        );
    }

    #[test]
    fn test_code_reviewer_read_only_capabilities() {
        let agent = CodeReviewerAgent;
        let caps = agent.capabilities();

        assert!(!caps.shell, "Reviewer should not have shell capability");
        assert!(
            !caps.file_write,
            "Reviewer should not have file_write capability"
        );
        assert!(caps.file_read, "Reviewer should have file_read capability");
        assert!(
            !caps.sub_agents,
            "Reviewer should not have sub_agents capability"
        );
        assert!(!caps.mcp, "Reviewer should not have mcp capability");
    }

    #[test]
    fn test_code_reviewer_sub_visibility() {
        let agent = CodeReviewerAgent;
        // Reviewers are sub-agents, not main agents
        assert_eq!(agent.visibility(), AgentVisibility::Sub);
    }

    #[test]
    fn test_code_reviewer_no_model_override() {
        let agent = CodeReviewerAgent;
        assert!(
            agent.model_override().is_none(),
            "Reviewer should not force a specific model"
        );
    }

    #[test]
    fn test_code_reviewer_tool_count() {
        let agent = CodeReviewerAgent;
        let tools = agent.available_tools();
        // Reviewers should have minimal tools
        assert_eq!(tools.len(), 3, "Reviewer should have exactly 3 tools");
    }

    // =========================================================================
    // AgentCapabilities::read_only Tests
    // =========================================================================

    #[test]
    fn test_read_only_capabilities() {
        let caps = AgentCapabilities::read_only();

        assert!(!caps.shell);
        assert!(!caps.file_write);
        assert!(caps.file_read);
        assert!(!caps.sub_agents);
        assert!(!caps.mcp);
    }

    // =========================================================================
    // Macro-generated consistency tests
    // =========================================================================

    #[test]
    fn test_code_reviewer_exact_tool_count() {
        let agent = CodeReviewerAgent;
        let tools = agent.available_tools();
        assert_eq!(
            tools.len(),
            3,
            "Reviewer should have exactly 3 tools: {:?}",
            tools
        );
    }

    #[test]
    fn test_code_reviewer_capabilities_match_tools() {
        let agent = CodeReviewerAgent;
        let caps = agent.capabilities();
        let tools = agent.available_tools();

        // file_read capability should match having read tools
        assert!(caps.file_read);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"grep"));

        // no shell capability = no shell tool
        assert!(!caps.shell);
        assert!(!tools.contains(&"run_shell_command"));

        // no file_write capability = no write tools
        assert!(!caps.file_write);
        assert!(!tools.contains(&"edit_file"));
        assert!(!tools.contains(&"delete_file"));

        // no sub_agents capability = no agent tools
        assert!(!caps.sub_agents);
        assert!(!tools.contains(&"invoke_agent"));
        assert!(!tools.contains(&"list_agents"));
    }

    #[test]
    fn test_code_reviewer_name_is_kebab_case() {
        let agent = CodeReviewerAgent;
        let name = agent.name();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "Agent name should be kebab-case: {}",
            name
        );
    }

    #[test]
    fn test_code_reviewer_system_prompt_contains_review_keywords() {
        let agent = CodeReviewerAgent;
        let prompt = agent.system_prompt();
        let lower = prompt.to_lowercase();
        assert!(
            lower.contains("review") || lower.contains("code") || lower.contains("quality"),
            "System prompt should mention review concepts"
        );
    }

    #[test]
    fn test_code_reviewer_description_matches_role() {
        let agent = CodeReviewerAgent;
        let desc = agent.description().to_lowercase();
        // Description should mention review or code
        assert!(
            desc.contains("review") || desc.contains("code"),
            "Description should describe reviewing role: {}",
            agent.description()
        );
    }

    // =========================================================================
    // AgentVisibility::Sub behavior tests
    // =========================================================================

    #[test]
    fn test_sub_visibility_is_not_main() {
        assert_ne!(AgentVisibility::Sub, AgentVisibility::Main);
    }

    #[test]
    fn test_sub_visibility_is_not_hidden() {
        assert_ne!(AgentVisibility::Sub, AgentVisibility::Hidden);
    }
}
