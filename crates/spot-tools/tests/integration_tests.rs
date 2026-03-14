//! Integration tests for spot-tools public API.
//!
//! Tests cover:
//! - Tool registry creation and tool lookup
//! - Individual tools with mock files (using tempdir)
//! - Shell tool with simple commands

use spot_tools::tools::SpotToolRegistry;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::TempDir;

// =========================================================================
// Tool Registry Public API Tests
// =========================================================================

#[test]
fn test_registry_exposes_all_expected_tools() {
    let registry = SpotToolRegistry::new();
    let tool_names: HashSet<String> = registry
        .all_tools()
        .iter()
        .map(|t| t.definition().name.to_string())
        .collect();

    let expected = [
        "list_files",
        "read_file",
        "edit_file",
        "delete_file",
        "grep",
        "run_shell_command",
        "list_processes",
        "read_process_output",
        "kill_process",
    ];

    for name in expected {
        assert!(tool_names.contains(name), "Missing tool: {}", name);
    }
}

#[test]
fn test_tool_lookup_by_name() {
    let registry = SpotToolRegistry::new();

    // Single tool lookup
    let tools = registry.tools_by_name(&["read_file"]);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].definition().name, "read_file");

    // Multiple tool lookup preserves order
    let tools = registry.tools_by_name(&["grep", "edit_file", "list_files"]);
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0].definition().name, "grep");
    assert_eq!(tools[1].definition().name, "edit_file");
    assert_eq!(tools[2].definition().name, "list_files");
}

#[test]
fn test_tool_lookup_nonexistent_returns_empty() {
    let registry = SpotToolRegistry::new();
    let tools = registry.tools_by_name(&["imaginary_tool"]);
    assert!(tools.is_empty());
}

#[test]
fn test_read_only_tools_are_safe_subset() {
    let registry = SpotToolRegistry::new();
    let read_only_names: HashSet<String> = registry
        .read_only_tools()
        .iter()
        .map(|t| t.definition().name.to_string())
        .collect();

    // Should include read-safe tools
    assert!(read_only_names.contains("list_files"));
    assert!(read_only_names.contains("read_file"));
    assert!(read_only_names.contains("grep"));

    // Should NOT include mutating tools
    assert!(!read_only_names.contains("edit_file"));
    assert!(!read_only_names.contains("delete_file"));
    assert!(!read_only_names.contains("run_shell_command"));
}

#[test]
fn test_adding_external_tools_extends_registry() {
    let mut registry = SpotToolRegistry::new();
    let base_count = registry.all_tools().len();

    // Add an external tool (reusing an existing type for simplicity)
    let extra_tool = Arc::new(spot_tools::tools::SpotToolRegistry::default().list_files);
    registry.add_tool(extra_tool);

    assert_eq!(registry.all_tools().len(), base_count + 1);
}

#[test]
fn test_tool_definitions_are_well_formed() {
    let registry = SpotToolRegistry::new();

    for tool in registry.all_tools() {
        let def = tool.definition();
        // Every tool must have a non-empty name
        assert!(!def.name.is_empty(), "Tool name is empty");
        // Every tool must have a description
        assert!(
            !def.description.is_empty(),
            "Tool '{}' has empty description",
            def.name
        );
        // Parameters must be a JSON object
        let params = def.parameters();
        assert!(
            params.is_object(),
            "Tool '{}' parameters is not a JSON object",
            def.name
        );
    }
}

// =========================================================================
// Individual Tool Tests with Tempdir
// =========================================================================

#[tokio::test]
async fn test_read_file_tool_with_tempdir() {
    use serdes_ai_tools::{RunContext, Tool};

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test_file.txt");
    std::fs::write(&file_path, "Hello, world!\nSecond line.").unwrap();

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "file_path": file_path.to_string_lossy()
    });

    let result = registry.read_file.call(&ctx, args).await;
    assert!(result.is_ok(), "read_file should succeed: {:?}", result);
}

#[tokio::test]
async fn test_list_files_tool_with_tempdir() {
    use serdes_ai_tools::{RunContext, Tool};

    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("file1.txt"), "content1").unwrap();
    std::fs::write(dir.path().join("file2.rs"), "fn main() {}").unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "path": dir.path().to_string_lossy()
    });

    let result = registry.list_files.call(&ctx, args).await;
    assert!(result.is_ok(), "list_files should succeed: {:?}", result);
}

#[tokio::test]
async fn test_edit_file_tool_with_tempdir() {
    use serdes_ai_tools::{RunContext, Tool};

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("editable.txt");
    std::fs::write(&file_path, "old content here").unwrap();

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "file_path": file_path.to_string_lossy(),
        "content": "new content here"
    });

    let result = registry.edit_file.call(&ctx, args).await;
    assert!(result.is_ok(), "edit_file should succeed: {:?}", result);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("new content here"));
}

#[tokio::test]
async fn test_delete_file_tool_with_tempdir() {
    use serdes_ai_tools::{RunContext, Tool};

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("to_delete.txt");
    std::fs::write(&file_path, "delete me").unwrap();
    assert!(file_path.exists());

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "file_path": file_path.to_string_lossy()
    });

    let result = registry.delete_file.call(&ctx, args).await;
    assert!(result.is_ok(), "delete_file should succeed: {:?}", result);
    assert!(!file_path.exists());
}

#[tokio::test]
async fn test_grep_tool_with_tempdir() {
    use serdes_ai_tools::{RunContext, Tool};

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("search_me.txt"),
        "needle in haystack\nmore hay\nneedle again",
    )
    .unwrap();

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "pattern": "needle",
        "path": dir.path().to_string_lossy()
    });

    let result = registry.grep.call(&ctx, args).await;
    assert!(result.is_ok(), "grep should succeed: {:?}", result);
}

// =========================================================================
// Shell Tool Tests
// =========================================================================

#[tokio::test]
async fn test_shell_tool_echo() {
    use serdes_ai_tools::{RunContext, Tool};

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "command": "echo hello"
    });

    let result = registry.run_shell_command.call(&ctx, args).await;
    assert!(result.is_ok(), "shell tool should succeed: {:?}", result);
}

#[tokio::test]
async fn test_shell_tool_with_working_directory() {
    use serdes_ai_tools::{RunContext, Tool};

    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("marker.txt"), "found").unwrap();

    let registry = SpotToolRegistry::new();
    let ctx = RunContext::minimal("test-model");

    let args = serde_json::json!({
        "command": "ls marker.txt",
        "working_directory": dir.path().to_string_lossy()
    });

    let result = registry.run_shell_command.call(&ctx, args).await;
    assert!(
        result.is_ok(),
        "shell tool with cwd should succeed: {:?}",
        result
    );
}

// =========================================================================
// Tool Arc/Send/Sync Tests
// =========================================================================

#[test]
fn test_tools_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    let registry = SpotToolRegistry::new();
    for tool in registry.all_tools() {
        // Verifies that each ArcTool is Send + Sync at compile time
        let _ = std::thread::spawn(move || {
            let _ = tool.definition();
        });
    }

    assert_send_sync::<SpotToolRegistry>();
}
