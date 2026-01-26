//! Terminal emulation module using alacritty_terminal and portable-pty.
//!
//! This module provides PTY-backed terminal instances that can be embedded
//! in the GUI for shell command execution with real-time output streaming.

mod instance;
mod pty;
mod security;
mod store;
mod types;

pub use instance::Terminal;
pub use pty::{
    headless_env, interactive_env, spawn_pty, spawn_user_shell, PtyConfig, PtyEvent, SpawnedPty,
};
pub use security::{validate_command, CommandValidation, RiskLevel, SAFE_COMMAND_PREFIXES};
pub use store::*;
pub use types::*;
