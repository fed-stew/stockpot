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
use super::kill_process_tool::KillProcessTool;
use super::list_files_tool::ListFilesTool;
use super::list_processes_tool::ListProcessesTool;
use super::read_file_tool::ReadFileTool;
use super::read_process_output_tool::ReadProcessOutputTool;
use super::reasoning_tool::ShareReasoningTool;
use super::shell_tool::RunShellCommandTool;

/// Arc-wrapped tool for shared ownership.
pub type ArcTool = Arc<dyn Tool + Send + Sync>;

// Re-export all tool types for convenience

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
    // Process management tools
    pub list_processes: ListProcessesTool,
    pub read_process_output: ReadProcessOutputTool,
    pub kill_process: KillProcessTool,
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
            // Process management tools
            Arc::new(self.list_processes.clone()),
            Arc::new(self.read_process_output.clone()),
            Arc::new(self.kill_process.clone()),
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
                "list_processes" => tools.push(Arc::new(self.list_processes.clone())),
                "read_process_output" => tools.push(Arc::new(self.read_process_output.clone())),
                "kill_process" => tools.push(Arc::new(self.kill_process.clone())),
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
    use std::collections::HashSet;

    // =========================================================================
    // Registry Creation Tests
    // =========================================================================

    #[test]
    fn test_registry_creation() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.all_tools().len(), 12);
        assert_eq!(registry.definitions().len(), 12);
    }

    #[test]
    fn test_registry_default_trait() {
        let registry = SpotToolRegistry::default();
        assert_eq!(registry.all_tools().len(), 12);
    }

    #[test]
    fn test_registry_new_equals_default() {
        let new_registry = SpotToolRegistry::new();
        let default_registry = SpotToolRegistry::default();

        let new_names: HashSet<_> = new_registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();
        let default_names: HashSet<_> = default_registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        assert_eq!(new_names, default_names);
    }

    // =========================================================================
    // all_tools Tests
    // =========================================================================

    #[test]
    fn test_all_tools_returns_correct_count() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.all_tools().len(), 12);
    }

    #[test]
    fn test_all_tools_contains_expected_tools() {
        let registry = SpotToolRegistry::new();
        let tool_names: HashSet<_> = registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let expected = [
            "list_files",
            "read_file",
            "edit_file",
            "delete_file",
            "grep",
            "run_shell_command",
            "share_your_reasoning",
            "invoke_agent",
            "list_agents",
        ];

        for name in expected {
            assert!(tool_names.contains(name), "Missing tool: {}", name);
        }
    }

    #[test]
    fn test_all_tools_no_duplicates() {
        let registry = SpotToolRegistry::new();
        let tool_names: Vec<_> = registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let unique_names: HashSet<_> = tool_names.iter().collect();
        assert_eq!(
            tool_names.len(),
            unique_names.len(),
            "Duplicate tools found"
        );
    }

    #[test]
    fn test_all_tools_returns_arc_wrapped() {
        let registry = SpotToolRegistry::new();
        let tools = registry.all_tools();

        // Each tool should be independently usable via Arc
        for tool in &tools {
            let _def = tool.definition(); // Should work through Arc
            assert!(!tool.definition().name.is_empty());
        }
    }

    #[test]
    fn test_all_tools_can_be_called_multiple_times() {
        let registry = SpotToolRegistry::new();

        let tools1 = registry.all_tools();
        let tools2 = registry.all_tools();

        assert_eq!(tools1.len(), tools2.len());

        let names1: HashSet<_> = tools1.iter().map(|t| t.definition().name.clone()).collect();
        let names2: HashSet<_> = tools2.iter().map(|t| t.definition().name.clone()).collect();
        assert_eq!(names1, names2);
    }

    // =========================================================================
    // definitions Tests
    // =========================================================================

    #[test]
    fn test_definitions_returns_correct_count() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.definitions().len(), 12);
    }

    #[test]
    fn test_definitions_match_all_tools() {
        let registry = SpotToolRegistry::new();

        let tool_names: HashSet<_> = registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let def_names: HashSet<_> = registry
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();

        assert_eq!(tool_names, def_names);
    }

    #[test]
    fn test_definitions_have_valid_structure() {
        let registry = SpotToolRegistry::new();

        for def in registry.definitions() {
            assert!(!def.name.is_empty(), "Definition name is empty");
            assert!(
                !def.description.is_empty(),
                "Definition description is empty"
            );
            // Parameters should be a valid JSON object
            assert!(
                def.parameters().is_object(),
                "Parameters should be an object"
            );
        }
    }

    // =========================================================================
    // tools_by_name Tests
    // =========================================================================

    #[test]
    fn test_tools_by_name() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["read_file", "edit_file"]);
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_tools_by_name_single_tool() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["grep"]);

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].definition().name, "grep");
    }

    #[test]
    fn test_tools_by_name_all_tools() {
        let registry = SpotToolRegistry::new();
        let names = [
            "list_files",
            "read_file",
            "edit_file",
            "delete_file",
            "grep",
            "run_shell_command",
            "share_your_reasoning",
            "invoke_agent",
            "list_agents",
        ];

        let tools = registry.tools_by_name(&names);
        assert_eq!(tools.len(), 9);
    }

    #[test]
    fn test_tools_by_name_empty_list() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&[]);
        assert!(tools.is_empty());
    }

    #[test]
    fn test_tools_by_name_unknown_tool() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["nonexistent_tool"]);
        assert!(tools.is_empty());
    }

    #[test]
    fn test_tools_by_name_mixed_valid_invalid() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["read_file", "nonexistent", "grep"]);

        assert_eq!(tools.len(), 2);

        let names: HashSet<_> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(names.contains("read_file"));
        assert!(names.contains("grep"));
    }

    #[test]
    fn test_tools_by_name_preserves_order() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["grep", "read_file", "list_files"]);

        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].definition().name, "grep");
        assert_eq!(tools[1].definition().name, "read_file");
        assert_eq!(tools[2].definition().name, "list_files");
    }

    #[test]
    fn test_tools_by_name_duplicate_names() {
        let registry = SpotToolRegistry::new();
        let tools = registry.tools_by_name(&["read_file", "read_file", "read_file"]);

        // Each occurrence should add a tool
        assert_eq!(tools.len(), 3);
        for tool in &tools {
            assert_eq!(tool.definition().name, "read_file");
        }
    }

    #[test]
    fn test_tools_by_name_case_sensitive() {
        let registry = SpotToolRegistry::new();

        let tools_lower = registry.tools_by_name(&["read_file"]);
        let tools_upper = registry.tools_by_name(&["READ_FILE"]);
        let tools_mixed = registry.tools_by_name(&["Read_File"]);

        assert_eq!(tools_lower.len(), 1);
        assert!(tools_upper.is_empty());
        assert!(tools_mixed.is_empty());
    }

    // =========================================================================
    // read_only_tools Tests
    // =========================================================================

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
    fn test_read_only_tools_count() {
        let registry = SpotToolRegistry::new();
        let tools = registry.read_only_tools();
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn test_read_only_tools_exact_set() {
        let registry = SpotToolRegistry::new();
        let tool_names: HashSet<_> = registry
            .read_only_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let expected: HashSet<_> = [
            "list_files".to_string(),
            "read_file".to_string(),
            "grep".to_string(),
            "share_your_reasoning".to_string(),
        ]
        .into_iter()
        .collect();

        assert_eq!(tool_names, expected);
    }

    #[test]
    fn test_read_only_tools_excludes_dangerous() {
        let registry = SpotToolRegistry::new();
        let tool_names: HashSet<_> = registry
            .read_only_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        assert!(!tool_names.contains("edit_file"));
        assert!(!tool_names.contains("delete_file"));
        assert!(!tool_names.contains("run_shell_command"));
        assert!(!tool_names.contains("invoke_agent"));
        assert!(!tool_names.contains("list_agents"));
    }

    // =========================================================================
    // file_tools Tests
    // =========================================================================

    #[test]
    fn test_file_tools_count() {
        let registry = SpotToolRegistry::new();
        let tools = registry.file_tools();
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn test_file_tools_exact_set() {
        let registry = SpotToolRegistry::new();
        let tool_names: HashSet<_> = registry
            .file_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let expected: HashSet<_> = [
            "list_files".to_string(),
            "read_file".to_string(),
            "edit_file".to_string(),
            "delete_file".to_string(),
            "grep".to_string(),
        ]
        .into_iter()
        .collect();

        assert_eq!(tool_names, expected);
    }

    #[test]
    fn test_file_tools_excludes_non_file_tools() {
        let registry = SpotToolRegistry::new();
        let tool_names: HashSet<_> = registry
            .file_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        assert!(!tool_names.contains("run_shell_command"));
        assert!(!tool_names.contains("share_your_reasoning"));
        assert!(!tool_names.contains("invoke_agent"));
        assert!(!tool_names.contains("list_agents"));
    }

    // =========================================================================
    // Tool Definition Validation Tests
    // =========================================================================

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let def = tool.definition();
            assert!(!def.name.is_empty(), "Tool name is empty");
            assert!(!def.description.is_empty(), "Tool description is empty");
        }
    }

    #[test]
    fn test_tool_names_are_snake_case() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let name = tool.definition().name;
            assert!(
                name.chars().all(|c| c.is_lowercase() || c == '_'),
                "Tool name '{}' is not snake_case",
                name
            );
        }
    }

    #[test]
    fn test_tool_descriptions_are_meaningful() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let def = tool.definition();
            // Description should be at least 10 characters
            assert!(
                def.description.len() >= 10,
                "Tool '{}' has too short description: '{}'",
                def.name,
                def.description
            );
        }
    }

    #[test]
    fn test_tool_parameters_are_objects() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let def = tool.definition();
            let params = def.parameters();
            assert!(
                params.is_object(),
                "Tool '{}' parameters should be an object",
                def.name
            );
        }
    }

    #[test]
    fn test_tool_parameters_have_type_object() {
        let registry = SpotToolRegistry::new();
        for tool in registry.all_tools() {
            let def = tool.definition();
            let params = def.parameters();
            let obj = params.as_object().unwrap();

            if let Some(type_val) = obj.get("type") {
                assert_eq!(
                    type_val.as_str(),
                    Some("object"),
                    "Tool '{}' parameters type should be 'object'",
                    def.name
                );
            }
        }
    }

    // =========================================================================
    // Specific Tool Verification Tests
    // =========================================================================

    #[test]
    fn test_list_files_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.list_files.definition().name, "list_files");
    }

    #[test]
    fn test_read_file_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.read_file.definition().name, "read_file");
    }

    #[test]
    fn test_edit_file_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.edit_file.definition().name, "edit_file");
    }

    #[test]
    fn test_delete_file_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.delete_file.definition().name, "delete_file");
    }

    #[test]
    fn test_grep_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.grep.definition().name, "grep");
    }

    #[test]
    fn test_run_shell_command_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(
            registry.run_shell_command.definition().name,
            "run_shell_command"
        );
    }

    #[test]
    fn test_share_reasoning_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(
            registry.share_reasoning.definition().name,
            "share_your_reasoning"
        );
    }

    #[test]
    fn test_invoke_agent_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.invoke_agent.definition().name, "invoke_agent");
    }

    #[test]
    fn test_list_agents_tool_exists() {
        let registry = SpotToolRegistry::new();
        assert_eq!(registry.list_agents.definition().name, "list_agents");
    }

    // =========================================================================
    // Tool Subset Relationship Tests
    // =========================================================================

    #[test]
    fn test_read_only_is_subset_of_all() {
        let registry = SpotToolRegistry::new();

        let all_names: HashSet<_> = registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let read_only_names: HashSet<_> = registry
            .read_only_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        assert!(read_only_names.is_subset(&all_names));
    }

    #[test]
    fn test_file_tools_is_subset_of_all() {
        let registry = SpotToolRegistry::new();

        let all_names: HashSet<_> = registry
            .all_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let file_names: HashSet<_> = registry
            .file_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        assert!(file_names.is_subset(&all_names));
    }

    #[test]
    fn test_read_only_and_file_tools_overlap() {
        let registry = SpotToolRegistry::new();

        let read_only_names: HashSet<_> = registry
            .read_only_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let file_names: HashSet<_> = registry
            .file_tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();

        let intersection: HashSet<_> = read_only_names.intersection(&file_names).collect();

        // list_files, read_file, grep should be in both
        assert!(intersection.contains(&"list_files".to_string()));
        assert!(intersection.contains(&"read_file".to_string()));
        assert!(intersection.contains(&"grep".to_string()));
    }

    // =========================================================================
    // Debug Trait Tests
    // =========================================================================

    #[test]
    fn test_registry_implements_debug() {
        let registry = SpotToolRegistry::new();
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("SpotToolRegistry"));
    }

    // =========================================================================
    // Arc Cloning Tests
    // =========================================================================

    #[test]
    fn test_arc_tools_can_be_cloned() {
        let registry = SpotToolRegistry::new();
        let tools = registry.all_tools();

        for tool in &tools {
            let cloned = Arc::clone(tool);
            assert_eq!(tool.definition().name, cloned.definition().name);
        }
    }

    #[test]
    fn test_tools_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        let registry = SpotToolRegistry::new();
        let tools = registry.all_tools();

        for tool in tools {
            // This compiles only if ArcTool is Send + Sync
            let _ = std::thread::spawn(move || {
                let _ = tool.definition();
            });
        }
    }
}
