//! Tool registry for Stockpot agents.
//!
//! This module provides the SpotToolRegistry which aggregates all available
//! serdesAI-compatible tool implementations.

use std::sync::Arc;

use serdes_ai_tools::Tool;

use super::agent_tools::{InvokeAgentTool, ListAgentsTool};
use super::delete_file_tool::DeleteFileTool;
use super::edit_file_tool::EditFileTool;
use super::grep_tool::GrepTool;
use super::list_files_tool::ListFilesTool;
use super::read_file_tool::ReadFileTool;
use super::reasoning_tool::ShareReasoningTool;
use super::shell_tool::RunShellCommandTool;

/// Arc-wrapped tool for shared ownership.
pub type ArcTool = Arc<dyn Tool + Send + Sync>;

// Re-export all tool types for convenience
pub use super::delete_file_tool::DeleteFileTool as DeleteFileToolType;
pub use super::edit_file_tool::EditFileTool as EditFileToolType;
pub use super::grep_tool::GrepTool as GrepToolType;
pub use super::list_files_tool::ListFilesTool as ListFilesToolType;
pub use super::read_file_tool::ReadFileTool as ReadFileToolType;
pub use super::reasoning_tool::ShareReasoningTool as ShareReasoningToolType;
pub use super::shell_tool::RunShellCommandTool as RunShellCommandToolType;

/// Registry holding all available Stockpot tools.
///
/// This provides a convenient way to create and access all tools
/// for use with serdesAI agents.
///
/// # Example
///
/// ```ignore
/// use stockpot::tools::registry::SpotToolRegistry;
///
/// let registry = SpotToolRegistry::new();
/// let tools = registry.all_tools();
/// ```
#[derive(Debug, Default)]
pub struct SpotToolRegistry {
    pub list_files: ListFilesTool,
    pub read_file: ReadFileTool,
    pub edit_file: EditFileTool,
    pub delete_file: DeleteFileTool,
    pub grep: GrepTool,
    pub run_shell_command: RunShellCommandTool,
    pub share_reasoning: ShareReasoningTool,
    pub invoke_agent: InvokeAgentTool,
    pub list_agents: ListAgentsTool,
}

impl SpotToolRegistry {
    /// Create a new registry with all tools.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all tools as Arc-wrapped trait objects for shared ownership.
    pub fn all_tools(&self) -> Vec<ArcTool> {
        vec![
            Arc::new(self.list_files.clone()),
            Arc::new(self.read_file.clone()),
            Arc::new(self.edit_file.clone()),
            Arc::new(self.delete_file.clone()),
            Arc::new(self.grep.clone()),
            Arc::new(self.run_shell_command.clone()),
            Arc::new(self.share_reasoning.clone()),
            Arc::new(self.invoke_agent.clone()),
            Arc::new(self.list_agents.clone()),
        ]
    }

    /// Get tool definitions for all tools.
    pub fn definitions(&self) -> Vec<serdes_ai_tools::ToolDefinition> {
        self.all_tools().iter().map(|t| t.definition()).collect()
    }

    /// Get a subset of tools by name.
    pub fn tools_by_name(&self, names: &[&str]) -> Vec<ArcTool> {
        let mut tools: Vec<ArcTool> = Vec::new();

        for name in names {
            match *name {
                "list_files" => tools.push(Arc::new(self.list_files.clone())),
                "read_file" => tools.push(Arc::new(self.read_file.clone())),
                "edit_file" => tools.push(Arc::new(self.edit_file.clone())),
                "delete_file" => tools.push(Arc::new(self.delete_file.clone())),
                "grep" => tools.push(Arc::new(self.grep.clone())),
                "run_shell_command" => tools.push(Arc::new(self.run_shell_command.clone())),
                "share_your_reasoning" => tools.push(Arc::new(self.share_reasoning.clone())),
                "invoke_agent" => tools.push(Arc::new(self.invoke_agent.clone())),
                "list_agents" => tools.push(Arc::new(self.list_agents.clone())),
                _ => {} // Unknown tool, skip
            }
        }

        tools
    }

    /// Get read-only tools (safe for reviewers and planning agents).
    pub fn read_only_tools(&self) -> Vec<ArcTool> {
        vec![
            Arc::new(self.list_files.clone()),
            Arc::new(self.read_file.clone()),
            Arc::new(self.grep.clone()),
            Arc::new(self.share_reasoning.clone()),
        ]
    }

    /// Get file operation tools only.
    pub fn file_tools(&self) -> Vec<ArcTool> {
        vec![
            Arc::new(self.list_files.clone()),
            Arc::new(self.read_file.clone()),
            Arc::new(self.edit_file.clone()),
            Arc::new(self.delete_file.clone()),
            Arc::new(self.grep.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.all_tools().len(), 9);
        assert_eq!(registry.definitions().len(), 9);
    }

    #[test]
    fn test_tools_by_name() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["read_file", "edit_file"]);
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_read_only_tools() {
        let registry = SpotToolRegistry::new();
        let tools = registry.read_only_tools();
        // Should not include edit_file, delete_file, run_shell_command
        for tool in &tools {
            let name = tool.definition().name;
            assert!(
                name == "list_files"
                    || name == "read_file"
                    || name == "grep"
                    || name == "share_your_reasoning",
                "Unexpected tool in read_only: {}",
                name
            );
        }
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let def = tool.definition();
            assert!(!def.name.is_empty(), "Tool name is empty");
            assert!(!def.description.is_empty(), "Tool description is empty");
        }
    }
}
