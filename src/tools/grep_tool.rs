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
