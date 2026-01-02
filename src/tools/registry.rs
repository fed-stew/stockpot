//! Tool registry for Stockpot agents.
//!
//! This module provides serdesAI-compatible tool implementations that wrap
//! the underlying file and shell operations.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::agent_tools::{InvokeAgentTool, ListAgentsTool};
use super::file_ops::{self, FileError};
use super::shell::{self, ShellError};

/// Arc-wrapped tool for shared ownership.
pub type ArcTool = Arc<dyn Tool + Send + Sync>;

// ============================================================================
// ListFilesTool
// ============================================================================

/// Tool for listing files in a directory.
#[derive(Debug, Clone, Default)]
pub struct ListFilesTool;

#[derive(Debug, Deserialize)]
struct ListFilesArgs {
    directory: Option<String>,
    recursive: Option<bool>,
    max_depth: Option<usize>,
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
                    "Maximum depth for recursive listing. Defaults to 10.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "list_files", ?args, "Tool called");

        let args: ListFilesArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "list_files", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        let directory = args.directory.as_deref().unwrap_or(".");
        let recursive = args.recursive.unwrap_or(true);
        let max_depth = args.max_depth;

        match file_ops::list_files(directory, recursive, max_depth) {
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

                output.push_str(&format!(
                    "\n\nSummary: {} files, {} directories, {} bytes total",
                    result.total_files, result.total_dirs, result.total_size
                ));

                Ok(ToolReturn::text(output))
            }
            Err(e) => Ok(ToolReturn::error(format!("Failed to list files: {}", e))),
        }
    }
}

// ============================================================================
// ReadFileTool
// ============================================================================

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
                        "# File: {} (lines {}..{} of {})
{}",
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
            Err(FileError::TooLarge(size, max)) => {
                Ok(ToolReturn::error(format!(
                    "File too large: {} bytes (max: {}). Use start_line and num_lines for partial reads.",
                    size, max
                )))
            }
            Err(e) => Ok(ToolReturn::error(format!("Failed to read file: {}", e))),
        }
    }
}

// ============================================================================
// EditFileTool
// ============================================================================

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

// ============================================================================
// GrepTool
// ============================================================================

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

// ============================================================================
// RunShellCommandTool
// ============================================================================

/// Tool for executing shell commands.
#[derive(Debug, Clone, Default)]
pub struct RunShellCommandTool;

#[derive(Debug, Deserialize)]
struct RunShellCommandArgs {
    command: String,
    working_directory: Option<String>,
    timeout_seconds: Option<u64>,
}

#[async_trait]
impl Tool for RunShellCommandTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "run_shell_command",
            "Execute a shell command with comprehensive monitoring. \
             Commands are executed in a controlled environment with timeout handling. \
             Use this to run tests, build projects, or execute system commands.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string("command", "The shell command to execute.", true)
                .string(
                    "working_directory",
                    "Working directory for command execution. If not specified, \
                     uses the current working directory.",
                    false,
                )
                .integer(
                    "timeout_seconds",
                    "Timeout in seconds. If no output is produced for this duration, \
                     the process will be terminated. Defaults to 60 seconds.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "run_shell_command", ?args, "Tool called");

        let args: RunShellCommandArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "run_shell_command", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        // Build the command runner with options
        let mut runner = shell::CommandRunner::new();

        if let Some(dir) = &args.working_directory {
            runner = runner.working_dir(dir);
        }

        if let Some(timeout) = args.timeout_seconds {
            runner = runner.timeout(timeout);
        }

        match runner.run(&args.command) {
            Ok(result) => {
                let mut output = String::new();

                // Include exit status
                if result.success {
                    output.push_str(&format!(
                        "Command completed successfully (exit code: {})\n",
                        result.exit_code
                    ));
                } else {
                    output.push_str(&format!(
                        "Command failed (exit code: {})\n",
                        result.exit_code
                    ));
                }

                // Include stdout if present
                if !result.stdout.trim().is_empty() {
                    output.push_str("\n--- stdout ---\n");
                    output.push_str(&result.stdout);
                }

                // Include stderr if present
                if !result.stderr.trim().is_empty() {
                    output.push_str("\n--- stderr ---\n");
                    output.push_str(&result.stderr);
                }

                Ok(ToolReturn::text(output))
            }
            Err(ShellError::NotFound(cmd)) => {
                Ok(ToolReturn::error(format!("Command not found: {}", cmd)))
            }
            Err(ShellError::Timeout(secs)) => Ok(ToolReturn::error(format!(
                "Command timed out after {} seconds",
                secs
            ))),
            Err(e) => Ok(ToolReturn::error(format!(
                "Command execution failed: {}",
                e
            ))),
        }
    }
}

// ============================================================================
// ShareReasoningTool
// ============================================================================

/// Tool for sharing the agent's reasoning with the user.
#[derive(Debug, Clone, Default)]
pub struct ShareReasoningTool;

#[derive(Debug, Deserialize)]
struct ShareReasoningArgs {
    reasoning: String,
    next_steps: Option<String>,
}

