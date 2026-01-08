//! ReadFile tool implementation.
//!
//! Provides a serdesAI-compatible tool for reading file contents.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::file_ops::{self, FileError};

/// Tool for reading file contents.
#[derive(Debug, Clone, Default)]
pub struct ReadFileTool;

#[derive(Debug, Deserialize)]
struct ReadFileArgs {
    file_path: String,
    start_line: Option<usize>,
    num_lines: Option<usize>,
}

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "read_file",
            "Read file contents with optional line-range selection. \
             Protects against reading excessively large files that could \
             overwhelm the context window.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "file_path",
                    "Path to the file to read. Can be relative or absolute.",
                    true,
                )
                .integer(
                    "start_line",
                    "Starting line number for partial reads (1-based indexing). \
                     If specified, num_lines should also be provided.",
                    false,
                )
                .integer(
                    "num_lines",
                    "Number of lines to read starting from start_line.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "read_file", ?args, "Tool called");

        let args: ReadFileArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "read_file", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        match file_ops::read_file(
            &args.file_path,
            args.start_line,
            args.num_lines,
            None, // use default max size
        ) {
            Ok(result) => {
                let mut output = result.content;

                // Add metadata as a comment if we're reading a partial file
                if args.start_line.is_some() {
                    output = format!(
                        "# File: {} (lines {}..{} of {})\n{}",
                        result.path,
                        args.start_line.unwrap_or(1),
                        args.start_line.unwrap_or(1) + args.num_lines.unwrap_or(result.lines) - 1,
                        result.lines,
                        output
                    );
                }

                Ok(ToolReturn::text(output))
            }
            Err(FileError::NotFound(path)) => {
                Ok(ToolReturn::error(format!("File not found: {}", path)))
            }
            Err(FileError::TooLarge(size, max)) => Ok(ToolReturn::error(format!(
                "File too large: {} bytes (max: {}). Use start_line and num_lines for partial reads.",
                size, max
            ))),
            Err(FileError::TokenLimitExceeded { estimated_tokens, total_lines, suggested_chunk_size }) => {
                Ok(ToolReturn::error(format!(
                    "[FILE TOO LARGE: ~{} tokens, {} lines]\n\
                     This file exceeds the 10,000 token safety limit.\n\
                     Please read it in chunks using start_line and num_lines parameters.\n\
                     Suggested: start_line=1, num_lines={}",
                    estimated_tokens, total_lines, suggested_chunk_size
                )))
            }
            Err(e) => Ok(ToolReturn::error(format!("Failed to read file: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file_tool_not_found() {
        let tool = ReadFileTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "file_path": "/nonexistent/file.txt"
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.as_text().unwrap().contains("not found"));
    }
}
