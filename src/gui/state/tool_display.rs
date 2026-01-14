//! Tool call display formatting utilities
//!
//! Provides human-readable formatting for tool calls in the chat UI.

/// Structured tool call display info for styled rendering
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDisplayInfo {
    /// The action verb (e.g., "Edited", "Read", "Searched")
    pub verb: String,
    /// The subject/target (e.g., file path, search pattern)
    pub subject: String,
}

impl ToolDisplayInfo {
    pub fn new(verb: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            verb: verb.into(),
            subject: subject.into(),
        }
    }
}

/// Get structured display info for a tool call
pub fn get_tool_display_info(name: &str, args: &serde_json::Value) -> ToolDisplayInfo {
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
            ToolDisplayInfo::new("Listed", format!("{}{}", dir, rec_str))
        }
        "read_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Read", path)
        }
        "edit_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Edited", path)
        }
        "delete_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Deleted", path)
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
            ToolDisplayInfo::new("Searched", format!("'{}' in {}", pattern, dir))
        }
        "run_shell_command" | "agent_run_shell_command" => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let preview = if cmd.len() > 60 {
                format!("{}...", &cmd[..57])
            } else {
                cmd.to_string()
            };
            ToolDisplayInfo::new("Ran", preview)
        }
        "invoke_agent" => {
            let agent = args
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Invoked", agent)
        }
        _ => {
            // For unknown tools, use the tool name as the verb
            ToolDisplayInfo::new(name, "")
        }
    }
}

/// Format a tool call as a simple string (legacy, for markdown embedding)
pub fn format_tool_call_display(name: &str, args: &serde_json::Value) -> String {
    let info = get_tool_display_info(name, args);
    if info.subject.is_empty() {
        format!("• {} ", info.verb)
    } else {
        format!("• {}  {}", info.verb, info.subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_display_info_list_files() {
        let args = serde_json::json!({"directory": "src", "recursive": true});
        let info = get_tool_display_info("list_files", &args);
        assert_eq!(info, ToolDisplayInfo::new("Listed", "src (recursive)"));

        let args_non_recursive = serde_json::json!({"directory": ".", "recursive": false});
        let info = get_tool_display_info("list_files", &args_non_recursive);
        assert_eq!(info, ToolDisplayInfo::new("Listed", "."));
    }

    #[test]
    fn test_get_tool_display_info_read_file() {
        let args = serde_json::json!({"file_path": "src/main.rs"});
        let info = get_tool_display_info("read_file", &args);
        assert_eq!(info, ToolDisplayInfo::new("Read", "src/main.rs"));
    }

    #[test]
    fn test_get_tool_display_info_edit_file() {
        let args = serde_json::json!({"file_path": "test.py"});
        let info = get_tool_display_info("edit_file", &args);
        assert_eq!(info, ToolDisplayInfo::new("Edited", "test.py"));
    }

    #[test]
    fn test_get_tool_display_info_delete_file() {
        let args = serde_json::json!({"file_path": "old.txt"});
        let info = get_tool_display_info("delete_file", &args);
        assert_eq!(info, ToolDisplayInfo::new("Deleted", "old.txt"));
    }

    #[test]
    fn test_get_tool_display_info_grep() {
        let args = serde_json::json!({"search_string": "TODO", "directory": "src"});
        let info = get_tool_display_info("grep", &args);
        assert_eq!(info, ToolDisplayInfo::new("Searched", "'TODO' in src"));
    }

    #[test]
    fn test_get_tool_display_info_shell_command() {
        let args = serde_json::json!({"command": "cargo build"});
        let info = get_tool_display_info("run_shell_command", &args);
        assert_eq!(info, ToolDisplayInfo::new("Ran", "cargo build"));

        // Test truncation for long commands
        let long_cmd = "a".repeat(100);
        let args_long = serde_json::json!({"command": long_cmd});
        let info = get_tool_display_info("agent_run_shell_command", &args_long);
        assert_eq!(info.verb, "Ran");
        assert!(info.subject.ends_with("..."));
        assert!(info.subject.len() <= 60); // Should be truncated
    }

    #[test]
    fn test_get_tool_display_info_invoke_agent() {
        let args = serde_json::json!({"agent_name": "code-reviewer"});
        let info = get_tool_display_info("invoke_agent", &args);
        assert_eq!(info, ToolDisplayInfo::new("Invoked", "code-reviewer"));
    }

    #[test]
    fn test_get_tool_display_info_unknown_tool() {
        let args = serde_json::json!({});
        let info = get_tool_display_info("custom_tool", &args);
        assert_eq!(info, ToolDisplayInfo::new("custom_tool", ""));
    }

    #[test]
    fn test_format_tool_call_display_legacy_wrapper() {
        // Test the legacy wrapper for backward compatibility
        let args = serde_json::json!({"file_path": "test.rs"});
        let display = format_tool_call_display("read_file", &args);
        assert_eq!(display, "• Read  test.rs");

        // Test with unknown tool (empty subject)
        let args_empty = serde_json::json!({});
        let display = format_tool_call_display("some_unknown_tool", &args_empty);
        assert_eq!(display, "• some_unknown_tool ");
    }
}
