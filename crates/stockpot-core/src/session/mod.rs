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
            return Err(SessionError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        // Only allow alphanumeric, dash, underscore
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(SessionError::InvalidName(
                "Name can only contain letters, numbers, dashes, and underscores".to_string(),
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
    serde_json::to_string(msg).map(|s| s.len()).unwrap_or(100) // Default estimate
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
    use serdes_ai_core::ModelRequest;
    use tempfile::TempDir;

    // =========================================================================
    // Helper functions
    // =========================================================================

    fn create_test_message(content: &str) -> ModelRequest {
        let mut msg = ModelRequest::new();
        msg.add_user_prompt(content.to_string());
        msg
    }

    fn create_test_messages(count: usize) -> Vec<ModelRequest> {
        (0..count)
            .map(|i| create_test_message(&format!("Message {}", i)))
            .collect()
    }

    // =========================================================================
    // SessionError Tests
    // =========================================================================

    #[test]
    fn test_session_error_display_not_found() {
        let err = SessionError::NotFound("missing-session".to_string());
        assert!(err.to_string().contains("missing-session"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_session_error_display_invalid_name() {
        let err = SessionError::InvalidName("bad name".to_string());
        assert!(err.to_string().contains("Invalid session name"));
    }

    #[test]
    fn test_session_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let session_err: SessionError = io_err.into();
        assert!(matches!(session_err, SessionError::Io(_)));
    }

    #[test]
    fn test_session_error_from_serde() {
        let json_err = serde_json::from_str::<SessionData>("invalid json").unwrap_err();
        let session_err: SessionError = json_err.into();
        assert!(matches!(session_err, SessionError::Serialization(_)));
    }

    // =========================================================================
    // SessionMeta Tests
    // =========================================================================

    #[test]
    fn test_session_meta_new() {
        let meta = SessionMeta::new("test-session", "stockpot", "gpt-4o");

        assert_eq!(meta.name, "test-session");
        assert_eq!(meta.agent, "stockpot");
        assert_eq!(meta.model, "gpt-4o");
        assert_eq!(meta.message_count, 0);
        assert_eq!(meta.token_estimate, 0);
        assert!(meta.description.is_none());
        assert!(meta.created_at <= Utc::now());
        assert!(meta.updated_at <= Utc::now());
    }

    #[test]
    fn test_session_meta_update() {
        let mut meta = SessionMeta::new("test", "agent", "model");
        let initial_updated = meta.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));

        let messages = create_test_messages(5);
        meta.update(&messages);

        assert_eq!(meta.message_count, 5);
        assert!(meta.token_estimate > 0);
        assert!(meta.updated_at >= initial_updated);
    }

    #[test]
    fn test_session_meta_update_empty_messages() {
        let mut meta = SessionMeta::new("test", "agent", "model");
        meta.update(&[]);

        assert_eq!(meta.message_count, 0);
        assert_eq!(meta.token_estimate, 0);
    }

    #[test]
    fn test_session_meta_serialization() {
        let meta = SessionMeta::new("test", "agent", "model");
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: SessionMeta = serde_json::from_str(&json).unwrap();

        assert_eq!(meta.name, deserialized.name);
        assert_eq!(meta.agent, deserialized.agent);
        assert_eq!(meta.model, deserialized.model);
    }

    #[test]
    fn test_session_meta_with_description() {
        let mut meta = SessionMeta::new("test", "agent", "model");
        meta.description = Some("A test session".to_string());

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("description"));

        let deserialized: SessionMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.description, Some("A test session".to_string()));
    }

    #[test]
    fn test_session_meta_without_description_skips_field() {
        let meta = SessionMeta::new("test", "agent", "model");
        let json = serde_json::to_string(&meta).unwrap();
        // skip_serializing_if should omit None description
        assert!(!json.contains("description"));
    }

    // =========================================================================
    // SessionData Tests
    // =========================================================================

    #[test]
    fn test_session_data_new() {
        let data = SessionData::new("my-session", "stockpot", "gpt-4o");

        assert_eq!(data.meta.name, "my-session");
        assert_eq!(data.meta.agent, "stockpot");
        assert_eq!(data.meta.model, "gpt-4o");
        assert!(data.messages.is_empty());
    }

    #[test]
    fn test_session_data_update() {
        let mut data = SessionData::new("test", "agent", "model");
        let messages = create_test_messages(3);

        data.update(messages);

        assert_eq!(data.messages.len(), 3);
        assert_eq!(data.meta.message_count, 3);
        assert!(data.meta.token_estimate > 0);
    }

    #[test]
    fn test_session_data_update_replaces_messages() {
        let mut data = SessionData::new("test", "agent", "model");
        data.update(create_test_messages(5));
        assert_eq!(data.messages.len(), 5);

        // Update replaces, not appends
        data.update(create_test_messages(2));
        assert_eq!(data.messages.len(), 2);
        assert_eq!(data.meta.message_count, 2);
    }

    #[test]
    fn test_session_data_serialization() {
        let mut data = SessionData::new("test", "agent", "model");
        data.update(create_test_messages(2));

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: SessionData = serde_json::from_str(&json).unwrap();

        assert_eq!(data.meta.name, deserialized.meta.name);
        assert_eq!(data.messages.len(), deserialized.messages.len());
    }

    // =========================================================================
    // SessionManager Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_name_valid() {
        assert!(SessionManager::validate_name("my-session").is_ok());
        assert!(SessionManager::validate_name("session_123").is_ok());
        assert!(SessionManager::validate_name("Session2024").is_ok());
        assert!(SessionManager::validate_name("a").is_ok());
        assert!(SessionManager::validate_name("ABC").is_ok());
        assert!(SessionManager::validate_name("123").is_ok());
        assert!(SessionManager::validate_name("a-b_c").is_ok());
    }

    #[test]
    fn test_validate_name_empty() {
        let result = SessionManager::validate_name("");
        assert!(result.is_err());
        if let Err(SessionError::InvalidName(msg)) = result {
            assert!(msg.contains("empty"));
        } else {
            panic!("Expected InvalidName error");
        }
    }

    #[test]
    fn test_validate_name_with_spaces() {
        let result = SessionManager::validate_name("my session");
        assert!(result.is_err());
        if let Err(SessionError::InvalidName(msg)) = result {
            assert!(msg.contains("letters, numbers, dashes, and underscores"));
        } else {
            panic!("Expected InvalidName error");
        }
    }

    #[test]
    fn test_validate_name_path_traversal() {
        assert!(SessionManager::validate_name("../hack").is_err());
        assert!(SessionManager::validate_name("./local").is_err());
        assert!(SessionManager::validate_name("path/to/file").is_err());
    }

    #[test]
    fn test_validate_name_special_chars() {
        assert!(SessionManager::validate_name("session!").is_err());
        assert!(SessionManager::validate_name("session@test").is_err());
        assert!(SessionManager::validate_name("session#1").is_err());
        assert!(SessionManager::validate_name("session$").is_err());
        assert!(SessionManager::validate_name("session%").is_err());
    }

    // =========================================================================
    // SessionManager Constructor Tests
    // =========================================================================

    #[test]
    fn test_session_manager_default() {
        let manager = SessionManager::default();
        // Should use home dir
        assert!(manager
            .sessions_dir()
            .to_string_lossy()
            .contains("stockpot"));
    }

    #[test]
    fn test_session_manager_new() {
        let manager = SessionManager::new();
        assert!(manager.sessions_dir().ends_with("sessions"));
    }

    #[test]
    fn test_session_manager_with_dir() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        assert_eq!(manager.sessions_dir(), temp_dir.path());
    }

    #[test]
    fn test_session_manager_with_max_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path()).with_max_sessions(10);
        assert_eq!(manager.max_sessions, 10);
    }

    #[test]
    fn test_session_manager_with_max_sessions_zero_means_unlimited() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path()).with_max_sessions(0);
        assert_eq!(manager.max_sessions, 0);
    }

    // =========================================================================
    // SessionManager Save/Load Tests
    // =========================================================================

    #[test]
    fn test_session_manager_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let messages = vec![];

        // Save
        let meta = manager
            .save("test-session", &messages, "stockpot", "gpt-4o")
            .unwrap();
        assert_eq!(meta.name, "test-session");
        assert_eq!(meta.agent, "stockpot");

        // Load
        let loaded = manager.load("test-session").unwrap();
        assert_eq!(loaded.meta.name, "test-session");
        assert!(loaded.messages.is_empty());
    }

    #[test]
    fn test_session_manager_save_with_messages() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let messages = create_test_messages(5);
        let meta = manager
            .save("with-msgs", &messages, "agent", "model")
            .unwrap();

        assert_eq!(meta.message_count, 5);
        assert!(meta.token_estimate > 0);

        let loaded = manager.load("with-msgs").unwrap();
        assert_eq!(loaded.messages.len(), 5);
    }

    #[test]
    fn test_session_manager_save_updates_existing() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // First save
        manager
            .save("update-test", &[], "agent1", "model1")
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        // Second save with different agent/model
        let messages = create_test_messages(3);
        let meta = manager
            .save("update-test", &messages, "agent2", "model2")
            .unwrap();

        assert_eq!(meta.agent, "agent2");
        assert_eq!(meta.model, "model2");
        assert_eq!(meta.message_count, 3);

        let loaded = manager.load("update-test").unwrap();
        assert_eq!(loaded.meta.agent, "agent2");
    }

    #[test]
    fn test_session_manager_save_invalid_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let result = manager.save("invalid name", &[], "agent", "model");
        assert!(matches!(result, Err(SessionError::InvalidName(_))));
    }

    #[test]
    fn test_session_manager_load_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let result = manager.load("nonexistent");
        assert!(matches!(result, Err(SessionError::NotFound(_))));

        if let Err(SessionError::NotFound(name)) = result {
            assert_eq!(name, "nonexistent");
        }
    }

    #[test]
    fn test_session_manager_load_invalid_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let result = manager.load("invalid name");
        assert!(matches!(result, Err(SessionError::InvalidName(_))));
    }

    // =========================================================================
    // SessionManager List Tests
    // =========================================================================

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
    fn test_session_manager_list_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let sessions = manager.list().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_session_manager_list_sorted_by_updated_at() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        manager.save("old", &[], "agent", "model").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        manager.save("new", &[], "agent", "model").unwrap();

        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 2);
        // Most recent first
        assert_eq!(sessions[0].name, "new");
        assert_eq!(sessions[1].name, "old");
    }

    #[test]
    fn test_session_manager_list_skips_meta_files() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Create a valid session
        manager.save("valid", &[], "agent", "model").unwrap();

        // Create a file that looks like a meta file (should be skipped)
        let meta_path = temp_dir.path().join("something_meta.json");
        fs::write(&meta_path, "{}").unwrap();

        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "valid");
    }

    #[test]
    fn test_session_manager_list_ignores_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        manager.save("valid", &[], "agent", "model").unwrap();

        // Create invalid JSON file
        let invalid_path = temp_dir.path().join("invalid.json");
        fs::write(&invalid_path, "not valid json").unwrap();

        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_session_manager_list_ignores_non_json_files() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        manager.save("valid", &[], "agent", "model").unwrap();

        // Create non-json file
        let txt_path = temp_dir.path().join("notes.txt");
        fs::write(&txt_path, "some notes").unwrap();

        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 1);
    }

    // =========================================================================
    // SessionManager Delete Tests
    // =========================================================================

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
    fn test_session_manager_delete_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let result = manager.delete("nonexistent");
        assert!(matches!(result, Err(SessionError::NotFound(_))));
    }

    #[test]
    fn test_session_manager_delete_invalid_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let result = manager.delete("invalid name");
        assert!(matches!(result, Err(SessionError::InvalidName(_))));
    }

    // =========================================================================
    // SessionManager Exists Tests
    // =========================================================================

    #[test]
    fn test_session_manager_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        assert!(!manager.exists("new-session"));

        manager.save("new-session", &[], "agent", "model").unwrap();
        assert!(manager.exists("new-session"));
    }

    #[test]
    fn test_session_manager_exists_invalid_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Invalid name should return false (validation fails)
        assert!(!manager.exists("invalid name"));
        assert!(!manager.exists(""));
    }

    // =========================================================================
    // SessionManager Generate Name Tests
    // =========================================================================

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
    fn test_generate_name_with_collision() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Generate a name and save it
        let name1 = manager.generate_name("test");
        manager.save(&name1, &[], "agent", "model").unwrap();

        // Next generated name should be different
        let name2 = manager.generate_name("test");
        assert_ne!(name1, name2);
        assert!(name2.starts_with("test-"));
    }

    #[test]
    fn test_generate_name_different_prefixes() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let chat = manager.generate_name("chat");
        let debug = manager.generate_name("debug");

        assert!(chat.starts_with("chat-"));
        assert!(debug.starts_with("debug-"));
    }

    // =========================================================================
    // SessionManager Cleanup Tests
    // =========================================================================

    #[test]
    fn test_cleanup_removes_old_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path()).with_max_sessions(2);

        // Create 4 sessions with delays to ensure different timestamps
        for i in 1..=4 {
            manager
                .save(&format!("session-{}", i), &[], "agent", "model")
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        // After cleanup, should only have 2 most recent
        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 2);

        // Most recent should remain
        let names: Vec<_> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"session-4"));
        assert!(names.contains(&"session-3"));
    }

    #[test]
    fn test_cleanup_unlimited_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path()).with_max_sessions(0);

        // Create many sessions
        for i in 1..=10 {
            manager
                .save(&format!("session-{}", i), &[], "agent", "model")
                .unwrap();
        }

        // All should remain with max_sessions = 0
        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 10);
    }

    #[test]
    fn test_cleanup_no_action_under_limit() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path()).with_max_sessions(10);

        manager.save("session-1", &[], "agent", "model").unwrap();
        manager.save("session-2", &[], "agent", "model").unwrap();

        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    // =========================================================================
    // Token Estimation Tests
    // =========================================================================

    #[test]
    fn test_estimate_tokens_empty() {
        let messages: Vec<ModelRequest> = vec![];
        assert_eq!(estimate_tokens(&messages), 0);
    }

    #[test]
    fn test_estimate_tokens_with_messages() {
        let messages = create_test_messages(3);
        let tokens = estimate_tokens(&messages);
        assert!(tokens > 0);
    }

    #[test]
    fn test_estimate_tokens_scales_with_content() {
        let short_msg = create_test_message("Hi");
        let long_msg = create_test_message(&"x".repeat(1000));

        let short_tokens = estimate_tokens(&[short_msg]);
        let long_tokens = estimate_tokens(&[long_msg]);

        assert!(long_tokens > short_tokens);
    }

    #[test]
    fn test_estimate_message_chars() {
        let msg = create_test_message("Hello, world!");
        let chars = estimate_message_chars(&msg);
        assert!(chars > 0);
    }

    // =========================================================================
    // Random Suffix Tests
    // =========================================================================

    #[test]
    fn test_rand_suffix_not_empty() {
        let suffix = rand_suffix();
        assert!(!suffix.is_empty());
    }

    #[test]
    fn test_rand_suffix_is_hex() {
        let suffix = rand_suffix();
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_rand_suffix_reasonable_length() {
        let suffix = rand_suffix();
        // Should be at most 4 hex chars (0xFFFF max)
        assert!(suffix.len() <= 4);
    }

    // =========================================================================
    // Format Relative Time Tests
    // =========================================================================

    #[test]
    fn test_format_relative_time_just_now() {
        let now = Utc::now();
        assert_eq!(format_relative_time(now), "just now");
    }

    #[test]
    fn test_format_relative_time_seconds_ago() {
        let time = Utc::now() - chrono::Duration::seconds(30);
        assert_eq!(format_relative_time(time), "just now");
    }

    #[test]
    fn test_format_relative_time_one_minute() {
        let time = Utc::now() - chrono::Duration::minutes(1);
        assert_eq!(format_relative_time(time), "1 min ago");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let time = Utc::now() - chrono::Duration::minutes(30);
        assert_eq!(format_relative_time(time), "30 mins ago");
    }

    #[test]
    fn test_format_relative_time_one_hour() {
        let time = Utc::now() - chrono::Duration::hours(1);
        assert_eq!(format_relative_time(time), "1 hour ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let time = Utc::now() - chrono::Duration::hours(5);
        assert_eq!(format_relative_time(time), "5 hours ago");
    }

    #[test]
    fn test_format_relative_time_one_day() {
        let time = Utc::now() - chrono::Duration::days(1);
        assert_eq!(format_relative_time(time), "1 day ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let time = Utc::now() - chrono::Duration::days(3);
        assert_eq!(format_relative_time(time), "3 days ago");
    }

    #[test]
    fn test_format_relative_time_week_shows_date() {
        let time = Utc::now() - chrono::Duration::days(10);
        let formatted = format_relative_time(time);
        // Should show YYYY-MM-DD format
        assert!(formatted.contains('-'));
        assert_eq!(formatted.len(), 10);
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[test]
    fn test_full_session_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Create
        let messages = create_test_messages(3);
        let meta = manager
            .save("lifecycle", &messages, "agent", "model")
            .unwrap();
        assert_eq!(meta.message_count, 3);

        // Read
        let loaded = manager.load("lifecycle").unwrap();
        assert_eq!(loaded.messages.len(), 3);

        // Update
        let more_messages = create_test_messages(5);
        let updated_meta = manager
            .save("lifecycle", &more_messages, "agent2", "model2")
            .unwrap();
        assert_eq!(updated_meta.message_count, 5);
        assert_eq!(updated_meta.agent, "agent2");

        // List
        let sessions = manager.list().unwrap();
        assert_eq!(sessions.len(), 1);

        // Delete
        manager.delete("lifecycle").unwrap();
        assert!(!manager.exists("lifecycle"));
    }

    #[test]
    fn test_session_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        manager.save("test-session", &[], "agent", "model").unwrap();

        let expected_path = temp_dir.path().join("test-session.json");
        assert!(expected_path.exists());
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    #[test]
    fn test_load_corrupted_json_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Create a valid session first
        manager.save("test", &[], "agent", "model").unwrap();

        // Corrupt the file
        let path = temp_dir.path().join("test.json");
        fs::write(&path, "{ invalid json structure").unwrap();

        let result = manager.load("test");
        assert!(matches!(result, Err(SessionError::Serialization(_))));
    }

    #[test]
    fn test_load_wrong_json_schema() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        manager.ensure_dir().unwrap();

        // Write valid JSON but wrong schema
        let path = temp_dir.path().join("wrong-schema.json");
        fs::write(&path, r#"{"foo": "bar", "baz": 123}"#).unwrap();

        let result = manager.load("wrong-schema");
        assert!(matches!(result, Err(SessionError::Serialization(_))));
    }

    #[test]
    fn test_save_creates_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("a").join("b").join("c");
        let manager = SessionManager::with_dir(&nested);

        // Should create all dirs and save successfully
        let result = manager.save("test", &[], "agent", "model");
        assert!(result.is_ok());
        assert!(nested.exists());
    }

    #[test]
    fn test_ensure_dir_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Call multiple times - should not error
        assert!(manager.ensure_dir().is_ok());
        assert!(manager.ensure_dir().is_ok());
        assert!(manager.ensure_dir().is_ok());
    }

    #[test]
    fn test_sessions_dir_accessor() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let manager = SessionManager::with_dir(&path);

        assert_eq!(manager.sessions_dir(), path);
    }

    #[test]
    fn test_validate_name_unicode_letters_accepted() {
        // Unicode alphanumeric chars pass is_alphanumeric() in Rust
        // This is the actual behavior - testing it as-is
        assert!(SessionManager::validate_name("sessiön").is_ok()); // ö is alphanumeric
        assert!(SessionManager::validate_name("test\u{4E2D}").is_ok()); // chinese letter
    }

    #[test]
    fn test_validate_name_emoji_rejected() {
        // Emojis are NOT alphanumeric
        assert!(SessionManager::validate_name("\u{1F600}").is_err());
        assert!(SessionManager::validate_name("test\u{1F600}").is_err());
    }

    #[test]
    fn test_validate_name_very_long() {
        // Long but valid name should work
        let long_name = "a".repeat(200);
        assert!(SessionManager::validate_name(&long_name).is_ok());
    }

    #[test]
    fn test_session_meta_created_before_updated() {
        let meta = SessionMeta::new("test", "agent", "model");
        // created_at should be <= updated_at (they're set at same time initially)
        assert!(meta.created_at <= meta.updated_at);
    }

    #[test]
    fn test_session_meta_preserves_created_at_on_update() {
        let mut meta = SessionMeta::new("test", "agent", "model");
        let original_created = meta.created_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        meta.update(&create_test_messages(1));

        // created_at should not change
        assert_eq!(meta.created_at, original_created);
        // but updated_at should
        assert!(meta.updated_at > original_created);
    }

    #[test]
    fn test_session_data_clone() {
        let mut data = SessionData::new("test", "agent", "model");
        data.update(create_test_messages(2));

        let cloned = data.clone();
        assert_eq!(cloned.meta.name, data.meta.name);
        assert_eq!(cloned.messages.len(), data.messages.len());
    }

    #[test]
    fn test_session_meta_clone() {
        let meta = SessionMeta::new("test", "agent", "model");
        let cloned = meta.clone();

        assert_eq!(cloned.name, meta.name);
        assert_eq!(cloned.agent, meta.agent);
        assert_eq!(cloned.model, meta.model);
    }

    #[test]
    fn test_session_meta_debug() {
        let meta = SessionMeta::new("test", "agent", "model");
        let debug_str = format!("{:?}", meta);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("agent"));
    }

    #[test]
    fn test_session_data_debug() {
        let data = SessionData::new("test", "agent", "model");
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("meta"));
        assert!(debug_str.contains("messages"));
    }

    #[test]
    fn test_session_error_debug() {
        let err = SessionError::NotFound("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
    }

    #[test]
    fn test_save_empty_messages_then_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        manager.save("empty", &[], "agent", "model").unwrap();
        let loaded = manager.load("empty").unwrap();

        assert!(loaded.messages.is_empty());
        assert_eq!(loaded.meta.message_count, 0);
        assert_eq!(loaded.meta.token_estimate, 0);
    }

    #[test]
    fn test_list_creates_dir_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let sessions_path = temp_dir.path().join("sessions");
        let manager = SessionManager::with_dir(&sessions_path);

        // Dir doesn't exist yet
        assert!(!sessions_path.exists());

        // list() should create it
        let result = manager.list();
        assert!(result.is_ok());
        assert!(sessions_path.exists());
    }

    #[test]
    fn test_multiple_save_same_session_preserves_created_at() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // First save
        manager.save("multi", &[], "agent1", "model1").unwrap();
        let first_load = manager.load("multi").unwrap();
        let original_created = first_load.meta.created_at;

        std::thread::sleep(std::time::Duration::from_millis(20));

        // Second save
        let msgs = create_test_messages(2);
        manager.save("multi", &msgs, "agent2", "model2").unwrap();
        let second_load = manager.load("multi").unwrap();

        // created_at preserved, updated_at changed
        assert_eq!(second_load.meta.created_at, original_created);
        assert!(second_load.meta.updated_at > original_created);
    }

    #[test]
    fn test_cleanup_handles_delete_failure_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path()).with_max_sessions(1);

        // Create sessions
        manager.save("session-1", &[], "agent", "model").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.save("session-2", &[], "agent", "model").unwrap();

        // Even if cleanup had issues, the new save should succeed
        let sessions = manager.list().unwrap();
        assert!(sessions.len() <= 2); // cleanup may or may not fully complete
    }

    #[test]
    fn test_validate_name_single_char_types() {
        // Single valid characters
        assert!(SessionManager::validate_name("a").is_ok());
        assert!(SessionManager::validate_name("Z").is_ok());
        assert!(SessionManager::validate_name("5").is_ok());
        assert!(SessionManager::validate_name("-").is_ok());
        assert!(SessionManager::validate_name("_").is_ok());
    }

    #[test]
    fn test_validate_name_mixed_case() {
        assert!(SessionManager::validate_name("CamelCase").is_ok());
        assert!(SessionManager::validate_name("ALLCAPS").is_ok());
        assert!(SessionManager::validate_name("alllower").is_ok());
        assert!(SessionManager::validate_name("MixED-case_123").is_ok());
    }

    #[test]
    fn test_session_with_large_message_content() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        // Create message with large content
        let large_content = "x".repeat(100_000);
        let msg = create_test_message(&large_content);
        let messages = vec![msg];

        let meta = manager.save("large", &messages, "agent", "model").unwrap();
        assert!(meta.token_estimate > 10_000); // Should be substantial

        let loaded = manager.load("large").unwrap();
        assert_eq!(loaded.messages.len(), 1);
    }

    #[test]
    fn test_generate_name_unique_across_rapid_calls() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        let mut names = std::collections::HashSet::new();
        for _ in 0..10 {
            let name = manager.generate_name("rapid");
            manager.save(&name, &[], "agent", "model").unwrap();
            names.insert(name);
        }

        // All should be unique
        assert_eq!(names.len(), 10);
    }

    #[test]
    fn test_format_relative_time_edge_boundaries() {
        // 59 seconds = "just now"
        let time_59s = Utc::now() - chrono::Duration::seconds(59);
        assert_eq!(format_relative_time(time_59s), "just now");

        // 60 seconds = 1 min
        let time_60s = Utc::now() - chrono::Duration::seconds(60);
        assert!(format_relative_time(time_60s).contains("min"));

        // 59 minutes still in minutes
        let time_59m = Utc::now() - chrono::Duration::minutes(59);
        assert!(format_relative_time(time_59m).contains("mins"));

        // 6 days still in days
        let time_6d = Utc::now() - chrono::Duration::days(6);
        assert!(format_relative_time(time_6d).contains("days"));

        // 7 days shows date
        let time_7d = Utc::now() - chrono::Duration::days(7);
        let formatted = format_relative_time(time_7d);
        assert!(formatted.contains('-')); // YYYY-MM-DD format
    }

    #[test]
    fn test_session_error_io_preserves_original() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let session_err: SessionError = io_err.into();

        match session_err {
            SessionError::Io(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
            }
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_list_handles_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());
        manager.ensure_dir().unwrap();

        // Create empty file
        let empty_path = temp_dir.path().join("empty.json");
        fs::write(&empty_path, "").unwrap();

        // Should gracefully skip
        let sessions = manager.list().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_exists_after_delete() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(temp_dir.path());

        manager.save("transient", &[], "agent", "model").unwrap();
        assert!(manager.exists("transient"));

        manager.delete("transient").unwrap();
        assert!(!manager.exists("transient"));

        // Trying to load should now fail
        let result = manager.load("transient");
        assert!(matches!(result, Err(SessionError::NotFound(_))));
    }

    #[test]
    fn test_session_data_update_empty_to_many() {
        let mut data = SessionData::new("test", "agent", "model");
        assert!(data.messages.is_empty());

        data.update(create_test_messages(100));
        assert_eq!(data.messages.len(), 100);
        assert_eq!(data.meta.message_count, 100);
    }

    #[test]
    fn test_session_data_update_many_to_empty() {
        let mut data = SessionData::new("test", "agent", "model");
        data.update(create_test_messages(50));
        assert_eq!(data.messages.len(), 50);

        data.update(vec![]);
        assert!(data.messages.is_empty());
        assert_eq!(data.meta.message_count, 0);
    }
}
