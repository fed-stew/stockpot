//! Agent-related tools for sub-agent invocation.
//!
//! These tools allow agents to delegate tasks to other specialized agents.

use crate::agents::{AgentExecutor, AgentManager};
use crate::db::Database;
use crate::mcp::McpManager;
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serdes_ai_tools::{
    RunContext, SchemaBuilder, Tool, ToolDefinition, ToolError, ToolResult, ToolReturn,
};
use tracing::{debug, warn};

// ============================================================================
// InvokeAgentTool
// ============================================================================

/// Tool for invoking another agent with a prompt.
///
/// This allows agents to delegate specialized tasks to other agents.
/// For example, the main stockpot agent might delegate code review
/// to a language-specific reviewer agent.
#[derive(Debug, Clone, Default)]
pub struct InvokeAgentTool;

#[derive(Debug, Deserialize)]
struct InvokeAgentArgs {
    /// Name of the agent to invoke.
    agent_name: String,
    /// The prompt to send to the agent.
    prompt: String,
    /// Optional session ID for conversation continuity.
    #[serde(default)]
    session_id: Option<String>,
}

#[async_trait]
impl Tool for InvokeAgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "invoke_agent",
            "Invoke another agent with a prompt. Use this to delegate specialized tasks \
             to other agents like code reviewers or planners.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "agent_name",
                    "The name of the agent to invoke (e.g., 'python-reviewer', 'planner')",
                    true,
                )
                .string("prompt", "The prompt/task to send to the agent", true)
                .string(
                    "session_id",
                    "Optional session ID for conversation continuity",
                    false,
                )
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "invoke_agent", ?args, "Tool called");

        let args: InvokeAgentArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "invoke_agent", error = %e, ?args, "Failed to parse arguments");
            ToolError::execution_failed(format!("Invalid arguments: {}. Got: {}", e, args))
        })?;

        // TODO: Full implementation requires access to Database and executor context
        // For now, return an error explaining the limitation
        Err(ToolError::execution_failed(format!(
            "Sub-agent invocation is not yet fully implemented. \
             To use the '{}' agent, switch to it with /agent {} and ask your question directly.",
            args.agent_name, args.agent_name
        )))
    }
}

// ============================================================================
// ListAgentsTool
// ============================================================================

/// Tool for listing available agents.
///
/// Returns information about all registered agents that can be invoked.
#[derive(Debug, Clone, Default)]
pub struct ListAgentsTool;

#[async_trait]
impl Tool for ListAgentsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "list_agents",
            "List all available agents. Use this to discover what specialized agents \
             are available for delegation.",
        )
        .with_parameters(SchemaBuilder::new().build().expect("schema build failed"))
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "list_agents", ?args, "Tool called");

        // Create a temporary manager to list agents
        let manager = AgentManager::new();
        let agents = manager.list();

        let agent_list: Vec<_> = agents
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "display_name": a.display_name,
                    "description": a.description
                })
            })
            .collect();

        Ok(ToolReturn::json(serde_json::json!({
            "agents": agent_list,
            "count": agent_list.len()
        })))
    }
}

// ============================================================================
// ShareReasoningTool (moved from registry.rs for organization)
// ============================================================================

/// Tool for sharing agent reasoning with the user.
///
/// This helps users understand the agent's thought process.
#[derive(Debug, Clone, Default)]
pub struct ShareReasoningTool;

#[derive(Debug, Deserialize)]
struct ShareReasoningArgs {
    /// The agent's current reasoning/thinking.
    reasoning: String,
    /// Optional planned next steps.
    #[serde(default)]
    next_steps: Option<String>,
}

#[async_trait]
impl Tool for ShareReasoningTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "share_your_reasoning",
            "Share your current reasoning and planned next steps with the user. \
             Use this to explain your thought process before taking actions.",
        )
        .with_parameters(
            SchemaBuilder::new()
                .string(
                    "reasoning",
                    "Your current thought process and analysis",
                    true,
                )
                .string("next_steps", "What you plan to do next", false)
                .build()
                .expect("schema build failed"),
        )
    }

    async fn call(&self, _ctx: &RunContext, args: JsonValue) -> ToolResult {
        debug!(tool = "share_reasoning", ?args, "Tool called");

        let args: ShareReasoningArgs = serde_json::from_value(args.clone()).map_err(|e| {
            warn!(tool = "share_reasoning", error = %e, ?args, "Failed to parse arguments");
            ToolError::execution_failed(format!("Invalid arguments: {}. Got: {}", e, args))
        })?;

        let mut output = format!("💭 **Reasoning:**\n{}\n", args.reasoning);

        if let Some(steps) = args.next_steps {
            output.push_str(&format!("\n📋 **Next Steps:**\n{}", steps));
        }

        Ok(ToolReturn::text(output))
    }
}

// ============================================================================
// Helper Types
// ============================================================================

/// Result of invoking a sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeAgentResult {
    pub agent_name: String,
    pub response: String,
    pub session_id: Option<String>,
    pub success: bool,
}

/// Error type for agent tool operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentToolError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),
}

// ============================================================================
// Executor-based Invocation (for use when we have database access)
// ============================================================================

/// Invoke a sub-agent with full executor support.
///
/// This is the full implementation for when we have access to the database.
pub async fn invoke_agent_with_executor(
    db: &Database,
    manager: &AgentManager,
    agent_name: &str,
    prompt: &str,
) -> Result<InvokeAgentResult, AgentToolError> {
    let agent = manager
        .get(agent_name)
        .ok_or_else(|| AgentToolError::AgentNotFound(agent_name.to_string()))?;

    let model_registry = ModelRegistry::load_from_db(db).unwrap_or_default();
    let executor = AgentExecutor::new(db, &model_registry);
    let tool_registry = SpotToolRegistry::new();
    let mcp_manager = McpManager::new();

    match executor
        .execute(
            agent,
            "gpt-4o", // TODO: Get from context
            prompt,
            None,
            &tool_registry,
            &mcp_manager,
        )
        .await
    {
        Ok(result) => Ok(InvokeAgentResult {
            agent_name: agent_name.to_string(),
            response: result.output,
            session_id: Some(result.run_id),
            success: true,
        }),
        Err(e) => Err(AgentToolError::ExecutionFailed(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoke_agent_tool_definition() {
        let tool = InvokeAgentTool;
        let def = tool.definition();
        assert_eq!(def.name, "invoke_agent");
        assert!(def.description.contains("delegate"));
    }

    #[test]
    fn test_list_agents_tool_definition() {
        let tool = ListAgentsTool;
        let def = tool.definition();
        assert_eq!(def.name, "list_agents");
    }

    #[test]
    fn test_share_reasoning_tool_definition() {
        let tool = ShareReasoningTool;
        let def = tool.definition();
        assert_eq!(def.name, "share_your_reasoning");
    }
}
