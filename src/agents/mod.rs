//! Agent system for Stockpot.
//!
//! This module provides:
//! - [`SpotAgent`] trait for defining agents
//! - [`AgentManager`] for agent registry and switching
//! - Built-in agents (Stockpot, Planning, Reviewers)
//! - JSON-defined custom agents

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

mod base;
mod builtin;
mod executor;
pub mod json_agent;
mod manager;

pub use base::SpotAgent;
pub use executor::{AgentExecutor, ExecuteContext, RetryHandler};
pub use manager::{AgentInfo, AgentManager};

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

/// Agent visibility level for UI filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentVisibility {
    /// Always visible - primary agents (stockpot, planning)
    #[default]
    Main,
    /// Visible to Expert and Developer users - specialized agents (reviewers, explore)
    Sub,
    /// Only visible to Developer users - example/testing agents
    Hidden,
}

/// User experience level controlling agent visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserMode {
    /// Shows only Main agents
    #[default]
    Normal,
    /// Shows Main + Sub agents
    Expert,
    /// Shows all agents including Hidden
    Developer,
}

impl fmt::Display for UserMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Normal => "normal",
            Self::Expert => "expert",
            Self::Developer => "developer",
        };
        f.write_str(s)
    }
}

impl FromStr for UserMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "normal" => Ok(Self::Normal),
            "expert" => Ok(Self::Expert),
            "developer" => Ok(Self::Developer),
            _ => Err(format!("invalid user mode: {s}")),
        }
    }
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
