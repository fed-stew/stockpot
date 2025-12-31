//! Session management for Stockpot.
//!
//! This module handles saving and loading conversation sessions,
//! allowing users to persist and resume conversations.
//!
//! ## Storage Format
//!
//! Sessions are stored in `~/.stockpot/sessions/` as:
//! - `{name}.json` - Serialized message history
//! - `{name}_meta.json` - Session metadata
//!
//! ## Usage
//!
//! ```ignore
//! use stockpot::session::SessionManager;
//!
//! let manager = SessionManager::new();
//! 
//! // Save a session
//! manager.save("my-project", &messages, "stockpot", "gpt-4o")?;
//!
//! // List sessions
//! for session in manager.list()? {
//!     println!("{}: {} messages", session.name, session.message_count);
//! }
//!
//! // Load a session
//! let (messages, meta) = manager.load("my-project")?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serdes_ai_core::ModelRequest;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for session operations.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Invalid session name: {0}")]
    InvalidName(String),
}

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    /// Session name.
    pub name: String,
    
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
    
    /// Number of messages in the session.
    pub message_count: usize,
    
    /// Estimated token count.
    pub token_estimate: usize,
    
    /// Agent used in the session.
    pub agent: String,
    
    /// Model used in the session.
    pub model: String,
    
    /// Optional description/summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl SessionMeta {
    /// Create new session metadata.
    pub fn new(name: &str, agent: &str, model: &str) -> Self {
        let now = Utc::now();
        Self {
            name: name.to_string(),
            created_at: now,
            updated_at: now,
            message_count: 0,
            token_estimate: 0,
            agent: agent.to_string(),
            model: model.to_string(),
            description: None,
        }
    }

    /// Update metadata for current state.
    pub fn update(&mut self, messages: &[ModelRequest]) {
        self.updated_at = Utc::now();
        self.message_count = messages.len();
        self.token_estimate = estimate_tokens(messages);
    }
}

/// Session data stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Session metadata.
    pub meta: SessionMeta,
    
    /// Message history.
    pub messages: Vec<ModelRequest>,
}

impl SessionData {
    /// Create a new session.
    pub fn new(name: &str, agent: &str, model: &str) -> Self {
        Self {
            meta: SessionMeta::new(name, agent, model),
            messages: Vec::new(),
        }
    }

    /// Update with new messages.
    pub fn update(&mut self, messages: Vec<ModelRequest>) {
        self.messages = messages;
        self.meta.update(&self.messages);
    }
}

/// Session manager for saving and loading sessions.
pub struct SessionManager {
    /// Base directory for sessions.
    sessions_dir: PathBuf,
    
    /// Maximum number of sessions to keep (0 = unlimited).
    max_sessions: usize,
}

impl SessionManager {
    /// Create a new session manager with default settings.
    pub fn new() -> Self {
        let sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".stockpot")
            .join("sessions");
        
