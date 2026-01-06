//! RunShellCommand tool implementation.
//!
//! Provides a serdesAI-compatible tool for executing shell commands.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::shell::{self, ShellError};

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
