//! Tool to Activity converter
//!
//! Converts stockpot's MessageBus events (Tool messages, Agent events, etc.)
//! to Activity types for the activity feed.
//!
//! Note: Batching of consecutive file reads is handled in app.rs, not here.
//! This converter just returns activities immediately.

use serde_json::Value;

use super::{Activity, DiffLine, FileAction};
use crate::messaging::{ToolMessage, ToolStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Converter
// ─────────────────────────────────────────────────────────────────────────────

/// Converts MessageBus events to Activities
#[derive(Debug, Default)]
pub struct ActivityConverter;

impl ActivityConverter {
    /// Create a new converter
    pub fn new() -> Self {
        Self
    }

    /// Process a tool message and return Activities.
    /// Batching of consecutive Explored activities is handled in app.rs.
    pub fn process_tool(&self, tool: &ToolMessage) -> Vec<Activity> {
        tracing::info!(
            "CONVERTER: tool='{}' status={:?} has_args={}",
            tool.tool_name,
            tool.status,
            tool.args.is_some()
        );

        // Some tools need to be processed at Executing (when args are available)
        // Others at Completed/Failed
        let result = match tool.tool_name.as_str() {
            // File exploration tools: process at Executing (args available)
            "read_file" | "cp_read_file" => {
                if tool.status == ToolStatus::Executing {
                    Self::handle_read_file(tool)
                } else {
                    vec![]
                }
            }
            "list_files" | "cp_list_files" => {
                if tool.status == ToolStatus::Executing {
                    Self::handle_list_files(tool)
                } else {
                    vec![]
                }
            }
            // These need Completed/Failed status (result available)
            "grep" | "cp_grep" => {
                if matches!(tool.status, ToolStatus::Completed | ToolStatus::Failed) {
                    Self::handle_grep(tool)
                } else {
                    vec![]
                }
            }
            "run_shell_command" | "cp_agent_run_shell_command" => {
                if matches!(tool.status, ToolStatus::Completed | ToolStatus::Failed) {
                    Self::handle_shell(tool)
                } else {
                    vec![]
                }
            }
            "edit_file" | "cp_edit_file" => {
                if matches!(tool.status, ToolStatus::Completed | ToolStatus::Failed) {
                    Self::handle_edit_file(tool)
                } else {
                    vec![]
                }
            }
            "delete_file" | "cp_delete_file" => {
                if matches!(tool.status, ToolStatus::Completed | ToolStatus::Failed) {
                    Self::handle_delete_file(tool)
                } else {
                    vec![]
                }
            }
            "invoke_agent" | "cp_invoke_agent" => {
                if matches!(tool.status, ToolStatus::Completed | ToolStatus::Failed) {
                    Self::handle_invoke_agent(tool)
                } else {
                    vec![]
                }
            }
            "share_your_reasoning" | "cp_agent_share_your_reasoning" => {
                // Reasoning: process at Executing (args available)
                if tool.status == ToolStatus::Executing {
                    Self::handle_reasoning(tool)
                } else {
                    vec![]
                }
            }
            _ => {
                if matches!(tool.status, ToolStatus::Completed | ToolStatus::Failed) {
                    Self::handle_generic_tool(tool)
                } else {
                    vec![]
                }
            }
        };

        result
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Tool Handlers
    // ─────────────────────────────────────────────────────────────────────────

    fn handle_read_file(tool: &ToolMessage) -> Vec<Activity> {
        let path = match extract_file_path(&tool.args) {
            Some(p) => p,
            None => return vec![],
        };
        vec![Activity::explored(vec![FileAction::Read(path)])]
    }

    fn handle_list_files(tool: &ToolMessage) -> Vec<Activity> {
        let dir = extract_directory(&tool.args).unwrap_or_else(|| ".".to_string());
        // Show as "Ran" activity so it doesn't merge with read_file operations
        vec![Activity::ran(format!("list {}", dir), vec![], None)]
    }

    fn handle_grep(tool: &ToolMessage) -> Vec<Activity> {
        let search = extract_string(&tool.args, "search_string").unwrap_or_default();
        let dir = extract_directory(&tool.args).unwrap_or_else(|| ".".to_string());

        // Grep is shown as a Ran activity (search command)
        let command = format!("grep '{}' {}", search, dir);
        let output = tool
            .result
            .as_ref()
            .map(|r| r.lines().take(10).map(String::from).collect())
            .unwrap_or_default();

        vec![Activity::ran(command, output, None)]
    }

    fn handle_shell(tool: &ToolMessage) -> Vec<Activity> {
        let command = extract_string(&tool.args, "command").unwrap_or_default();
        // Safety limit: cap output at 100 lines (display will further cap to 10)
        let output: Vec<String> = tool
            .result
            .as_ref()
            .map(|r| r.lines().take(100).map(String::from).collect())
            .unwrap_or_default();

        let notes = if tool.status == ToolStatus::Failed {
            tool.error.clone()
        } else {
            None
        };

        vec![Activity::ran(command, output, notes)]
    }

    fn handle_edit_file(tool: &ToolMessage) -> Vec<Activity> {
        let file_path = extract_file_path(&tool.args)
            .or_else(|| extract_payload_file_path(&tool.args))
            .unwrap_or_else(|| "unknown".to_string());

        // Parse diff from result
        let (additions, deletions, diff_lines) = tool
            .result
            .as_ref()
            .map(|r| parse_diff_output(r))
            .unwrap_or((0, 0, vec![]));

        vec![Activity::edited(
            file_path, additions, deletions, diff_lines,
        )]
    }

    fn handle_delete_file(tool: &ToolMessage) -> Vec<Activity> {
        let file_path = extract_file_path(&tool.args).unwrap_or_else(|| "unknown".to_string());
        // Show deletion as an edit with only deletions
        vec![Activity::edited(file_path, 0, 1, vec![])]
    }

    fn handle_invoke_agent(tool: &ToolMessage) -> Vec<Activity> {
        let agent_name = extract_string(&tool.args, "agent_name").unwrap_or_default();
        let display_name = agent_name.clone();

        let mut activity = Activity::nested_agent(&agent_name, &display_name);

        // If completed, mark it and add any response content
        if tool.status == ToolStatus::Completed {
            if let Activity::NestedAgent {
                ref mut completed,
                ref mut content,
                ..
            } = activity
            {
                *completed = true;
                if let Some(result) = &tool.result {
                    *content = result.clone();
                }
            }
        }

        vec![activity]
    }

    fn handle_reasoning(tool: &ToolMessage) -> Vec<Activity> {
        let reasoning = extract_string(&tool.args, "reasoning").unwrap_or_default();
        let next_steps = extract_string(&tool.args, "next_steps");

        let description = if let Some(steps) = next_steps {
            format!("{} → {}", reasoning, steps)
        } else {
            reasoning
        };

        vec![Activity::task(description)]
    }

    fn handle_generic_tool(tool: &ToolMessage) -> Vec<Activity> {
        // Generic fallback: show as a "Used" task
        let description = format!("Used {}", tool.tool_name);
        let mut activity = Activity::task(description);

        // Mark as completed if tool completed successfully
        if let Activity::Task {
            ref mut completed, ..
        } = activity
        {
            *completed = tool.status == ToolStatus::Completed;
        }

        vec![activity]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsing Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract file path from tool args
fn extract_file_path(args: &Option<Value>) -> Option<String> {
    args.as_ref()?.get("file_path")?.as_str().map(String::from)
}

/// Extract file path from nested payload (for edit_file)
fn extract_payload_file_path(args: &Option<Value>) -> Option<String> {
    args.as_ref()?
        .get("payload")?
        .get("file_path")?
        .as_str()
        .map(String::from)
}

/// Extract directory from tool args
fn extract_directory(args: &Option<Value>) -> Option<String> {
    args.as_ref()?
        .get("directory")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extract a string field from tool args
fn extract_string(args: &Option<Value>, field: &str) -> Option<String> {
    args.as_ref()?
        .get(field)
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Parse diff output into (additions, deletions, diff_lines)
fn parse_diff_output(result: &str) -> (i32, i32, Vec<DiffLine>) {
    let mut additions = 0i32;
    let mut deletions = 0i32;
    let mut diff_lines = Vec::new();
    let mut line_num = 1u32;

    for line in result.lines() {
        // Skip header lines
        if line.starts_with("---") || line.starts_with("+++") || line.starts_with("@@") {
            continue;
        }

        if let Some(content) = line.strip_prefix('+') {
            additions += 1;
            diff_lines.push(DiffLine::Added(line_num, content.to_string()));
            line_num += 1;
        } else if let Some(content) = line.strip_prefix('-') {
            deletions += 1;
            diff_lines.push(DiffLine::Removed(line_num, content.to_string()));
            // Don't increment line_num for removed lines
        } else if let Some(content) = line.strip_prefix(' ') {
            diff_lines.push(DiffLine::Context(line_num, content.to_string()));
            line_num += 1;
        } else if !line.is_empty() {
            // Treat as context if no prefix
            diff_lines.push(DiffLine::Context(line_num, line.to_string()));
            line_num += 1;
        }
    }

    (additions, deletions, diff_lines)
}

// ─────────────────────────────────────────────────────────────────────────────
// Message Conversion Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a UserMessage activity from text
pub fn user_message_activity(content: &str) -> Activity {
    Activity::user_message(content)
}

/// Create an AssistantMessage activity from text
pub fn assistant_message_activity(content: &str) -> Activity {
    Activity::assistant_message(content)
}

/// Create a Thinking activity from reasoning text
pub fn thinking_activity(content: &str) -> Activity {
    Activity::thinking(content)
}

/// Create a Streaming activity for live content
pub fn streaming_activity(title: &str) -> Activity {
    Activity::streaming(title, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_simple() {
        let diff = "+added line\n-removed line\n context line";
        let (add, del, lines) = parse_diff_output(diff);

        assert_eq!(add, 1);
        assert_eq!(del, 1);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_extract_file_path() {
        let args = serde_json::json!({
            "file_path": "src/main.rs"
        });

        assert_eq!(
            extract_file_path(&Some(args)),
            Some("src/main.rs".to_string())
        );
    }

    #[test]
    fn test_extract_directory() {
        let args = serde_json::json!({
            "directory": "src/"
        });

        assert_eq!(extract_directory(&Some(args)), Some("src/".to_string()));
    }

    #[test]
    fn test_read_file_returns_explored_at_executing() {
        let converter = ActivityConverter::new();

        // read_file creates activity at Executing (when args are available)
        let tool = ToolMessage {
            tool_name: "read_file".to_string(),
            status: ToolStatus::Executing,
            args: Some(serde_json::json!({ "file_path": "a.rs" })),
            ..Default::default()
        };

        let result = converter.process_tool(&tool);
        assert_eq!(result.len(), 1);

        if let Activity::Explored { actions, .. } = &result[0] {
            assert_eq!(actions.len(), 1);
            assert!(matches!(&actions[0], FileAction::Read(p) if p == "a.rs"));
        } else {
            panic!("Expected Explored activity");
        }
    }

    #[test]
    fn test_read_file_no_activity_at_completed() {
        let converter = ActivityConverter::new();

        // At Completed, args are None, so no activity
        let tool = ToolMessage {
            tool_name: "read_file".to_string(),
            status: ToolStatus::Completed,
            args: None,
            ..Default::default()
        };

        let result = converter.process_tool(&tool);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_shell_returns_ran_activity() {
        let converter = ActivityConverter::new();

        let shell_tool = ToolMessage {
            tool_name: "run_shell_command".to_string(),
            status: ToolStatus::Completed,
            args: Some(serde_json::json!({ "command": "ls" })),
            result: Some("file1\nfile2".to_string()),
            ..Default::default()
        };
        let result = converter.process_tool(&shell_tool);

        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Activity::Ran { .. }));
    }
}