#[async_trait]
impl Tool for ShareReasoningTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "share_your_reasoning",
            "Share your current reasoning and planned next steps with the user. \
             This provides transparency into your decision-making process. \
             Use this to explain WHY you're doing something before doing it.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "reasoning",
                    "Your current thought process, analysis, or reasoning. \
                     This should be clear, comprehensive, and explain the 'why' behind decisions.",
                    true,
                )
                .string(
                    "next_steps",
                    "Planned upcoming actions or steps you intend to take. \
                     Can be omitted if no specific next steps are determined.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "share_reasoning", ?args, "Tool called");

        let args: ShareReasoningArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "share_reasoning", error = %e, ?args, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!(
                "Invalid arguments: {}. Got: {}",
                e, args
            ))
        })?;

        // Just acknowledge - the actual display
        // Note: In a real implementation, this would send to a message bus
        // for the UI to display. For now, we just acknowledge.
        let mut output = format!("🧠 Reasoning shared:\n{}", args.reasoning);

        if let Some(steps) = &args.next_steps {
            output.push_str(&format!("\n\n📋 Next steps:\n{}", steps));
        }

        Ok(ToolReturn::text(output))
    }
}

// ============================================================================
// DeleteFileTool
// ============================================================================

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

// ============================================================================
// SpotToolRegistry
// ============================================================================

/// Registry holding all available Stockpot tools.
///
/// This provides a convenient way to create and access all tools
/// for use with serdesAI agents.
#[derive(Debug, Default)]
pub struct SpotToolRegistry {
    pub list_files: ListFilesTool,
    pub read_file: ReadFileTool,
    pub edit_file: EditFileTool,
    pub delete_file: DeleteFileTool,
    pub grep: GrepTool,
    pub run_shell_command: RunShellCommandTool,
    pub share_reasoning: ShareReasoningTool,
    pub invoke_agent: InvokeAgentTool,
    pub list_agents: ListAgentsTool,
}

impl SpotToolRegistry {
    /// Create a new registry with all tools.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all tools as Arc-wrapped trait objects for shared ownership.
    pub fn all_tools(&self) -> Vec<ArcTool> {
        vec![
            Arc::new(self.list_files.clone()),
            Arc::new(self.read_file.clone()),
            Arc::new(self.edit_file.clone()),
            Arc::new(self.delete_file.clone()),
            Arc::new(self.grep.clone()),
            Arc::new(self.run_shell_command.clone()),
            Arc::new(self.share_reasoning.clone()),
            Arc::new(self.invoke_agent.clone()),
            Arc::new(self.list_agents.clone()),
        ]
    }

    /// Get tool definitions for all tools.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.all_tools().iter().map(|t| t.definition()).collect()
    }

    /// Get a subset of tools by name.
    pub fn tools_by_name(&self, names: &[&str]) -> Vec<ArcTool> {
        let mut tools: Vec<ArcTool> = Vec::new();

        for name in names {
            match *name {
                "list_files" => tools.push(Arc::new(self.list_files.clone())),
                "read_file" => tools.push(Arc::new(self.read_file.clone())),
                "edit_file" => tools.push(Arc::new(self.edit_file.clone())),
                "delete_file" => tools.push(Arc::new(self.delete_file.clone())),
                "grep" => tools.push(Arc::new(self.grep.clone())),
                "run_shell_command" => tools.push(Arc::new(self.run_shell_command.clone())),
                "share_your_reasoning" => tools.push(Arc::new(self.share_reasoning.clone())),
                "invoke_agent" => tools.push(Arc::new(self.invoke_agent.clone())),
                "list_agents" => tools.push(Arc::new(self.list_agents.clone())),
                _ => {} // Unknown tool, skip
            }
        }

        tools
    }

    /// Get read-only tools (safe for reviewers and planning agents).
    pub fn read_only_tools(&self) -> Vec<ArcTool> {
        vec![
            Arc::new(self.list_files.clone()),
            Arc::new(self.read_file.clone()),
            Arc::new(self.grep.clone()),
            Arc::new(self.share_reasoning.clone()),
        ]
    }

    /// Get file operation tools only.
    pub fn file_tools(&self) -> Vec<ArcTool> {
        vec![
            Arc::new(self.list_files.clone()),
            Arc::new(self.read_file.clone()),
            Arc::new(self.edit_file.clone()),
            Arc::new(self.delete_file.clone()),
            Arc::new(self.grep.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.all_tools().len(), 9);
        assert_eq!(registry.definitions().len(), 9);
    }

    #[test]
    fn test_tools_by_name() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["read_file", "edit_file"]);
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_read_only_tools() {
        let registry = SpotToolRegistry::new();
        let tools = registry.read_only_tools();
        // Should not include edit_file, delete_file, run_shell_command
        for tool in &tools {
            let name = tool.definition().name;
            assert!(
                name == "list_files"
                    || name == "read_file"
                    || name == "grep"
                    || name == "share_your_reasoning",
                "Unexpected tool in read_only: {}",
                name
            );
        }
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let def = tool.definition();
            assert!(!def.name.is_empty(), "Tool name is empty");
            assert!(!def.description.is_empty(), "Tool description is empty");
        }
    }

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

    #[tokio::test]
    async fn test_share_reasoning_tool() {
        let tool = ShareReasoningTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "reasoning": "I need to analyze the code structure first.",
                    "next_steps": "1. List files\n2. Read main.rs"
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        let text = ret.as_text().unwrap();
        assert!(text.contains("Reasoning shared"));
        assert!(text.contains("Next steps"));
    }
}
