//! OAuth authentication and token management.
//!
//! This module handles:
//! - Token storage in SQLite
//! - Token refresh when expired
//! - Model factory functions that load tokens from storage

mod chatgpt;
mod claude_code;
pub mod google;
mod storage;

pub use chatgpt::{get_chatgpt_model, run_chatgpt_auth, run_chatgpt_auth_with_progress};
pub use claude_code::{get_claude_code_model, run_claude_code_auth, run_claude_code_auth_with_progress};
pub use google::{get_google_model, run_google_auth, run_google_auth_with_progress};
pub use storage::TokenStorage;

/// Trait for reporting OAuth flow progress.
///
/// Implement this trait to receive progress messages from OAuth flows.
/// The default `StdoutProgress` implementation prints to stdout.
pub trait AuthProgress: Send + Sync {
    /// Report an informational message.
    fn info(&self, msg: &str);
    /// Report a success message.
    fn success(&self, msg: &str);
    /// Report a warning message.
    fn warning(&self, msg: &str);
    /// Report an error message.
    fn error(&self, msg: &str);
    /// Called with the auth URL and callback port before waiting for tokens.
    /// TUI can use this to show a dialog with the URL.
    fn on_auth_url(&self, _url: &str, _port: u16) {
        // Default: do nothing (messages already contain this info)
    }
}

/// Default progress reporter that prints to stdout.
#[derive(Default)]
pub struct StdoutProgress;

impl AuthProgress for StdoutProgress {
    fn info(&self, msg: &str) {
        println!("{}", msg);
    }

    fn success(&self, msg: &str) {
        println!("{}", msg);
    }

    fn warning(&self, msg: &str) {
        println!("{}", msg);
    }

    fn error(&self, msg: &str) {
        println!("{}", msg);
    }
}

/// Supported OAuth providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    ChatGpt,
    ClaudeCode,
    Google,
}

impl OAuthProvider {
    /// Get the provider name as stored in the database.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::ClaudeCode => "claude-code",
            Self::Google => "google",
        }
    }
}
