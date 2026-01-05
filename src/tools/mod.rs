//! Tool implementations for Stockpot agents.
//!
//! Provides file operations, shell execution, and agent tools.
//!
//! ## serdesAI Tool Integration
//!
//! The [`registry`] module provides serdesAI-compatible tool implementations
//! that can be used with the agent executor:
//!
//! ```ignore
//! use stockpot::tools::registry::SpotToolRegistry;
//!
//! let registry = SpotToolRegistry::new();
//! let tools = registry.all_tools();
//! ```

// Core operations (low-level)
pub mod agent_tools;
mod common;
pub mod diff;
mod file_ops;
mod shell;

// Tool implementations (serdesAI wrappers)
mod delete_file_tool;
mod edit_file_tool;
mod grep_tool;
mod list_files_tool;
mod read_file_tool;
mod reasoning_tool;
mod shell_tool;

// Registry
pub mod registry;

// Re-export low-level operations (for direct use)
pub use agent_tools::{
    InvokeAgentTool, ListAgentsTool, ShareReasoningTool as AgentShareReasoningTool,
};
pub use common::{should_ignore, IGNORE_PATTERNS};
pub use diff::{apply_unified_diff, is_unified_diff, UnifiedDiff};
pub use file_ops::{apply_diff, grep, list_files, read_file, write_file};
pub use shell::{CommandResult, CommandRunner};

// Re-export tool types for convenience
pub use delete_file_tool::DeleteFileTool;
pub use edit_file_tool::EditFileTool;
pub use grep_tool::GrepTool;
pub use list_files_tool::ListFilesTool;
pub use read_file_tool::ReadFileTool;
pub use reasoning_tool::ShareReasoningTool;
pub use shell_tool::RunShellCommandTool;

// Re-export registry types
pub use registry::{ArcTool, SpotToolRegistry};
