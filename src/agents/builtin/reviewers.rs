//! Code reviewer agents.

use crate::agents::{AgentCapabilities, SpotAgent};

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
                vec!["read_file", "list_files", "grep", "share_reasoning"]
            }

            fn capabilities(&self) -> AgentCapabilities {
                AgentCapabilities::read_only()
            }
        }
    };
}

define_reviewer!(
    CodeReviewerAgent,
    "code-reviewer",
    "Code Reviewer 🔍",
    "Thorough code reviewer for any language - focusing on quality, security, and best practices",
    "prompts/code_reviewer.md"
);
