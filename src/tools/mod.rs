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

mod file_ops;
mod shell;
mod agent_tools;
mod common;
pub mod diff;
pub mod registry;

// Re-export low-level operations (for direct use)
pub use file_ops::{list_files, read_file, write_file, grep, apply_diff};
pub use shell::{CommandRunner, CommandResult};
pub use agent_tools::{InvokeAgentTool, ListAgentsTool, ShareReasoningTool as AgentShareReasoningTool};
pub use common::{IGNORE_PATTERNS, should_ignore};
pub use diff::{UnifiedDiff, apply_unified_diff, is_unified_diff};

// Re-export registry types for convenience
pub use registry::{
    SpotToolRegistry, ArcTool,
    ListFilesTool, ReadFileTool, EditFileTool, DeleteFileTool,
    GrepTool, RunShellCommandTool, ShareReasoningTool,
};
