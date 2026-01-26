//! ListProcesses tool implementation.
//!
//! Lists all active terminal processes tracked by the system.

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tracing::debug;

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

use super::tool_context::get_global_context;

/// Tool for listing active terminal processes.
#[derive(Debug, Clone, Default)]
pub struct ListProcessesTool;

#[async_trait]
impl Tool for ListProcessesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "list_processes",
            "List all active terminal processes. Returns process IDs, status, and basic info \
             for each running or recently completed command. Use this to check on background \
             processes or see what commands are still running.",
        )
        .with_parameters(SchemaBuilder::new().build().expect("schema build failed"))
    }

    async fn call(&self, _ctx: &RunContext, _args: JsonValue) -> ToolResult {
        debug!(tool = "list_processes", "Tool called");

        let Some(tool_ctx) = get_global_context() else {
            return Ok(ToolReturn::text(
                "No active processes (terminal system not initialized)",
            ));
        };

        let snapshots = tool_ctx.store.all_snapshots();

        if snapshots.is_empty() {
            return Ok(ToolReturn::text("No active processes"));
        }

        let mut output = String::new();
        output.push_str(&format!("Active processes: {}\n\n", snapshots.len()));

        for snap in &snapshots {
            let status = if let Some(code) = snap.exit_code {
                if code == 0 {
                    format!("✓ Completed (exit: {})", code)
                } else {
                    format!("✗ Failed (exit: {})", code)
                }
            } else {
                "⟳ Running".to_string()
            };

            let kind = match snap.kind {
                crate::terminal::ProcessKind::Llm => "LLM",
                crate::terminal::ProcessKind::User => "User",
            };

            output.push_str(&format!("• {} [{}] - {}\n", snap.process_id, kind, status));

            // Show output preview (first 100 chars)
            let preview: String = snap.output.chars().take(100).collect();
            if !preview.is_empty() {
                let preview = preview.replace('\n', " ");
                output.push_str(&format!("  Preview: {}...\n", preview));
            }
            output.push('\n');
        }

        Ok(ToolReturn::text(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition() {
        let tool = ListProcessesTool;
        let def = tool.definition();
        assert_eq!(def.name(), "list_processes");
        assert!(def.description().contains("List"));
    }

    #[tokio::test]
    async fn test_call_without_context() {
        let tool = ListProcessesTool;
        let ctx = RunContext::minimal("test");
        let result = tool.call(&ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let ret = result.unwrap();
        assert!(ret.as_text().unwrap().contains("not initialized"));
    }
}
