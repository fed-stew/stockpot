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
