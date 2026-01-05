//! ShareReasoning tool implementation.
//!
//! Provides a serdesAI-compatible tool for sharing agent reasoning with users.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use serdes_ai_tools::{RunContext, SchemaBuilder, Tool, ToolDefinition, ToolResult, ToolReturn};

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

#[cfg(test)]
mod tests {
    use super::*;

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