        Self {
            sessions_dir,
            max_sessions: 50, // Keep last 50 sessions by default
        }
    }

    /// Create with custom directory.
    pub fn with_dir(dir: impl AsRef<Path>) -> Self {
        Self {
            sessions_dir: dir.as_ref().to_path_buf(),
            max_sessions: 50,
        }
    }

    /// Set maximum number of sessions to keep.
    pub fn with_max_sessions(mut self, max: usize) -> Self {
        self.max_sessions = max;
        self
    }

    /// Ensure the sessions directory exists.
    fn ensure_dir(&self) -> Result<(), SessionError> {
        fs::create_dir_all(&self.sessions_dir)?;
        Ok(())
    }

    /// Get path for a session file.
    fn session_path(&self, name: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", name))
    }

    /// Validate session name.
    fn validate_name(name: &str) -> Result<(), SessionError> {
        if name.is_empty() {
            return Err(SessionError::InvalidName("Name cannot be empty".to_string()));
        }
        
        // Only allow alphanumeric, dash, underscore
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(SessionError::InvalidName(
                "Name can only contain letters, numbers, dashes, and underscores".to_string()
            ));
        }
        
        Ok(())
    }

    /// Save a session.
    pub fn save(
        &self,
        name: &str,
        messages: &[ModelRequest],
        agent: &str,
        model: &str,
    ) -> Result<SessionMeta, SessionError> {
        Self::validate_name(name)?;
        self.ensure_dir()?;
        
        let path = self.session_path(name);
        
        // Load existing or create new
        let mut session = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str::<SessionData>(&content)?
        } else {
            SessionData::new(name, agent, model)
        };
        
        // Update with new messages
        session.update(messages.to_vec());
        session.meta.agent = agent.to_string();
        session.meta.model = model.to_string();
        
        // Write to disk
        let content = serde_json::to_string_pretty(&session)?;
        fs::write(&path, content)?;
        
        // Cleanup old sessions if needed
        self.cleanup()?;
        
        Ok(session.meta)
    }

    /// Load a session.
    pub fn load(&self, name: &str) -> Result<SessionData, SessionError> {
        Self::validate_name(name)?;
        
        let path = self.session_path(name);
        
        if !path.exists() {
            return Err(SessionError::NotFound(name.to_string()));
        }
        
        let content = fs::read_to_string(&path)?;
        let session: SessionData = serde_json::from_str(&content)?;
        
        Ok(session)
    }

    /// List all sessions.
    pub fn list(&self) -> Result<Vec<SessionMeta>, SessionError> {
        self.ensure_dir()?;
        
        let mut sessions = Vec::new();
        
        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                // Skip metadata files (we read from main file)
                if let Some(stem) = path.file_stem() {
                    let name = stem.to_string_lossy();
                    if name.ends_with("_meta") {
                        continue;
                    }
                }
                
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<SessionData>(&content) {
                        sessions.push(session.meta);
                    }
                }
            }
        }
        
        // Sort by updated_at descending (most recent first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(sessions)
    }

    /// Delete a session.
    pub fn delete(&self, name: &str) -> Result<(), SessionError> {
        Self::validate_name(name)?;
        
        let path = self.session_path(name);
        
        if !path.exists() {
            return Err(SessionError::NotFound(name.to_string()));
        }
        
        fs::remove_file(path)?;
        
        Ok(())
    }

    /// Check if a session exists.
    pub fn exists(&self, name: &str) -> bool {
        Self::validate_name(name).is_ok() && self.session_path(name).exists()
    }

    /// Generate a unique session name.
    pub fn generate_name(&self, prefix: &str) -> String {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let base_name = format!("{}-{}", prefix, timestamp);
        
        if !self.exists(&base_name) {
            return base_name;
        }
        
        // Add suffix if collision
        for i in 1..100 {
            let name = format!("{}-{}", base_name, i);
            if !self.exists(&name) {
                return name;
            }
        }
        
        // Fallback with random suffix
        format!("{}-{}", base_name, rand_suffix())
    }

    /// Cleanup old sessions beyond the limit.
    fn cleanup(&self) -> Result<(), SessionError> {
        if self.max_sessions == 0 {
            return Ok(()); // Unlimited
        }
        
        let sessions = self.list()?;
        
        if sessions.len() > self.max_sessions {
            // Delete oldest sessions
            for session in sessions.iter().skip(self.max_sessions) {
                let _ = self.delete(&session.name);
            }
        }
        
        Ok(())
    }

    /// Get session directory path.
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate token count for messages.
/// 
/// Uses a simple heuristic: ~4 characters per token.
fn estimate_tokens(messages: &[ModelRequest]) -> usize {
    let mut total_chars = 0;
    
    for msg in messages {
        // Estimate based on content - this is a rough approximation
        // Real token counting would require the model's tokenizer
        total_chars += estimate_message_chars(msg);
    }
    
    // Rough estimate: 4 chars per token
    total_chars / 4
}

/// Estimate character count for a single message.
fn estimate_message_chars(msg: &ModelRequest) -> usize {
    // ModelRequest is complex, let's serialize and count
    serde_json::to_string(msg)
        .map(|s| s.len())
        .unwrap_or(100) // Default estimate
}

/// Generate a random suffix for names.
fn rand_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos % 0xFFFF)
}

/// Format a relative time string.
pub fn format_relative_time(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    
    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        let mins = diff.num_minutes();
        format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if diff.num_hours() < 24 {
        let hours = diff.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if diff.num_days() < 7 {
        let days = diff.num_days();
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_name() {
        assert!(SessionManager::validate_name("my-session").is_ok());
        assert!(SessionManager::validate_name("session_123").is_ok());
        assert!(SessionManager::validate_name("Session2024").is_ok());
        
        assert!(SessionManager::validate_name("").is_err());
        assert!(SessionManager::validate_name("my session").is_err());
        assert!(SessionManager::validate_name("../hack").is_err());
    }

    #[test]
    fn test_session_manager_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        
        let messages = vec![];
        
        // Save
        let meta = manager.save("test-session", &messages, "stockpot", "gpt-4o").unwrap();
        assert_eq!(meta.name, "test-session");
        assert_eq!(meta.agent, "stockpot");
        
        // Load
        let loaded = manager.load("test-session").unwrap();
        assert_eq!(loaded.meta.name, "test-session");
        assert!(loaded.messages.is_empty());
    }

    #[test]
    fn test_session_manager_list() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        
        // Save multiple sessions
        manager.save("session-1", &[], "agent", "model").unwrap();
        manager.save("session-2", &[], "agent", "model").unwrap();
        manager.save("session-3", &[], "agent", "model").unwrap();
        
        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn test_session_manager_delete() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        
        manager.save("to-delete", &[], "agent", "model").unwrap();
        assert!(manager.exists("to-delete"));
        
        manager.delete("to-delete").unwrap();
        assert!(!manager.exists("to-delete"));
    }

    #[test]
    fn test_generate_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        
        let name1 = manager.generate_name("chat");
        let name2 = manager.generate_name("chat");
        
        assert!(name1.starts_with("chat-"));
        assert!(name2.starts_with("chat-"));
    }

    #[test]
    fn test_format_relative_time() {
        let now = Utc::now();
        assert_eq!(format_relative_time(now), "just now");
        
        let hour_ago = now - chrono::Duration::hours(1);
        assert!(format_relative_time(hour_ago).contains("hour"));
    }
}
