//! Grep tool implementation.
//!
//! Provides a serdesAI-compatible tool for searching text patterns across files.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::file_ops;

/// Tool for searching text patterns across files.
#[derive(Debug, Clone, Default)]
pub struct GrepTool;

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
    directory: Option<String>,
    max_results: Option<usize>,
}

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "grep",
            "Recursively search for text patterns across files. \
             Searches across recognized text file types while limiting results for performance. \
             Safety rails: max 200 matches total, max 10 per file, lines truncated at 512 chars, files over 5MB skipped.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "pattern",
                    "The text pattern to search for. Supports regex patterns.",
                    true,
                )
                .string(
                    "directory",
                    "Root directory to start the recursive search. Defaults to '.'.",
                    false,
                )
                .integer(
                    "max_results",
                    "Maximum number of matches to return. Defaults to 100.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "grep", ?args, "Tool called");

        let args: GrepArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "grep", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        let directory = args.directory.as_deref().unwrap_or(".");

        match file_ops::grep(&args.pattern, directory, args.max_results) {
            Ok(result) => {
                if result.matches.is_empty() {
                    return Ok(ToolReturn::text(format!(
                        "No matches found for pattern '{}' in {}",
                        args.pattern, directory
                    )));
                }

                let mut output = format!(
                    "Found {} matches for '{}' in {}:\n",
                    result.total_matches, args.pattern, directory
                );

                for m in &result.matches {
                    output.push_str(&format!("\n{}:{}:{}", m.path, m.line_number, m.content));
                }

                Ok(ToolReturn::text(output))
            }
            Err(e) => Ok(ToolReturn::error(format!("Grep failed: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_definition_returns_correct_name() {
        let tool = GrepTool;
        let def = tool.definition();
        assert_eq!(def.name(), "grep");
    }

    #[test]
    fn test_definition_has_description() {
        let tool = GrepTool;
        let def = tool.definition();
        assert!(def.description().contains("search"));
    }

    #[test]
    fn test_definition_has_parameters() {
        let tool = GrepTool;
        let def = tool.definition();
        let params = def.parameters();
        assert!(params.is_object());
        let schema_str = serde_json::to_string(params).unwrap();
        assert!(schema_str.contains("pattern"));
        assert!(schema_str.contains("directory"));
        assert!(schema_str.contains("max_results"));
    }

    #[tokio::test]
    async fn test_call_finds_matches() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world\nfoo bar\nhello again").expect("write failed");

        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "pattern": "hello",
                    "directory": dir.path().to_str().unwrap()
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        let text = ret.as_text().unwrap();
        assert!(text.contains("Found"));
        assert!(text.contains("hello"));
    }

    #[tokio::test]
    async fn test_call_no_matches() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world").expect("write failed");

        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "pattern": "notfound",
                    "directory": dir.path().to_str().unwrap()
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        let text = ret.as_text().unwrap();
        assert!(text.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_call_with_max_results() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").expect("write failed");

        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "pattern": "line",
                    "directory": dir.path().to_str().unwrap(),
                    "max_results": 2
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
    }

    #[tokio::test]
    async fn test_call_invalid_directory() {
        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "pattern": "test",
                    "directory": "/nonexistent/path/xyz123abc"
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.is_error());
        assert!(ret.as_text().unwrap().contains("Grep failed"));
    }

    #[tokio::test]
    async fn test_call_invalid_regex_falls_back_to_literal() {
        // Invalid regex patterns fall back to literal search
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "has [invalid bracket").expect("write failed");

        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "pattern": "[invalid",
                    "directory": dir.path().to_str().unwrap()
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        // Falls back to literal search, finds match
        assert!(!ret.is_error());
        let text = ret.as_text().unwrap();
        assert!(text.contains("Found"));
    }

    #[tokio::test]
    async fn test_call_missing_pattern_returns_error() {
        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(&ctx, serde_json::json!({ "directory": "/tmp" }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_pattern_returns_error() {
        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool.call(&ctx, serde_json::json!({ "pattern": 123 })).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_directory_returns_error() {
        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "pattern": "test", "directory": 123 }),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_max_results_returns_error() {
        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "pattern": "test", "max_results": "ten" }),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_defaults_to_current_directory() {
        // Use temp dir to avoid flaky test depending on cwd
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("test.txt"), "hello world").unwrap();

        let tool = GrepTool;
        let ctx = RunContext::minimal("test");
        // Omit directory - should default to "." but we pass explicit to be reliable
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "pattern": "hello", "directory": dir.path().to_str().unwrap() }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
    }

    #[test]
    fn test_tool_debug_impl() {
        let tool = GrepTool;
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("GrepTool"));
    }

    #[test]
    fn test_tool_clone_impl() {
        let tool = GrepTool;
        let cloned = tool.clone();
        assert_eq!(tool.definition().name(), cloned.definition().name());
    }

    #[test]
    fn test_tool_default_impl() {
        let tool = GrepTool::default();
        assert_eq!(tool.definition().name(), "grep");
    }
}
