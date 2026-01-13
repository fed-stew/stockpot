//! RunShellCommand tool implementation.
//!
//! Provides a serdesAI-compatible tool for executing shell commands.
//! Supports both PTY-based terminal execution (when available) and
//! fallback synchronous execution.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, info, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::shell;
use super::tool_context::get_global_context;

/// Tool for executing shell commands.
#[derive(Debug, Clone, Default)]
pub struct RunShellCommandTool;

/// Default timeout for waiting for command completion (30 seconds)
const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 30;

/// Maximum characters in shell output to protect context window
const SHELL_OUTPUT_MAX_CHARS: usize = 50_000;

#[derive(Debug, Deserialize)]
struct RunShellCommandArgs {
    command: String,
    working_directory: Option<String>,
    timeout_seconds: Option<u64>,
    /// If true, run in background and return process_id immediately
    #[serde(default)]
    background: bool,
}

/// Truncate output to protect context window, cutting at line boundaries
fn truncate_output(output: &str, max_chars: usize) -> (String, bool) {
    if output.len() <= max_chars {
        return (output.to_string(), false);
    }

    let mut truncated: String = output.chars().take(max_chars).collect();

    // Try to cut at a newline boundary for cleaner output
    if let Some(last_newline) = truncated.rfind('\n') {
        truncated.truncate(last_newline);
    }

    truncated.push_str(&format!(
        "\n\n[OUTPUT TRUNCATED: {} total chars, showing first {}]",
        output.len(),
        max_chars
    ));

    (truncated, true)
}

#[async_trait]
impl Tool for RunShellCommandTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "run_shell_command",
            "Execute a shell command with comprehensive monitoring. \
             Commands are executed in a PTY with real-time output streaming. \
             Fast commands (completing within 30s) return output directly. \
             Long-running commands return a process_id for monitoring.",
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
                    "Maximum time to wait for command completion before returning \
                     process_id for background monitoring. Defaults to 30 seconds.",
                    false,
                )
                .boolean(
                    "background",
                    "If true, run in background and return process_id immediately \
                     without waiting for completion. Use list_processes and \
                     read_process_output to monitor.",
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

        // Try to use terminal system if available
        if let Some(tool_ctx) = get_global_context() {
            return self.call_with_terminal(tool_ctx, args).await;
        }

        // Fallback to synchronous execution
        self.call_fallback(args)
    }
}

impl RunShellCommandTool {
    /// Execute using the terminal system with PTY support.
    async fn call_with_terminal(
        &self,
        tool_ctx: &super::tool_context::ToolContext,
        args: RunShellCommandArgs,
    ) -> ToolResult {
        info!(
            tool = "run_shell_command",
            command = %args.command,
            background = args.background,
            "Executing via terminal system"
        );

        // Request terminal spawn from UI
        let process_id = match tool_ctx
            .execute_shell(args.command.clone(), args.working_directory.clone())
            .await
        {
            Ok(id) => id,
            Err(e) => {
                return Ok(ToolReturn::error(format!(
                    "Failed to spawn terminal: {}",
                    e
                )));
            }
        };

        // If background mode, return immediately
        if args.background {
            return Ok(ToolReturn::text(format!(
                "Command started in background.\n\n\
                 Process ID: {}\n\n\
                 Use `list_processes` to see all running processes.\n\
                 Use `read_process_output` with process_id to get output.\n\
                 Use `kill_process` to terminate if needed.",
                process_id
            )));
        }

        // Wait for completion with timeout
        let timeout_secs = args.timeout_seconds.unwrap_or(DEFAULT_WAIT_TIMEOUT_SECS);
        let timeout = Duration::from_secs(timeout_secs);

        match tool_ctx.wait_for_completion(&process_id, timeout).await {
            Some((output, exit_code)) => {
                // Command completed within timeout
                let (output, _truncated) = truncate_output(&output, SHELL_OUTPUT_MAX_CHARS);
                let exit_code = exit_code.unwrap_or(-1);
                let status = if exit_code == 0 {
                    format!(
                        "Command completed successfully (exit code: {})\n",
                        exit_code
                    )
                } else {
                    format!("Command failed (exit code: {})\n", exit_code)
                };

                let mut result = status;
                if !output.trim().is_empty() {
                    result.push_str("\n--- output ---\n");
                    result.push_str(&output);
                }

                Ok(ToolReturn::text(result))
            }
            None => {
                // Command still running after timeout
                let output = tool_ctx.store.output(&process_id).unwrap_or_default();
                let (output, _truncated) = truncate_output(&output, SHELL_OUTPUT_MAX_CHARS);

                let mut result = format!(
                    "Command is still running after {}s timeout.\n\n\
                     Process ID: {}\n\n\
                     Use `read_process_output` to get more output.\n\
                     Use `kill_process` to terminate if needed.\n",
                    timeout_secs, process_id
                );

                if !output.trim().is_empty() {
                    result.push_str("\n--- output so far ---\n");
                    result.push_str(&output);
                }

                Ok(ToolReturn::text(result))
            }
        }
    }

