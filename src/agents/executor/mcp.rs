//! MCP (Model Context Protocol) tool executor.
//!
//! Provides `McpToolExecutor` which wraps MCP tools to work with
//! serdesAI's tool execution interface.

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use serdes_ai_tools::{RunContext, Tool, ToolDefinition, ToolError, ToolReturn};

use crate::mcp::McpManager;

/// Tool executor that calls MCP server tools.
///
/// Note: We use a raw pointer to McpManager because we can't easily
/// share Arc across async boundaries here. The pointer is valid for
/// the duration of the executor run.
pub(super) struct McpToolExecutor {
    pub server_name: String,
    pub tool_name: String,
    pub mcp_manager_ptr: *const McpManager,
}

// Safety: The pointer is only used during a single executor run
// where the McpManager is guaranteed to outlive the tool executor.
unsafe impl Send for McpToolExecutor {}
unsafe impl Sync for McpToolExecutor {}

#[async_trait]
impl Tool for McpToolExecutor {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            self.tool_name.clone(),
            format!("MCP tool from {}", self.server_name),
        )
    }

    async fn call(&self, _ctx: &RunContext<()>, args: JsonValue) -> Result<ToolReturn, ToolError> {
        // Safety: The McpManager outlives this executor
        let manager = unsafe { &*self.mcp_manager_ptr };

        match manager
            .call_tool(&self.server_name, &self.tool_name, args)
            .await
        {
            Ok(result) => {
                // Convert MCP result to ToolReturn
                if result.is_error {
                    let error_msg = result
                        .content
                        .first()
                        .map(|c| match c {
                            serdes_ai_mcp::ToolResultContent::Text { text } => text.clone(),
                            _ => "MCP tool error".to_string(),
                        })
                        .unwrap_or_else(|| "Unknown error".to_string());
                    Ok(ToolReturn::error(error_msg))
                } else {
                    let text = result
                        .content
                        .into_iter()
                        .filter_map(|c| match c {
                            serdes_ai_mcp::ToolResultContent::Text { text } => Some(text),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(ToolReturn::text(text))
                }
            }
            Err(e) => Err(ToolError::ExecutionFailed {
                message: e.to_string(),
                retryable: false,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn mcp_tool_executor_definition_name() {
        // Use null pointer - safe as long as we don't call()
        let executor = McpToolExecutor {
            server_name: "filesystem".to_string(),
            tool_name: "read_file".to_string(),
            mcp_manager_ptr: ptr::null(),
        };

        let def = executor.definition();
        assert_eq!(def.name, "read_file");
    }

    #[test]
    fn mcp_tool_executor_definition_description() {
        let executor = McpToolExecutor {
            server_name: "github-mcp".to_string(),
            tool_name: "list_issues".to_string(),
            mcp_manager_ptr: ptr::null(),
        };

        let def = executor.definition();
        assert_eq!(def.description, "MCP tool from github-mcp");
    }

    #[test]
    fn mcp_tool_executor_stores_server_and_tool_names() {
        let executor = McpToolExecutor {
            server_name: "my-server".to_string(),
            tool_name: "my-tool".to_string(),
            mcp_manager_ptr: ptr::null(),
        };

        assert_eq!(executor.server_name, "my-server");
        assert_eq!(executor.tool_name, "my-tool");
    }

    // Note: call() tests require a running MCP server, skipped for unit tests.
    // Integration tests should cover MCP tool execution.
}
