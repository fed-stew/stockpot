//! Tool implementations and terminal emulation for Spot agents.
//!
//! Provides file operations, shell execution, and terminal management.
//!
//! ## serdesAI Tool Integration
//!
//! The [`tools`] module provides serdesAI-compatible tool implementations
//! that can be used with the agent executor:
//!
//! ```ignore
//! use spot_tools::tools::SpotToolRegistry;
//!
//! let registry = SpotToolRegistry::new();
//! let tools = registry.all_tools();
//! ```

pub mod terminal;
pub mod tools;
