//! Agent-related tools for sub-agent invocation.
//!
//! These tools allow agents to delegate tasks to other specialized agents.

use crate::agents::{AgentExecutor, AgentManager, RetryHandler};
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
    // Note: prompt and session_id are part of the JSON schema but not used
    // in this fallback implementation. The real implementation is in
    // executor/sub_agents.rs which has its own args parsing.
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

        let args: InvokeAgentArgs = crate::tools::common::parse_tool_args_lenient(
            "invoke_agent",
            args.clone(),
            &self.definition().parameters(),
        )?;

        // Note: This code path should not be reached in normal operation.
        // The AgentExecutor intercepts invoke_agent calls and routes them through
        // InvokeAgentExecutor (in executor/sub_agents.rs) which has proper context.
        // This fallback exists for direct tool testing or edge cases.
        let name = &args.agent_name;
        Err(ToolError::execution_failed(format!(
            "Sub-agent '{name}' cannot be invoked directly through this tool. \
             Use the agent executor's sub-agent support instead."
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
    model_name: &str,
) -> Result<InvokeAgentResult, AgentToolError> {
    let agent = manager
        .get(agent_name)
        .ok_or_else(|| AgentToolError::AgentNotFound(agent_name.to_string()))?;

    let model_registry = ModelRegistry::load_from_db(db).unwrap_or_default();

    // Create retry handler for automatic key rotation on 429s
    #[allow(clippy::arc_with_non_send_sync)]
    let retry_handler = match Database::open_at(db.path().clone()) {
        Ok(retry_db) => Some(RetryHandler::new(std::sync::Arc::new(retry_db))),
        Err(e) => {
            warn!(error = %e, "Failed to create retry handler, key rotation disabled");
            None
        }
    };

    let mut executor = AgentExecutor::new(db, &model_registry);
    if let Some(handler) = retry_handler {
        executor = executor.with_retry_handler(handler);
    }
    let tool_registry = SpotToolRegistry::new();
    let mcp_manager = McpManager::new();

    match executor
        .execute(
            agent,
            model_name,
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

    // =========================================================================
    // InvokeAgentTool Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_definition() {
        let tool = InvokeAgentTool;
        let def = tool.definition();
        assert_eq!(def.name, "invoke_agent");
        assert!(def.description.contains("delegate"));
    }

    #[test]
    fn test_invoke_agent_tool_has_parameters() {
        let tool = InvokeAgentTool;
        let def = tool.definition();
        // parameters() returns a reference to the schema
        let params = def.parameters();
        assert!(params.is_object() || params.is_null());
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_returns_not_implemented() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "planner",
                    "prompt": "Create a plan"
                }),
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("cannot be invoked directly"));
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_with_session_id() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "reviewer",
                    "prompt": "Review this code",
                    "session_id": "session-123"
                }),
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("reviewer"));
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_only_agent_name_required() {
        // InvokeAgentArgs only requires agent_name for the fallback error path
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "planner"
                }),
            )
            .await;

        // Should succeed in parsing but return the "cannot invoke directly" error
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be invoked directly"));
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_missing_agent_name() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "prompt": "Do something"
                }),
            )
            .await;

        assert!(result.is_err());
    }

    // =========================================================================
    // ListAgentsTool Tests
    // =========================================================================

    #[test]
    fn test_list_agents_tool_definition() {
        let tool = ListAgentsTool;
        let def = tool.definition();
        assert_eq!(def.name, "list_agents");
        assert!(def.description.contains("available"));
    }

    #[tokio::test]
    async fn test_list_agents_tool_returns_agents() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let ret = result.unwrap();
        let json = ret.as_json().expect("should be JSON");
        assert!(json.get("agents").is_some());
        assert!(json.get("count").is_some());

        let count = json.get("count").unwrap().as_u64().unwrap();
        assert!(count > 0, "should have at least one agent");
    }

    #[tokio::test]
    async fn test_list_agents_tool_includes_stockpot() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        let ret = result.unwrap();
        let json = ret.as_json().unwrap();
        let agents = json.get("agents").unwrap().as_array().unwrap();

        let stockpot = agents
            .iter()
            .find(|a| a.get("name").unwrap().as_str().unwrap() == "stockpot");
        assert!(stockpot.is_some(), "should include stockpot agent");
    }

    #[tokio::test]
    async fn test_list_agents_tool_agent_has_fields() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        let ret = result.unwrap();
        let json = ret.as_json().unwrap();
        let agents = json.get("agents").unwrap().as_array().unwrap();

        for agent in agents {
            assert!(agent.get("name").is_some());
            assert!(agent.get("display_name").is_some());
            assert!(agent.get("description").is_some());
        }
    }

    // =========================================================================
    // Helper Types Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_result_serialization() {
        let result = InvokeAgentResult {
            agent_name: "planner".to_string(),
            response: "Plan created".to_string(),
            session_id: Some("session-123".to_string()),
            success: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("planner"));

        let deserialized: InvokeAgentResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_name, "planner");
        assert!(deserialized.success);
    }

    #[test]
    fn test_invoke_agent_result_without_session() {
        let result = InvokeAgentResult {
            agent_name: "reviewer".to_string(),
            response: "Code looks good".to_string(),
            session_id: None,
            success: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: InvokeAgentResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.session_id.is_none());
    }

    #[test]
    fn test_agent_tool_error_display() {
        let not_found = AgentToolError::AgentNotFound("unknown-agent".to_string());
        assert_eq!(not_found.to_string(), "Agent not found: unknown-agent");

        let exec_failed = AgentToolError::ExecutionFailed("timeout".to_string());
        assert_eq!(exec_failed.to_string(), "Agent execution failed: timeout");
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_default() {
        let tool = InvokeAgentTool;
        assert_eq!(tool.definition().name, "invoke_agent");
    }

    #[test]
    fn test_list_agents_tool_default() {
        let tool = ListAgentsTool;
        assert_eq!(tool.definition().name, "list_agents");
    }

    // =========================================================================
    // Clone Trait Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_clone() {
        let tool = InvokeAgentTool;
        let cloned = tool.clone();
        assert_eq!(cloned.definition().name, "invoke_agent");
    }

    #[test]
    fn test_list_agents_tool_clone() {
        let tool = ListAgentsTool;
        let cloned = tool.clone();
        assert_eq!(cloned.definition().name, "list_agents");
    }

    // =========================================================================
    // Debug Trait Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_debug() {
        let tool = InvokeAgentTool;
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("InvokeAgentTool"));
    }

    #[test]
    fn test_list_agents_tool_debug() {
        let tool = ListAgentsTool;
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("ListAgentsTool"));
    }

    // =========================================================================
    // Schema Validation Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_schema_has_required_fields() {
        let tool = InvokeAgentTool;
        let def = tool.definition();
        let params = def.parameters();

        // Schema should be an object with properties
        assert!(params.is_object());
        let obj = params.as_object().unwrap();

        // Check for properties key
        let props = obj.get("properties");
        assert!(props.is_some(), "schema should have properties");

        let props_obj = props.unwrap().as_object().unwrap();
        assert!(props_obj.contains_key("agent_name"));
        assert!(props_obj.contains_key("prompt"));
        assert!(props_obj.contains_key("session_id"));
    }

    #[test]
    fn test_list_agents_tool_schema_is_empty_object() {
        let tool = ListAgentsTool;
        let def = tool.definition();
        let params = def.parameters();

        // list_agents has no parameters
        assert!(params.is_object());
    }

    // =========================================================================
    // Edge Case Tests - Empty Strings
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_empty_agent_name() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "",
                    "prompt": "Do something"
                }),
            )
            .await;

        // Empty agent name is technically valid args, so it parses but fails execution
        assert!(result.is_err());
        // Should mention the empty agent name in the error
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("not yet fully implemented") || err_str.contains("''"),
            "error should mention implementation or empty agent"
        );
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_empty_prompt() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "planner",
                    "prompt": ""
                }),
            )
            .await;

        // Empty prompt is valid args, fails at execution
        assert!(result.is_err());
    }

    // =========================================================================
    // Edge Case Tests - Invalid JSON Types
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_wrong_type_agent_name() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": 123,
                    "prompt": "Do something"
                }),
            )
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_ignores_invalid_extra_fields() {
        // Extra fields like prompt are ignored even if wrong type
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "planner",
                    "prompt": ["not", "a", "string"]
                }),
            )
            .await;

        // Parsing succeeds (extra fields ignored), returns invoke error
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be invoked directly"));
    }

    #[tokio::test]
    async fn test_list_agents_tool_ignores_extra_args() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        // list_agents should ignore extra parameters
        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "extra_field": "ignored",
                    "another": 123
                }),
            )
            .await;

        assert!(result.is_ok());
    }

    // =========================================================================
    // InvokeAgentResult Additional Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_result_debug() {
        let result = InvokeAgentResult {
            agent_name: "test".to_string(),
            response: "response".to_string(),
            session_id: None,
            success: false,
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("InvokeAgentResult"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("false"));
    }

    #[test]
    fn test_invoke_agent_result_clone() {
        let result = InvokeAgentResult {
            agent_name: "agent".to_string(),
            response: "resp".to_string(),
            session_id: Some("sess".to_string()),
            success: true,
        };

        let cloned = result.clone();
        assert_eq!(cloned.agent_name, result.agent_name);
        assert_eq!(cloned.response, result.response);
        assert_eq!(cloned.session_id, result.session_id);
        assert_eq!(cloned.success, result.success);
    }

    #[test]
    fn test_invoke_agent_result_failed() {
        let result = InvokeAgentResult {
            agent_name: "failed-agent".to_string(),
            response: "Error: something went wrong".to_string(),
            session_id: None,
            success: false,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], false);
        assert!(json["response"].as_str().unwrap().contains("Error"));
    }

    // =========================================================================
    // AgentToolError Additional Tests
    // =========================================================================

    #[test]
    fn test_agent_tool_error_debug() {
        let err = AgentToolError::AgentNotFound("missing".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("AgentNotFound"));
        assert!(debug_str.contains("missing"));
    }

    #[test]
    fn test_agent_tool_error_execution_failed_debug() {
        let err = AgentToolError::ExecutionFailed("crash".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ExecutionFailed"));
        assert!(debug_str.contains("crash"));
    }

    // =========================================================================
    // Args Deserialization Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_args_deserialize() {
        // InvokeAgentArgs only extracts agent_name for the fallback error path.
        // The full implementation in executor/sub_agents.rs parses all fields.
        let json = serde_json::json!({
            "agent_name": "planner",
            "prompt": "plan this",
            "session_id": "sess-123"
        });

        let args: InvokeAgentArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.agent_name, "planner");
    }

    #[test]
    fn test_invoke_agent_args_debug() {
        let args = InvokeAgentArgs {
            agent_name: "test".to_string(),
        };

        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("InvokeAgentArgs"));
    }

    // =========================================================================
    // Tool Description Content Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_description_content() {
        let tool = InvokeAgentTool;
        let def = tool.definition();

        assert!(def.description.contains("agent"));
        assert!(def.description.contains("delegate"));
    }

    #[test]
    fn test_list_agents_tool_description_content() {
        let tool = ListAgentsTool;
        let def = tool.definition();

        assert!(def.description.contains("List"));
        assert!(def.description.contains("agents"));
    }

    // =========================================================================
    // Null/Missing Args Edge Cases
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_ignores_extra_fields() {
        // Extra fields like prompt and session_id are ignored by the fallback
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "planner",
                    "prompt": "do it",
                    "session_id": null
                }),
            )
            .await;

        // Should succeed in parsing and return the "cannot invoke directly" error
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be invoked directly"));
    }

    // =========================================================================
    // Unicode and Special Characters Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_special_chars_in_name() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "agent-with-dashes_and_underscores",
                    "prompt": "test"
                }),
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("agent-with-dashes_and_underscores"));
    }

    // =========================================================================
    // Schema Required Fields Validation
    // =========================================================================

    #[test]
    fn test_invoke_agent_schema_required_array() {
        let tool = InvokeAgentTool;
        let def = tool.definition();
        let params = def.parameters();
        let obj = params.as_object().unwrap();

        // Check that required array exists and contains expected fields
        if let Some(required) = obj.get("required") {
            let req_arr = required.as_array().unwrap();
            let req_strs: Vec<&str> = req_arr.iter().map(|v| v.as_str().unwrap()).collect();
            assert!(
                req_strs.contains(&"agent_name"),
                "agent_name should be required"
            );
            assert!(req_strs.contains(&"prompt"), "prompt should be required");
            // session_id is optional, should NOT be in required
            assert!(
                !req_strs.contains(&"session_id"),
                "session_id should not be required"
            );
        }
    }

    // =========================================================================
    // Very Large Input Tests
    // =========================================================================

    // =========================================================================
    // Multiline Content Tests
    // =========================================================================

    // =========================================================================
    // ToolReturn Type Inspection Tests
    // =========================================================================

    #[tokio::test]
    async fn test_list_agents_tool_returns_json_type() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let ret = result.unwrap();
        // Should be JSON, not text
        assert!(ret.as_json().is_some());
        assert!(ret.as_text().is_none());
    }

    // =========================================================================
    // ListAgentsTool Response Structure Tests
    // =========================================================================

    #[tokio::test]
    async fn test_list_agents_tool_response_has_array_agents() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        let ret = result.unwrap();
        let json = ret.as_json().unwrap();

        let agents = json.get("agents").unwrap();
        assert!(agents.is_array());
    }

    #[tokio::test]
    async fn test_list_agents_tool_count_matches_agents_len() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        let ret = result.unwrap();
        let json = ret.as_json().unwrap();

        let agents = json.get("agents").unwrap().as_array().unwrap();
        let count = json.get("count").unwrap().as_u64().unwrap() as usize;

        assert_eq!(agents.len(), count);
    }

    #[tokio::test]
    async fn test_list_agents_tool_each_agent_has_name() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        let ret = result.unwrap();
        let json = ret.as_json().unwrap();
        let agents = json.get("agents").unwrap().as_array().unwrap();

        for agent in agents {
            let name = agent.get("name").unwrap().as_str().unwrap();
            assert!(!name.is_empty(), "agent name should not be empty");
        }
    }

    // =========================================================================
    // Tool Definition Consistency Tests
    // =========================================================================

    #[test]
    fn test_tool_definitions_have_non_empty_names() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(InvokeAgentTool), Box::new(ListAgentsTool)];

        for tool in tools {
            let def = tool.definition();
            assert!(!def.name.is_empty(), "tool name should not be empty");
        }
    }

    #[test]
    fn test_tool_definitions_have_non_empty_descriptions() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(InvokeAgentTool), Box::new(ListAgentsTool)];

        for tool in tools {
            let def = tool.definition();
            assert!(
                !def.description.is_empty(),
                "tool description should not be empty"
            );
        }
    }

    #[test]
    fn test_all_tool_names_are_unique() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(InvokeAgentTool), Box::new(ListAgentsTool)];

        let mut names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        let original_len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), original_len, "tool names should be unique");
    }

    // =========================================================================
    // Error Message Content Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_invalid_args_error_includes_received_args() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "wrong_field": "value"
                }),
            )
            .await;

        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        // Error message should include the invalid input for debugging
        assert!(err_str.contains("Got:") || err_str.contains("wrong_field"));
    }

    // =========================================================================
    // Edge Case: Extra Fields in Args
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_extra_fields_still_ignored() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "planner",
                    "prompt": "test",
                    "extra_field": "should be ignored",
                    "another_extra": 123
                }),
            )
            .await;

        // Should parse successfully, then fail at execution (not implemented)
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be invoked directly"));
    }

    // =========================================================================
    // InvokeAgentResult Deserialization Edge Cases
    // =========================================================================

    #[test]
    fn test_invoke_agent_result_deserialize_with_null_session() {
        let json = r#"{
            "agent_name": "test",
            "response": "resp",
            "session_id": null,
            "success": true
        }"#;

        let result: InvokeAgentResult = serde_json::from_str(json).unwrap();
        assert!(result.session_id.is_none());
    }

    #[test]
    fn test_invoke_agent_result_deserialize_with_empty_response() {
        let json = r#"{
            "agent_name": "test",
            "response": "",
            "session_id": null,
            "success": false
        }"#;

        let result: InvokeAgentResult = serde_json::from_str(json).unwrap();
        assert!(result.response.is_empty());
        assert!(!result.success);
    }

    // =========================================================================
    // AgentToolError Variants Exhaustive Tests
    // =========================================================================

    #[test]
    fn test_agent_tool_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AgentToolError>();
    }

    #[test]
    fn test_agent_tool_error_implements_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(AgentToolError::AgentNotFound("test".to_string()));
        assert!(!err.to_string().is_empty());
    }

    // =========================================================================
    // Whitespace Handling Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_whitespace_only_agent_name() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(
                &ctx,
                serde_json::json!({
                    "agent_name": "   ",
                    "prompt": "test"
                }),
            )
            .await;

        // Whitespace-only is valid for parsing, fails at execution
        assert!(result.is_err());
    }

    // =========================================================================
    // Escape Sequences in Content
    // =========================================================================

    // =========================================================================
    // Tool Trait Object Safety Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_tool_can_be_boxed() {
        let tool: Box<dyn Tool> = Box::new(InvokeAgentTool);
        assert_eq!(tool.definition().name, "invoke_agent");
    }

    #[test]
    fn test_list_agents_tool_can_be_boxed() {
        let tool: Box<dyn Tool> = Box::new(ListAgentsTool);
        assert_eq!(tool.definition().name, "list_agents");
    }

    // =========================================================================
    // Schema Type Validation Tests
    // =========================================================================

    #[test]
    fn test_invoke_agent_schema_property_types() {
        let tool = InvokeAgentTool;
        let def = tool.definition();
        let params = def.parameters();
        let obj = params.as_object().unwrap();
        let props = obj.get("properties").unwrap().as_object().unwrap();

        // Check that each property has a type field
        for (name, prop) in props {
            let prop_obj = prop.as_object().unwrap();
            let prop_type = prop_obj.get("type");
            assert!(prop_type.is_some(), "property {} should have a type", name);
            assert_eq!(
                prop_type.unwrap().as_str().unwrap(),
                "string",
                "property {} should be string type",
                name
            );
        }
    }

    // =========================================================================
    // Empty JSON Object Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_empty_json_object() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid arguments"));
    }

    // =========================================================================
    // Array Instead of Object Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_array_args() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool
            .call(&ctx, serde_json::json!(["planner", "test"]))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_agents_tool_array_args() {
        let tool = ListAgentsTool;
        let ctx = RunContext::minimal("test");

        // list_agents ignores args anyway, so even array should work
        let result = tool.call(&ctx, serde_json::json!(["ignored"])).await;
        // This might fail or succeed depending on implementation
        // The point is it shouldn't panic
        let _ = result;
    }

    // =========================================================================
    // Primitive Value Args Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_agent_tool_string_args() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!("just a string")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_number_args() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!(42)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_null_args() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::Value::Null).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invoke_agent_tool_bool_args() {
        let tool = InvokeAgentTool;
        let ctx = RunContext::minimal("test");

        let result = tool.call(&ctx, serde_json::json!(true)).await;
        assert!(result.is_err());
    }
}
