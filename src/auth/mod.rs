//! OAuth authentication and token management.
//!
//! This module handles:
//! - Token storage in SQLite
//! - Token refresh when expired
//! - Model factory functions that load tokens from storage

mod storage;
mod chatgpt;
mod claude_code;

pub use storage::{TokenStorage, TokenStorageError};
pub use chatgpt::{run_chatgpt_auth, get_chatgpt_model, ChatGptAuth};
pub use claude_code::{run_claude_code_auth, get_claude_code_model, ClaudeCodeAuth};

/// Supported OAuth providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    ChatGpt,
    ClaudeCode,
}

impl OAuthProvider {
    /// Get the provider name as stored in the database.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::ClaudeCode => "claude-code",
        }
    }
}
