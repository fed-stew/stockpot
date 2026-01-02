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

pub mod agent_tools;
mod common;
pub mod diff;
mod file_ops;
pub mod registry;
mod shell;

// Re-export low-level operations (for direct use)
pub use agent_tools::{
    InvokeAgentTool, ListAgentsTool, ShareReasoningTool as AgentShareReasoningTool,
};
pub use common::{should_ignore, IGNORE_PATTERNS};
pub use diff::{apply_unified_diff, is_unified_diff, UnifiedDiff};
pub use file_ops::{apply_diff, grep, list_files, read_file, write_file};
pub use shell::{CommandResult, CommandRunner};

// Re-export registry types for convenience
pub use registry::{
    ArcTool, DeleteFileTool, EditFileTool, GrepTool, ListFilesTool, ReadFileTool,
    RunShellCommandTool, ShareReasoningTool, SpotToolRegistry,
};
