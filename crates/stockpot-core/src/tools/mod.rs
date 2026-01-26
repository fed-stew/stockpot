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
//! use stockpot_core::tools::registry::SpotToolRegistry;
//!
//! let registry = SpotToolRegistry::new();
//! let tools = registry.all_tools();
//! ```

// Core operations (low-level)
pub mod agent_tools;
pub mod common;
pub mod diff;
mod file_ops;
mod shell;

// Tool implementations (serdesAI wrappers)
mod delete_file_tool;
mod edit_file_tool;
mod grep_tool;
mod kill_process_tool;
mod list_files_tool;
mod list_processes_tool;
mod read_file_tool;
mod read_process_output_tool;
mod shell_tool;
mod tool_context;

// Registry
pub mod registry;

// Re-export low-level operations (for direct use)

// Re-export tool types for convenience

// Re-export registry types
pub use registry::SpotToolRegistry;

// Re-export tool context
pub use tool_context::{get_global_context, set_global_context, ToolContext};

// Re-export lenient JSON parsing utilities
pub use common::{coerce_json_types, parse_tool_args_lenient};
