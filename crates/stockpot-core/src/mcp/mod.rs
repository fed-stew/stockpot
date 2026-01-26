//! MCP (Model Context Protocol) integration for Stockpot.
//!
//! This module provides:
//! - Configuration loading from `~/.stockpot/mcp_servers.json`
//! - MCP server lifecycle management (start/stop)
//! - Integration with the agent executor via McpToolset
//!
//! ## Configuration File Format
//!
//! The MCP servers are configured in `~/.stockpot/mcp_servers.json`:
//!
//! ```json
//! {
//!   "servers": {
//!     "filesystem": {
//!       "command": "npx",
//!       "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"]
//!     },
//!     "github": {
//!       "command": "npx",
//!       "args": ["-y", "@modelcontextprotocol/server-github"],
//!       "env": {
//!         "GITHUB_TOKEN": "${GITHUB_TOKEN}"
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use stockpot::mcp::McpManager;
//!
//! let mut manager = McpManager::new();
//! manager.load_config()?;
//!
//! // Start all configured servers
//! manager.start_all().await?;
//!
//! // Get toolsets for the agent
//! let toolsets = manager.toolsets();
//! ```

mod config;
mod manager;

pub use config::{McpConfig, McpServerEntry};
pub use manager::McpManager;
