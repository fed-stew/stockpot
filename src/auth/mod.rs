//! OAuth authentication and token management.
//!
//! This module handles:
//! - Token storage in SQLite
//! - Token refresh when expired
//! - Model factory functions that load tokens from storage

mod chatgpt;
mod claude_code;
mod storage;

pub use chatgpt::{get_chatgpt_model, run_chatgpt_auth};
pub use claude_code::{get_claude_code_model, run_claude_code_auth};
pub use storage::TokenStorage;

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
