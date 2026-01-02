//! Stockpot Library
//!
//! This crate provides the core functionality for the Stockpot CLI.
//!
//! This library exposes many types for external consumers. The unused_imports
//! warning is suppressed because these are re-exports meant for library users.

#![allow(dead_code)] // Library APIs may not be used internally
#![allow(unused_imports)] // Re-exports for library consumers
//!
//! ## Main Components
//!
//! - [`agents`] - Agent system (SpotAgent trait, AgentManager, AgentExecutor)
//! - [`auth`] - OAuth authentication for ChatGPT and Claude Code
//! - [`cli`] - Command-line interface (REPL, commands, runner)
//! - [`config`] - Configuration and settings management
//! - [`db`] - SQLite database for persistence
//! - [`mcp`] - Model Context Protocol server integration
//! - [`session`] - Conversation session management
//! - [`tools`] - Tool implementations (file ops, shell, grep)
//!
//! ## Quick Start
//!
//! ```ignore
//! use stockpot::{Database, AgentExecutor, SessionManager, ModelRegistry};
//!
//! let db = Database::open()?;
//! let registry = ModelRegistry::load_from_db(&db)?;
//! let executor = AgentExecutor::new(&db, &registry);
//! let sessions = SessionManager::new();
//! ```

pub mod agents;
pub mod auth;
pub mod cli;
pub mod config;
pub mod db;
pub mod mcp;
pub mod messaging;
pub mod models;
pub mod session;
pub mod tokens;
pub mod tools;

#[cfg(feature = "gui")]
pub mod gui;

// Re-export commonly used types
pub use agents::{
    load_json_agents, AgentCapabilities, AgentExecutor, AgentManager, AgentVisibility, BoxedAgent,
    ExecutorError, ExecutorResult, JsonAgent, JsonAgentDef, SpotAgent, StreamEvent, UserMode,
};
pub use auth::{ChatGptAuth, ClaudeCodeAuth, OAuthProvider};
pub use cli::{create_reedline, SpotCompleter, SpotPrompt, COMMANDS};
pub use config::{Settings, XdgDirs};
pub use db::Database;
pub use mcp::{McpConfig, McpManager, McpServerEntry};
pub use messaging::{Message, MessageBus, MessageSender, Spinner, SpinnerHandle, TerminalRenderer};
pub use models::{ModelConfig, ModelRegistry, ModelSettings, ModelType};
pub use session::{format_relative_time, SessionData, SessionError, SessionManager, SessionMeta};
pub use tokens::{estimate_message_tokens, estimate_tokens, should_compact};
pub use tools::{
    apply_unified_diff, grep, is_unified_diff, list_files, read_file, write_file, ArcTool,
    InvokeAgentTool, ListAgentsTool, SpotToolRegistry, UnifiedDiff,
};
