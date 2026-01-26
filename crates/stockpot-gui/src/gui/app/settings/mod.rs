//! Settings panel UI components
//!
//! Split into submodules for maintainability:
//! - `tabs`: Main settings panel and tab rendering
//! - `pinned_agents`: Agent pinning configuration
//! - `models`: Model management and configuration
//! - `mcp_servers`: MCP server configuration
//! - `general`: General application settings
//! - `dialogs`: Shared dialog components

mod dialogs;
mod general;
mod mcp_servers;
mod models;
mod pinned_agents;
mod tabs;

pub(crate) use tabs::SettingsTab;
