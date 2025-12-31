//! Agent system for Stockpot.
//!
//! This module provides:
//! - [`SpotAgent`] trait for defining agents
//! - [`AgentManager`] for agent registry and switching
//! - Built-in agents (Stockpot, Planning, Reviewers)
//! - JSON-defined custom agents

mod manager;
mod base;
mod builtin;
mod executor;
pub mod json_agent;

pub use base::{SpotAgent, BoxedAgent};
pub use manager::{AgentManager, AgentInfo, AgentError};
pub use builtin::*;
pub use executor::{
    AgentExecutor, ExecutorResult, ExecutorStreamReceiver, StreamEvent,
    ExecutorError, get_model,
};
pub use json_agent::{JsonAgent, JsonAgentDef, load_json_agents};
#[allow(deprecated)]
pub use executor::execute_agent;

/// Agent capability flags.
#[derive(Debug, Clone, Default)]
pub struct AgentCapabilities {
    /// Can execute shell commands
    pub shell: bool,
    /// Can modify files
    pub file_write: bool,
    /// Can read files
    pub file_read: bool,
    /// Can invoke sub-agents
    pub sub_agents: bool,
    /// Can use MCP tools
    pub mcp: bool,
}

impl AgentCapabilities {
    /// Full capabilities (for main stockpot agent).
    pub fn full() -> Self {
        Self {
            shell: true,
            file_write: true,
            file_read: true,
            sub_agents: true,
            mcp: true,
        }
    }

    /// Read-only capabilities (for reviewers).
    pub fn read_only() -> Self {
        Self {
            shell: false,
            file_write: false,
            file_read: true,
            sub_agents: false,
            mcp: false,
        }
    }

    /// Planning capabilities.
    pub fn planning() -> Self {
        Self {
            shell: false,
            file_write: false,
            file_read: true,
            sub_agents: true,
            mcp: false,
        }
    }
}
