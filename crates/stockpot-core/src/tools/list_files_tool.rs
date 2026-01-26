//! ListFiles tool implementation.
//!
//! Provides a serdesAI-compatible tool for listing files and directories.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::debug;

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::file_ops;

/// Maximum characters in list_files output to protect context window
const LIST_FILES_MAX_OUTPUT_CHARS: usize = 100_000;

/// Tool for listing files in a directory.
#[derive(Debug, Clone, Default)]
pub struct ListFilesTool;

#[derive(Debug, Deserialize)]
struct ListFilesArgs {
    directory: Option<String>,
    recursive: Option<bool>,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
}

#[async_trait]
impl Tool for ListFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "list_files",
            "List files and directories with intelligent filtering. \
             Automatically ignores common build artifacts, cache directories, \
             and other noise while providing rich file metadata.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "directory",
                    "Path to the directory to list. Can be relative or absolute. \
                     Defaults to '.' (current directory).",
                    false,
                )
                .boolean(
                    "recursive",
                    "Whether to recursively list subdirectories. Defaults to true.",
                    false,
                )
                .integer(
                    "max_depth",
                    "Maximum depth for recursive listing. Defaults to 10 (hard cap: 50).",
                    false,
                )
                .integer(
                    "max_entries",
                    "Maximum number of entries to return. Defaults to 2000 (hard cap: 10000).",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "list_files", ?args, "Tool called");

        let args: ListFilesArgs = crate::tools::common::parse_tool_args_lenient(
            "list_files",
            args.clone(),
            &self.definition().parameters(),
        )?;

        let directory = args.directory.as_deref().unwrap_or(".");
        let recursive = args.recursive.unwrap_or(true);
        let max_depth = args.max_depth;
        let max_entries = args.max_entries;

        match file_ops::list_files(directory, recursive, max_depth, max_entries) {
            Ok(result) => {
                // Format as a readable summary with file tree
                let mut output =
                    format!("DIRECTORY LISTING: {} (recursive={})", directory, recursive);

                for entry in &result.entries {
                    let indent = "  ".repeat(entry.depth);
                    let marker = if entry.is_dir { "/" } else { "" };
                    let size = if entry.is_dir {
                        String::new()
                    } else {
                        format!(" ({} bytes)", entry.size)
                    };
                    output.push_str(&format!("\n{}{}{}{}", indent, entry.name, marker, size));
                }

                let truncation_note = if result.truncated {
                    format!(
                        " (truncated to {} entries; totals reflect returned entries only)",
                        result.max_entries
                    )
                } else {
                    String::new()
                };

                output.push_str(&format!(
                    "\n\nSummary: {} files, {} directories, {} bytes total{}",
                    result.total_files, result.total_dirs, result.total_size, truncation_note
                ));

                // Protect against massive output overwhelming context
                if output.len() > LIST_FILES_MAX_OUTPUT_CHARS {
                    output.truncate(LIST_FILES_MAX_OUTPUT_CHARS);
                    // Find a good break point (newline) to avoid cutting mid-line
                    if let Some(last_newline) = output.rfind('\n') {
                        output.truncate(last_newline);
                    }
                    output.push_str("\n\n[OUTPUT TRUNCATED - directory listing too large. Use max_entries parameter or recursive=false for smaller results]");
                }

                Ok(ToolReturn::text(output))
            }
            Err(e) => Ok(ToolReturn::error(format!("Failed to list files: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_files_tool() {
        let tool = ListFilesTool;
        let ctx = RunContext::minimal("test");

        // Test with current directory
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "directory": ".",
                    "recursive": false
                }),
            )
            .await;

        assert!(result.is_ok());
    }
}
