//! DeleteFile tool implementation.
//!
//! Provides a serdesAI-compatible tool for deleting files.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

/// Tool for deleting files.
#[derive(Debug, Clone, Default)]
pub struct DeleteFileTool;

#[derive(Debug, Deserialize)]
struct DeleteFileArgs {
    file_path: String,
}

#[async_trait]
impl Tool for DeleteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "delete_file",
            "Safely delete a file. Will fail if the path is a directory.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string("file_path", "Path to the file to delete.", true)
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "delete_file", ?args, "Tool called");

        let args: DeleteFileArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "delete_file", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        let path = std::path::Path::new(&args.file_path);

        if !path.exists() {
            return Ok(ToolReturn::error(format!(
                "File not found: {}",
                args.file_path
            )));
        }

        if path.is_dir() {
            return Ok(ToolReturn::error(format!(
                "Cannot delete directory with this tool: {}",
                args.file_path
            )));
        }

        match std::fs::remove_file(path) {
            Ok(()) => Ok(ToolReturn::text(format!(
                "Successfully deleted: {}",
                args.file_path
            ))),
            Err(e) => Ok(ToolReturn::error(format!("Failed to delete file: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_definition_returns_correct_name() {
        let tool = DeleteFileTool;
        let def = tool.definition();
        assert_eq!(def.name(), "delete_file");
    }

    #[test]
    fn test_definition_has_description() {
        let tool = DeleteFileTool;
        let def = tool.definition();
        assert!(def.description().contains("delete"));
    }

    #[test]
    fn test_definition_has_parameters() {
        let tool = DeleteFileTool;
        let def = tool.definition();
        let params = def.parameters();
        assert!(params.is_object());
        let schema_str = serde_json::to_string(params).unwrap();
        assert!(schema_str.contains("file_path"));
    }

    #[tokio::test]
    async fn test_call_deletes_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("to_delete.txt");
        fs::write(&file_path, "content").expect("write failed");
        assert!(file_path.exists());

        let tool = DeleteFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "file_path": file_path.to_str().unwrap() }),
            )
            .await;

        assert!(result.is_ok());
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_call_file_not_found_returns_error() {
        let tool = DeleteFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "file_path": "/nonexistent/path/file.txt" }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.is_error());
        assert!(ret.as_text().unwrap().contains("File not found"));
    }

    #[tokio::test]
    async fn test_call_is_directory_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).expect("mkdir failed");

        let tool = DeleteFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "file_path": subdir.to_str().unwrap() }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.is_error());
        assert!(ret.as_text().unwrap().contains("Cannot delete directory"));
    }

    #[tokio::test]
    async fn test_call_missing_file_path_returns_error() {
        let tool = DeleteFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool.call(&ctx, serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_file_path_returns_error() {
        let tool = DeleteFileTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(&ctx, serde_json::json!({ "file_path": 123 }))
            .await;
        assert!(result.is_err());
    }
}
