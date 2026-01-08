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

                // Indicate if output was truncated
                if result.stdout_truncated || result.stderr_truncated {
                    output.push_str("\n\n⚠️ Output was truncated due to size limits.");
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
