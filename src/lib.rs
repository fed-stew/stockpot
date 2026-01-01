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
    AgentManager, SpotAgent, BoxedAgent, AgentExecutor, ExecutorResult, 
    ExecutorError, StreamEvent, AgentCapabilities,
    JsonAgent, JsonAgentDef, load_json_agents,
};
pub use auth::{ChatGptAuth, ClaudeCodeAuth, OAuthProvider};
pub use config::{Settings, XdgDirs};
pub use db::Database;
pub use mcp::{McpManager, McpConfig, McpServerEntry};
pub use messaging::{Message, MessageBus, MessageSender, TerminalRenderer, Spinner, SpinnerHandle};
pub use session::{SessionManager, SessionData, SessionMeta, SessionError, format_relative_time};
pub use models::{ModelRegistry, ModelSettings, ModelConfig, ModelType};
pub use tokens::{estimate_tokens, estimate_message_tokens, should_compact};
pub use cli::{SpotCompleter, SpotPrompt, create_reedline, COMMANDS};
pub use tools::{
    SpotToolRegistry, ArcTool,
    list_files, read_file, write_file, grep,
    UnifiedDiff, apply_unified_diff, is_unified_diff,
    InvokeAgentTool, ListAgentsTool,
};
