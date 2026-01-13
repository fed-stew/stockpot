//! ReadProcessOutput tool implementation.
//!
//! Reads output from a running or completed terminal process.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::tool_context::get_global_context;

/// Tool for reading process output.
#[derive(Debug, Clone, Default)]
pub struct ReadProcessOutputTool;

#[derive(Debug, Deserialize)]
struct ReadProcessOutputArgs {
    process_id: String,
    /// If true, wait up to 10 seconds for more output
    #[serde(default)]
    wait_for_more: bool,
}

#[async_trait]
impl Tool for ReadProcessOutputTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "read_process_output",
            "Read output from a terminal process. Returns the current output buffer \
             for the specified process ID. Optionally wait for more output if the \
             process is still running.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "process_id",
                    "The process ID to read output from (e.g., 'proc-1').",
                    true,
                )
                .boolean(
                    "wait_for_more",
                    "If true, wait up to 10 seconds for more output before returning. \
                     Useful for streaming output from running processes.",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "read_process_output", ?args, "Tool called");

        let args: ReadProcessOutputArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "read_process_output", error = %e, "Failed to parse arguments");
            serdes_ai_tools::ToolError::execution_failed(format!("Invalid arguments: {}", e))
        })?;

        let Some(tool_ctx) = get_global_context() else {
            return Ok(ToolReturn::error("Terminal system not initialized"));
        };

        // Get initial snapshot
        let Some(snapshot) = tool_ctx.store.snapshot(&args.process_id) else {
            return Ok(ToolReturn::error(format!(
                "Process not found: {}",
                args.process_id
            )));
        };

        // If process is still running and wait_for_more is true, wait for update
        if args.wait_for_more && snapshot.exit_code.is_none() {
            let timeout = std::time::Duration::from_secs(10);
            if let Some((output, exit_code)) = tool_ctx
                .wait_for_completion(&args.process_id, timeout)
                .await
            {
                return Ok(format_output(&args.process_id, &output, exit_code));
            }
            // Timeout - return current output
            if let Some(snap) = tool_ctx.store.snapshot(&args.process_id) {
                return Ok(format_output(
                    &args.process_id,
                    &snap.output,
                    snap.exit_code,
                ));
            }
        }

        Ok(format_output(
            &args.process_id,
            &snapshot.output,
            snapshot.exit_code,
        ))
    }
}

fn format_output(process_id: &str, output: &str, exit_code: Option<i32>) -> ToolReturn {
    let status = if let Some(code) = exit_code {
        if code == 0 {
            format!("Process {} completed (exit: {})", process_id, code)
        } else {
            format!("Process {} failed (exit: {})", process_id, code)
        }
    } else {
        format!("Process {} is still running", process_id)
    };

    let mut result = status;
    result.push_str("\n\n--- Output ---\n");
    if output.is_empty() {
        result.push_str("(no output yet)");
    } else {
        result.push_str(output);
    }

    ToolReturn::text(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition() {
        let tool = ReadProcessOutputTool;
        let def = tool.definition();
        assert_eq!(def.name(), "read_process_output");
        assert!(def.description().contains("Read"));
    }

    #[tokio::test]
    async fn test_call_without_context() {
        let tool = ReadProcessOutputTool;
        let ctx = RunContext::minimal("test");
        let result = tool
            .call(&ctx, serde_json::json!({ "process_id": "proc-1" }))
            .await;
        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.is_error() || ret.as_text().unwrap().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_call_missing_process_id() {
        let tool = ReadProcessOutputTool;
        let ctx = RunContext::minimal("test");
        let result = tool.call(&ctx, serde_json::json!({})).await;
        assert!(result.is_err());
    }
}
