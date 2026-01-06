//! EditFile tool implementation.
//!
//! Provides a serdesAI-compatible tool for creating or editing files.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::file_ops;

/// Tool for creating or editing files.
#[derive(Debug, Clone, Default)]
pub struct EditFileTool;

#[derive(Debug, Deserialize)]
struct EditFileArgs {
    file_path: String,
    content: String,
    #[serde(default)]
    create_directories: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "edit_file",
            "Create or overwrite a file with the provided content. \
             Supports creating parent directories if they don't exist.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string("file_path", "Path to the file to create or edit.", true)
                .string("content", "The full content to write to the file.", true)
                .boolean(
                    "create_directories",
                    "Whether to create parent directories if they don't exist. Defaults to false.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "edit_file", ?args, "Tool called");

        let args: EditFileArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "edit_file", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        match file_ops::write_file(&args.file_path, &args.content, args.create_directories) {
            Ok(()) => {
                let line_count = args.content.lines().count();
                let byte_count = args.content.len();
                Ok(ToolReturn::text(format!(
                    "Successfully wrote {} lines ({} bytes) to {}",
                    line_count, byte_count, args.file_path
                )))
            }
            Err(e) => Ok(ToolReturn::error(format!("Failed to write file: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_definition_returns_correct_name() {
        let tool = EditFileTool;
        let def = tool.definition();
        assert_eq!(def.name(), "edit_file");
    }

    #[test]
    fn test_definition_has_description() {
        let tool = EditFileTool;
        let def = tool.definition();
        assert!(def.description().contains("Create"));
    }

    #[test]
    fn test_definition_has_parameters() {
        let tool = EditFileTool;
        let def = tool.definition();
        let params = def.parameters();
        assert!(params.is_object());
        let schema_str = serde_json::to_string(params).unwrap();
        assert!(schema_str.contains("file_path"));
        assert!(schema_str.contains("content"));
        assert!(schema_str.contains("create_directories"));
    }

    #[tokio::test]
    async fn test_call_creates_new_file() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("new_file.txt");

        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "hello world"
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_call_overwrites_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("existing.txt");
        fs::write(&file_path, "old content").expect("write failed");

        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "new content"
                }),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_call_reports_line_and_byte_count() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("count.txt");

        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "line1\nline2\nline3"
                }),
            )
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("3 lines"));
        assert!(text.contains("17 bytes"));
    }

    #[tokio::test]
    async fn test_call_create_directories_true() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("subdir/nested/file.txt");

        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "nested content",
                    "create_directories": true
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_call_create_directories_false_fails() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("nonexistent/file.txt");

        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "content",
                    "create_directories": false
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.is_error());
        assert!(ret.as_text().unwrap().contains("Failed to write file"));
    }

    #[tokio::test]
    async fn test_call_missing_file_path_returns_error() {
        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(&ctx, serde_json::json!({ "content": "hello" }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_missing_content_returns_error() {
        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(&ctx, serde_json::json!({ "file_path": "/tmp/test.txt" }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_file_path_returns_error() {
        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "file_path": 123, "content": "hello" }),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_content_returns_error() {
        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "file_path": "/tmp/test.txt", "content": 123 }),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_empty_content() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("empty.txt");

        let tool = EditFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": ""
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "");
    }

    #[test]
    fn test_tool_debug_impl() {
        let tool = EditFileTool;
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("EditFileTool"));
    }

    #[test]
    fn test_tool_clone_impl() {
        let tool = EditFileTool;
        let cloned = tool.clone();
        assert_eq!(tool.definition().name(), cloned.definition().name());
    }

    #[test]
    fn test_tool_default_impl() {
        let tool = EditFileTool::default();
        assert_eq!(tool.definition().name(), "edit_file");
    }
}
