//! Reedline completion with Tab-triggered menu.
//!
//! Type "/" then Tab to see commands. Menu filters as you type.

mod completer;
mod context;
mod picker;
mod prompt;
mod reedline_factory;

pub use completer::SpotCompleter;
pub use context::{try_complete_input, CompletionContext};
pub use picker::{
    pick_agent_completion, pick_command, pick_mcp_server, pick_mcp_subcommand,
    pick_model_completion, pick_session_completion,
};
pub use prompt::{SpotHighlighter, SpotPrompt};
pub use reedline_factory::create_reedline;

/// All slash commands with descriptions
pub const COMMANDS: &[(&str, &str)] = &[
    ("/a", "Select agent"),
    ("/add-model", "Add custom model"),
    ("/agent", "Select agent"),
    ("/agents", "List agents"),
    ("/cd", "Change directory"),
    ("/chatgpt-auth", "ChatGPT OAuth login"),
    ("/claude-code-auth", "Claude Code OAuth"),
    ("/clear", "Clear screen"),
    ("/compact", "Compact message history"),
    ("/context", "Show context usage"),
    ("/delete-session", "Delete session"),
    ("/exit", "Exit"),
    ("/h", "Show help"),
    ("/help", "Show help"),
    ("/load", "Load session"),
    ("/m", "Select model"),
    ("/mcp", "MCP server management"),
    ("/model", "Select model"),
    ("/model_settings", "Model settings"),
    ("/models", "List available models"),
    ("/ms", "Model settings"),
    ("/new", "New conversation"),
    ("/pin", "Pin model to agent"),
    ("/pins", "List all agent pins"),
    ("/quit", "Exit"),
    ("/reasoning", "Set reasoning effort"),
    ("/resume", "Resume session"),
    ("/s", "Session info"),
    ("/save", "Save session"),
    ("/session", "Session info"),
    ("/sessions", "List sessions"),
    ("/set", "Configuration"),
    ("/show", "Show status"),
    ("/tools", "List tools"),
    ("/truncate", "Truncate history"),
    ("/unpin", "Unpin model"),
    ("/v", "Version info"),
    ("/verbosity", "Set verbosity"),
    ("/version", "Version info"),
    ("/yolo", "Toggle YOLO mode"),
];

/// MCP subcommands
pub const MCP_COMMANDS: &[&str] = &[
    "add",
    "disable",
    "enable",
    "help",
    "list",
    "remove",
    "restart",
    "start",
    "start-all",
    "status",
    "stop",
    "stop-all",
    "tools",
];

/// Check if a command is complete (exact match)
pub fn is_complete_command(input: &str) -> bool {
    COMMANDS.iter().any(|(cmd, _)| *cmd == input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_complete_command() {
        assert!(is_complete_command("/help"));
        assert!(is_complete_command("/model"));
        assert!(!is_complete_command("/hel"));
        assert!(!is_complete_command("/mod"));
    }

    #[test]
    fn test_commands_sorted() {
        assert!(COMMANDS.iter().any(|(c, _)| *c == "/help"));
        assert!(COMMANDS.iter().any(|(c, _)| *c == "/model"));
        assert!(COMMANDS.iter().any(|(c, _)| *c == "/exit"));
    }
}