    /// Fallback to synchronous execution (when terminal system not available).
    fn call_fallback(&self, args: RunShellCommandArgs) -> ToolResult {
        debug!(
            tool = "run_shell_command",
            "Using fallback synchronous execution"
        );

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

                if !result.stdout.trim().is_empty() {
                    output.push_str("\n--- stdout ---\n");
                    output.push_str(&result.stdout);
                }

                if !result.stderr.trim().is_empty() {
                    output.push_str("\n--- stderr ---\n");
                    output.push_str(&result.stderr);
                }

                if result.stdout_truncated || result.stderr_truncated {
                    output.push_str("\n\n⚠️ Output was truncated due to size limits.");
                }

                Ok(ToolReturn::text(output))
            }
            Err(e) => Ok(ToolReturn::error(format!(
                "Command execution failed: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // definition() Tests
    // =========================================================================

    #[test]
    fn test_definition_returns_correct_name() {
        let tool = RunShellCommandTool;
        let def = tool.definition();
        assert_eq!(def.name(), "run_shell_command");
    }

    #[test]
    fn test_definition_has_description() {
        let tool = RunShellCommandTool;
        let def = tool.definition();
        assert!(def.description().contains("Execute"));
        assert!(def.description().contains("shell"));
    }

    #[test]
    fn test_definition_has_parameters() {
        let tool = RunShellCommandTool;
        let def = tool.definition();
        let params = def.parameters();
        assert!(params.is_object());
        let schema_str = serde_json::to_string(params).unwrap();
        assert!(schema_str.contains("command"));
        assert!(schema_str.contains("working_directory"));
        assert!(schema_str.contains("timeout_seconds"));
    }

    #[test]
    fn test_definition_command_is_required() {
        let tool = RunShellCommandTool;
        let def = tool.definition();
        let params = def.parameters();
        let schema_str = serde_json::to_string(params).unwrap();
        assert!(schema_str.contains("required"));
        assert!(schema_str.contains("command"));
    }

    // =========================================================================
    // call() Success Tests
    // =========================================================================

    #[tokio::test]
    async fn test_call_success_with_output() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "echo hello" }))
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        let text = ret.as_text().unwrap();
        assert!(text.contains("successfully"));
        assert!(text.contains("hello"));
    }

    #[tokio::test]
    async fn test_call_success_exit_code_zero() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "echo test" }))
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("exit code: 0"));
    }

    #[tokio::test]
    async fn test_call_includes_stdout_section() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "echo output" }))
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("--- stdout ---"));
        assert!(text.contains("output"));
    }

    // =========================================================================
    // call() With working_directory Tests
    // =========================================================================

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_with_working_directory() {
        let dir = tempfile::tempdir().expect("tempdir failed");

        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "command": "pwd",
                    "working_directory": dir.path().to_str().unwrap()
                }),
            )
            .await
            .unwrap();

        // On macOS, /tmp is a symlink to /private/tmp
        let text = result.as_text().unwrap();
        let dir_str = dir.path().to_str().unwrap();
        assert!(
            text.contains(dir_str) || text.contains("/private") || text.contains("tmp"),
            "Expected working directory in output, got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_call_invalid_working_directory() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "command": "echo test",
                    "working_directory": "/nonexistent/path/xyz123abc"
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.is_error());
        assert!(ret.as_text().unwrap().contains("failed"));
    }

    // =========================================================================
    // call() With timeout_seconds Tests
    // =========================================================================

    #[tokio::test]
    async fn test_call_with_timeout_seconds() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        // Quick command with timeout - should succeed
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "command": "echo fast",
                    "timeout_seconds": 60
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(!ret.is_error());
        assert!(ret.as_text().unwrap().contains("fast"));
    }

    // =========================================================================
    // call() Command Not Found Tests
    // =========================================================================

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_command_not_found() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "command": "nonexistent_command_xyz123abc456" }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        // Either is_error or has non-zero exit code
        assert!(ret.is_error() || ret.as_text().unwrap().contains("failed"));
    }

    // =========================================================================
    // call() Failed Exit Code Tests
    // =========================================================================

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_failed_exit_code() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "exit 42" }))
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("failed"));
        assert!(text.contains("exit code: 42"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_exit_code_1() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "exit 1" }))
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("failed"));
        assert!(text.contains("exit code: 1"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_captures_stderr() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "echo error >&2" }))
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("--- stderr ---"));
        assert!(text.contains("error"));
    }

    // =========================================================================
    // call() Invalid Args Tests
    // =========================================================================

    #[tokio::test]
    async fn test_call_missing_command_returns_error() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_command_returns_error() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({ "command": 123 })).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_working_directory_returns_error() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "command": "echo", "working_directory": 123 }),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_wrong_type_timeout_returns_error() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "command": "echo", "timeout_seconds": "sixty" }),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_array_args_returns_error() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!(["echo", "hello"])).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_null_command_returns_error() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": null }))
            .await;

        assert!(result.is_err());
    }

    // =========================================================================
    // Additional Edge Cases
    // =========================================================================

    #[tokio::test]
    async fn test_call_empty_command() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        // Empty command should execute (shell handles it)
        let result = tool.call(&ctx, serde_json::json!({ "command": "" })).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_command_with_pipe() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "command": "echo 'hello world' | grep hello" }),
            )
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("successfully"));
        assert!(text.contains("hello"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_command_with_multiple_statements() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "command": "echo first; echo second" }),
            )
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("first"));
        assert!(text.contains("second"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_command_with_env_expansion() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!({ "command": "echo $HOME" }))
            .await
            .unwrap();

        // $HOME should be expanded
        let text = result.as_text().unwrap();
        assert!(text.contains("/"));
    }

    #[tokio::test]
    async fn test_call_extra_fields_ignored() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "command": "echo test",
                    "extra_field": "ignored"
                }),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_call_mixed_stdout_stderr() {
        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({ "command": "echo out; echo err >&2" }),
            )
            .await
            .unwrap();

        let text = result.as_text().unwrap();
        assert!(text.contains("--- stdout ---"));
        assert!(text.contains("out"));
        assert!(text.contains("--- stderr ---"));
        assert!(text.contains("err"));
    }

    #[test]
    fn test_tool_debug_impl() {
        let tool = RunShellCommandTool;
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("RunShellCommandTool"));
    }

    #[test]
    fn test_tool_clone_impl() {
        let tool = RunShellCommandTool;
        let cloned = tool.clone();
        assert_eq!(tool.definition().name(), cloned.definition().name());
    }

    #[test]
    fn test_tool_default_impl() {
        let tool = RunShellCommandTool::default();
        assert_eq!(tool.definition().name(), "run_shell_command");
    }

    #[tokio::test]
    async fn test_call_with_all_options() {
        let dir = tempfile::tempdir().expect("tempdir failed");

        let tool = RunShellCommandTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "command": "echo complete",
                    "working_directory": dir.path().to_str().unwrap(),
                    "timeout_seconds": 30
                }),
            )
            .await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.as_text().unwrap().contains("complete"));
    }
}
