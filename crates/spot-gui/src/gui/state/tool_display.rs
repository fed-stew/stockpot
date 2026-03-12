//! Tool call display formatting utilities
//!
//! Provides human-readable formatting for tool calls in the chat UI.

/// Safely truncate a string to at most `max_bytes` bytes, respecting UTF-8 character boundaries.
///
/// Returns a slice that ends at a valid char boundary, ensuring we never panic
/// on multi-byte characters like emojis.
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the last valid char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

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
                format!("{}...", safe_truncate(cmd, 57))
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

    #[test]
    fn test_safe_truncate_basic() {
        // Test that strings shorter than max_bytes are returned unchanged
        assert_eq!(safe_truncate("hello", 10), "hello");
        assert_eq!(safe_truncate("", 10), "");
        assert_eq!(safe_truncate("hello", 5), "hello");
    }

    #[test]
    fn test_safe_truncate_ascii() {
        // Test basic ASCII truncation
        assert_eq!(safe_truncate("hello world", 5), "hello");
        assert_eq!(safe_truncate("abcdefghij", 3), "abc");
    }

    #[test]
    fn test_safe_truncate_multibyte_chars() {
        // Test with emojis (4-byte UTF-8 characters)
        // "🧂" is 4 bytes, so truncating at byte 2 should give empty string
        assert_eq!(safe_truncate("🧂", 2), "");
        // Truncating at exactly 4 bytes should include the whole emoji
        assert_eq!(safe_truncate("🧂", 4), "🧂");
        // Test with emoji in the middle
        assert_eq!(safe_truncate("hello🧂world", 6), "hello");
        assert_eq!(safe_truncate("hello🧂world", 9), "hello🧂");
    }

    #[test]
    fn test_safe_truncate_command_with_emoji() {
        // This is the exact scenario that caused the crash!
        // A command with emojis that gets truncated at byte 57
        let cmd_with_emoji = "echo 'Adding some spice to the build process! 🧂🧂🧂 Let's go!'";
        // This should NOT panic
        let truncated = safe_truncate(cmd_with_emoji, 57);
        assert!(truncated.len() <= 57);
        assert!(truncated.is_char_boundary(truncated.len())); // Valid UTF-8
    }

    #[test]
    fn test_shell_command_with_emoji_no_panic() {
        // Integration test: verify the tool display handles emoji commands
        let cmd_with_emoji = "echo 'Adding some spice to the build process! 🧂🧂🧂 Let's go!'";
        let args = serde_json::json!({"command": cmd_with_emoji});
        // This should NOT panic!
        let info = get_tool_display_info("run_shell_command", &args);
        assert_eq!(info.verb, "Ran");
        assert!(info.subject.ends_with("..."));
        // Verify the subject is valid UTF-8 (it is, since we got here without panicking)
    }
}
