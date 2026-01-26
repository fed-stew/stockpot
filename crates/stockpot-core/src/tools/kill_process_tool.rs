//! KillProcess tool implementation.
//!
//! Terminates a running terminal process.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::debug;

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::tool_context::get_global_context;

/// Tool for killing a running process.
#[derive(Debug, Clone, Default)]
pub struct KillProcessTool;

#[derive(Debug, Deserialize)]
struct KillProcessArgs {
    process_id: String,
}

#[async_trait]
impl Tool for KillProcessTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "kill_process",
            "Terminate a running terminal process. Use this to stop long-running \
             commands or processes that are stuck. The process will receive SIGKILL.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "process_id",
                    "The process ID to kill (e.g., 'proc-1').",
                    true,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "kill_process", ?args, "Tool called");

        let args: KillProcessArgs = crate::tools::common::parse_tool_args_lenient(
            "kill_process",
            args.clone(),
            self.definition().parameters(),
        )?;

        let Some(tool_ctx) = get_global_context() else {
            return Ok(ToolReturn::error("Terminal system not initialized"));
        };

        // Check if process exists
        let Some(snapshot) = tool_ctx.store.snapshot(&args.process_id) else {
            return Ok(ToolReturn::error(format!(
                "Process not found: {}",
                args.process_id
            )));
        };

        // Check if already exited
        if snapshot.exit_code.is_some() {
            return Ok(ToolReturn::text(format!(
                "Process {} has already exited (exit code: {:?})",
                args.process_id, snapshot.exit_code
            )));
        }

        // Send kill request
        match tool_ctx.kill_process(args.process_id.clone()).await {
            Ok(()) => Ok(ToolReturn::text(format!(
                "Process {} has been terminated",
                args.process_id
            ))),
            Err(e) => Ok(ToolReturn::error(format!(
                "Failed to kill process {}: {}",
                args.process_id, e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition() {
        let tool = KillProcessTool;
        let def = tool.definition();
        assert_eq!(def.name(), "kill_process");
        assert!(def.description().contains("Terminate"));
    }

    #[tokio::test]
    async fn test_call_without_context() {
        let tool = KillProcessTool;
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
        let tool = KillProcessTool;
        let ctx = RunContext::minimal("test");
        let result = tool.call(&ctx, serde_json::json!({})).await;
        assert!(result.is_err());
    }
}
