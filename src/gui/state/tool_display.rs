//! Tool call display formatting utilities
//!
//! Provides human-readable formatting for tool calls in the chat UI.

/// Format a tool call as a nice one-liner for display in chat
pub fn format_tool_call_display(name: &str, args: &serde_json::Value) -> String {
    match name {
        "list_files" => {
            let dir = args
                .get("directory")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let recursive = args
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let rec_str = if recursive { " (recursive)" } else { "" };
            format!("📂 `{}`{}", dir, rec_str)
        }
        "read_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("📄 `{}`", path)
        }
        "edit_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("✏️ `{}`", path)
        }
        "delete_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("🗑️ `{}`", path)
        }
        "grep" => {
            let pattern = args
                .get("pattern")
                .or(args.get("search_string"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let dir = args
                .get("directory")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            format!("🔍 `{}` in `{}`", pattern, dir)
        }
        "run_shell_command" | "agent_run_shell_command" => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let preview = if cmd.len() > 60 {
                format!("{}...", &cmd[..57])
            } else {
                cmd.to_string()
            };
            format!("💻 `{}`", preview)
        }
        "invoke_agent" => {
            let agent = args
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("🤖 → {}", agent)
        }
        "agent_share_your_reasoning" => "💭 reasoning...".to_string(),
        _ => {
            // For unknown tools, show name with wrench emoji
            format!("🔧 {}", name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tool_call_display_list_files() {
        let args = serde_json::json!({"directory": "src", "recursive": true});
        let display = format_tool_call_display("list_files", &args);
        assert_eq!(display, "📂 `src` (recursive)");

        let args_non_recursive = serde_json::json!({"directory": ".", "recursive": false});
        let display = format_tool_call_display("list_files", &args_non_recursive);
        assert_eq!(display, "📂 `.`");
    }

    #[test]
    fn test_format_tool_call_display_read_file() {
        let args = serde_json::json!({"file_path": "src/main.rs"});
        let display = format_tool_call_display("read_file", &args);
        assert_eq!(display, "📄 `src/main.rs`");
    }

    #[test]
    fn test_format_tool_call_display_edit_file() {
        let args = serde_json::json!({"file_path": "test.py"});
        let display = format_tool_call_display("edit_file", &args);
        assert_eq!(display, "✏️ `test.py`");
    }

    #[test]
    fn test_format_tool_call_display_grep() {
        let args = serde_json::json!({"search_string": "TODO", "directory": "src"});
        let display = format_tool_call_display("grep", &args);
        assert_eq!(display, "🔍 `TODO` in `src`");
    }

    #[test]
    fn test_format_tool_call_display_shell_command() {
        let args = serde_json::json!({"command": "cargo build"});
        let display = format_tool_call_display("run_shell_command", &args);
        assert_eq!(display, "💻 `cargo build`");

        // Test truncation for long commands
        let long_cmd = "a".repeat(100);
        let args_long = serde_json::json!({"command": long_cmd});
        let display = format_tool_call_display("agent_run_shell_command", &args_long);
        assert!(display.ends_with("...`"));
        assert!(display.len() < 70); // Should be truncated
    }

    #[test]
    fn test_format_tool_call_display_invoke_agent() {
        let args = serde_json::json!({"agent_name": "code-reviewer"});
        let display = format_tool_call_display("invoke_agent", &args);
        assert_eq!(display, "🤖 → code-reviewer");
    }

    #[test]
    fn test_format_tool_call_display_unknown_tool() {
        let args = serde_json::json!({});
        let display = format_tool_call_display("custom_tool", &args);
        assert_eq!(display, "🔧 custom_tool");
    }
}
