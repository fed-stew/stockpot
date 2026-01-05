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
