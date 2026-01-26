//! Settings management via SQLite.

use std::collections::HashMap;

use crate::agents::UserMode;
use crate::db::Database;
use thiserror::Error;

/// PDF processing mode for attachments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfMode {
    #[default]
    Image, // Convert PDF pages to images
    TextExtract, // Extract text content from PDF
}

impl std::fmt::Display for PdfMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfMode::Image => write!(f, "image"),
            PdfMode::TextExtract => write!(f, "text"),
        }
    }
}

impl std::str::FromStr for PdfMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" | "text_extract" | "extract" => Ok(PdfMode::TextExtract),
            _ => Ok(PdfMode::Image),
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Setting not found: {0}")]
    NotFound(String),
}

/// Settings manager backed by SQLite.
pub struct Settings<'a> {
    db: &'a Database,
}

impl<'a> Settings<'a> {
    /// Create a new settings manager.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Get a setting value.
    pub fn get(&self, key: &str) -> Result<Option<String>, SettingsError> {
        let result: Result<String, _> =
            self.db
                .conn()
                .query_row("SELECT value FROM settings WHERE key = ?", [key], |row| {
                    row.get(0)
                });

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SettingsError::Database(e)),
        }
    }

    /// Get a setting value or return a default.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key)
            .ok()
            .flatten()
            .unwrap_or_else(|| default.to_string())
    }

    /// Get a boolean setting.
    pub fn get_bool(&self, key: &str) -> Result<bool, SettingsError> {
        match self.get(key)? {
            Some(v) => Ok(matches!(
                v.to_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            )),
            None => Ok(false),
        }
    }

    /// Set a setting value.
    pub fn set(&self, key: &str, value: &str) -> Result<(), SettingsError> {
        self.db.conn().execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            [key, value],
        )?;
        Ok(())
    }

    /// Delete a setting.
    pub fn delete(&self, key: &str) -> Result<(), SettingsError> {
        self.db
            .conn()
            .execute("DELETE FROM settings WHERE key = ?", [key])?;
        Ok(())
    }

    /// List all settings.
    pub fn list(&self) -> Result<Vec<(String, String)>, SettingsError> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = Vec::new();
        for row in rows {
            settings.push(row?);
        }
        Ok(settings)
    }

    // Convenience accessors for common settings

    /// Get the current model name.
    pub fn model(&self) -> String {
        self.get_or("model", "gpt-4o")
    }

    /// Get YOLO mode status (auto-accept shell commands without confirmation).
    pub fn yolo_mode(&self) -> bool {
        self.get_bool("yolo_mode").unwrap_or(false)
    }

    /// Set YOLO mode status.
    pub fn set_yolo_mode(&self, enabled: bool) -> Result<(), SettingsError> {
        self.set("yolo_mode", if enabled { "true" } else { "false" })
    }

    /// Get the assistant name.
    pub fn assistant_name(&self) -> String {
        self.get_or("assistant_name", "Stockpot")
    }

    /// Get the owner name.
    pub fn owner_name(&self) -> String {
        self.get_or("owner_name", "Master")
    }

    /// Get the current user mode.
    pub fn user_mode(&self) -> UserMode {
        self.get_or("user_mode", "normal")
            .parse()
            .unwrap_or_default()
    }

    /// Set the user mode.
    pub fn set_user_mode(&self, mode: UserMode) -> Result<(), SettingsError> {
        self.set("user_mode", &mode.to_string())
    }

    /// Get PDF processing mode (default: Image)
    pub fn pdf_mode(&self) -> PdfMode {
        self.get_or("pdf_mode", "image").parse().unwrap_or_default()
    }

    /// Set PDF processing mode
    pub fn set_pdf_mode(&self, mode: PdfMode) -> Result<(), SettingsError> {
        self.set("pdf_mode", &mode.to_string())
    }

    // Agent model pin management

    /// Build the settings key for an agent pin.
    fn agent_pin_key(agent_name: &str) -> String {
        format!("agent_pin.{}", agent_name)
    }

    /// Get the pinned model for an agent.
    pub fn get_agent_pinned_model(&self, agent_name: &str) -> Option<String> {
        self.get(&Self::agent_pin_key(agent_name)).ok().flatten()
    }

    /// Set the pinned model for an agent.
    pub fn set_agent_pinned_model(
        &self,
        agent_name: &str,
        model_name: &str,
    ) -> Result<(), SettingsError> {
        self.set(&Self::agent_pin_key(agent_name), model_name)
    }

    /// Clear the pinned model for an agent.
    pub fn clear_agent_pinned_model(&self, agent_name: &str) -> Result<(), SettingsError> {
        self.delete(&Self::agent_pin_key(agent_name))
    }

    /// Get all agent->model pin mappings.
    pub fn get_all_agent_pinned_models(&self) -> Result<HashMap<String, String>, SettingsError> {
        let prefix = "agent_pin.";
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT key, value FROM settings WHERE key LIKE ? ORDER BY key")?;
        let pattern = format!("{}%", prefix);
        let rows = stmt.query_map([pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut pins = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            // Strip the "agent_pin." prefix to get the agent name
            if let Some(agent_name) = key.strip_prefix(prefix) {
                pins.insert(agent_name.to_string(), value);
            }
        }
        Ok(pins)
    }

    /// Clear all agent pins that reference a specific model.
    /// Called when a model is removed to prevent orphaned pins.
    /// Returns the number of pins that were cleared.
    pub fn clear_pins_for_model(&self, model_name: &str) -> Result<usize, SettingsError> {
        // Get all agent pins
        let pins = self.get_all_agent_pinned_models()?;

        let mut cleared_count = 0;
        for (agent_name, pinned_model) in pins {
            if pinned_model == model_name {
                self.clear_agent_pinned_model(&agent_name)?;
                cleared_count += 1;
                tracing::debug!(
                    agent = %agent_name,
                    model = %model_name,
                    "Cleared orphaned agent pin for deleted model"
                );
            }
        }

        if cleared_count > 0 {
            tracing::info!(
                model = %model_name,
                count = cleared_count,
                "Cleared agent pins for deleted model"
            );
        }

        Ok(cleared_count)
    }

    // Agent MCP attachment management

    /// Build the settings key for an agent's MCP attachments.
    fn agent_mcp_key(agent_name: &str) -> String {
        format!("agent_mcp.{}", agent_name)
    }

    /// Get the MCPs attached to an agent (comma-separated list stored as single value).
    pub fn get_agent_mcps(&self, agent_name: &str) -> Vec<String> {
        self.get(&Self::agent_mcp_key(agent_name))
            .ok()
            .flatten()
            .map(|s| {
                s.split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Set the MCPs attached to an agent.
    pub fn set_agent_mcps(
        &self,
        agent_name: &str,
        mcp_names: &[String],
    ) -> Result<(), SettingsError> {
        let value = mcp_names.join(",");
        self.set(&Self::agent_mcp_key(agent_name), &value)
    }

    /// Add an MCP to an agent's attachments.
    pub fn add_agent_mcp(&self, agent_name: &str, mcp_name: &str) -> Result<(), SettingsError> {
        let mut mcps = self.get_agent_mcps(agent_name);
        if !mcps.contains(&mcp_name.to_string()) {
            mcps.push(mcp_name.to_string());
            self.set_agent_mcps(agent_name, &mcps)?;
        }
        Ok(())
    }

    /// Remove an MCP from an agent's attachments.
    pub fn remove_agent_mcp(&self, agent_name: &str, mcp_name: &str) -> Result<(), SettingsError> {
        let mcps: Vec<String> = self
            .get_agent_mcps(agent_name)
            .into_iter()
            .filter(|m| m != mcp_name)
            .collect();
        self.set_agent_mcps(agent_name, &mcps)
    }

    /// Clear all MCPs from an agent.
    pub fn clear_agent_mcps(&self, agent_name: &str) -> Result<(), SettingsError> {
        self.delete(&Self::agent_mcp_key(agent_name))
    }

    /// Get all agent->MCPs mappings.
    pub fn get_all_agent_mcps(&self) -> Result<HashMap<String, Vec<String>>, SettingsError> {
        let prefix = "agent_mcp.";
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT key, value FROM settings WHERE key LIKE ? ORDER BY key")?;
        let pattern = format!("{}%", prefix);
        let rows = stmt.query_map([pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut attachments = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            if let Some(agent_name) = key.strip_prefix(prefix) {
                let mcps: Vec<String> = value
                    .split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect();
                if !mcps.is_empty() {
                    attachments.insert(agent_name.to_string(), mcps);
                }
            }
        }
        Ok(attachments)
    }

    // ============ Helper Methods for Typed Access ============

    /// Get a string setting (returns None if not found)
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get(key).ok().flatten()
    }

    /// Set a string setting
    pub fn set_string(&self, key: &str, value: &str) {
        let _ = self.set(key, value);
    }

    /// Get a float setting (returns None if not found or invalid)
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key).ok().flatten().and_then(|s| s.parse().ok())
    }

    /// Set a float setting
    pub fn set_float(&self, key: &str, value: f64) {
        let _ = self.set(key, &value.to_string());
    }

    /// Get an int setting (returns None if not found or invalid)
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).ok().flatten().and_then(|s| s.parse().ok())
    }

    /// Set an int setting
    pub fn set_int(&self, key: &str, value: i64) {
        let _ = self.set(key, &value.to_string());
    }

    /// Set a bool setting
    pub fn set_bool(&self, key: &str, value: bool) {
        let _ = self.set(key, if value { "true" } else { "false" });
    }

    // ============ Context Compression Settings ============

    /// Get whether context compression is enabled (default: true)
    pub fn get_compression_enabled(&self) -> bool {
        // Default to true - compression is on by default
        self.get("compression.enabled")
            .ok()
            .flatten()
            .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0" | "no" | "off"))
            .unwrap_or(true)
    }

    /// Set whether context compression is enabled
    pub fn set_compression_enabled(&self, enabled: bool) {
        self.set_bool("compression.enabled", enabled);
    }

    /// Get compression strategy: "truncate" or "summarize" (default: "truncate")
    pub fn get_compression_strategy(&self) -> String {
        self.get_string("compression.strategy")
            .unwrap_or_else(|| "truncate".to_string())
    }

    /// Set compression strategy
    pub fn set_compression_strategy(&self, strategy: &str) {
        self.set_string("compression.strategy", strategy);
    }

    /// Get compression threshold (0.0-1.0, default: 0.75)
    pub fn get_compression_threshold(&self) -> f64 {
        self.get_float("compression.threshold").unwrap_or(0.75)
    }

    /// Set compression threshold
    pub fn set_compression_threshold(&self, threshold: f64) {
        self.set_float("compression.threshold", threshold);
    }

    /// Get target tokens for compression (default: 30000)
    pub fn get_compression_target_tokens(&self) -> usize {
        self.get_int("compression.target_tokens").unwrap_or(30000) as usize
    }

    /// Set target tokens for compression
    pub fn set_compression_target_tokens(&self, tokens: usize) {
        self.set_int("compression.target_tokens", tokens as i64);
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for settings management.
    //!
    //! Coverage:
    //! - PdfMode parsing and display
    //! - Basic CRUD operations (get, set, delete, list)
    //! - Boolean setting parsing
    //! - Default value fallbacks
    //! - Convenience accessors (model, yolo_mode, etc.)
    //! - Agent model pin management
    //! - Agent MCP attachment management

    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, db)
    }

    // =========================================================================
    // PdfMode Tests
    // =========================================================================

    #[test]
    fn test_pdf_mode_display_image() {
        assert_eq!(PdfMode::Image.to_string(), "image");
    }

    #[test]
    fn test_pdf_mode_display_text_extract() {
        assert_eq!(PdfMode::TextExtract.to_string(), "text");
    }

    #[test]
    fn test_pdf_mode_parse_text_variants() {
        assert_eq!("text".parse::<PdfMode>().unwrap(), PdfMode::TextExtract);
        assert_eq!(
            "text_extract".parse::<PdfMode>().unwrap(),
            PdfMode::TextExtract
        );
        assert_eq!("extract".parse::<PdfMode>().unwrap(), PdfMode::TextExtract);
        assert_eq!("TEXT".parse::<PdfMode>().unwrap(), PdfMode::TextExtract);
        assert_eq!("Extract".parse::<PdfMode>().unwrap(), PdfMode::TextExtract);
    }

    #[test]
    fn test_pdf_mode_parse_image_default() {
        // Any unrecognized value defaults to Image
        assert_eq!("image".parse::<PdfMode>().unwrap(), PdfMode::Image);
        assert_eq!("IMAGE".parse::<PdfMode>().unwrap(), PdfMode::Image);
        assert_eq!("unknown".parse::<PdfMode>().unwrap(), PdfMode::Image);
        assert_eq!("".parse::<PdfMode>().unwrap(), PdfMode::Image);
        assert_eq!("foo".parse::<PdfMode>().unwrap(), PdfMode::Image);
    }

    #[test]
    fn test_pdf_mode_default() {
        assert_eq!(PdfMode::default(), PdfMode::Image);
    }

    #[test]
    fn test_pdf_mode_equality() {
        assert_eq!(PdfMode::Image, PdfMode::Image);
        assert_eq!(PdfMode::TextExtract, PdfMode::TextExtract);
        assert_ne!(PdfMode::Image, PdfMode::TextExtract);
    }

    #[test]
    fn test_pdf_mode_clone() {
        let mode = PdfMode::TextExtract;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    // =========================================================================
    // Basic CRUD Tests
    // =========================================================================

    #[test]
    fn test_get_nonexistent_returns_none() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let result = settings.get("nonexistent_key").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_set_and_get() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("test_key", "test_value").unwrap();
        let result = settings.get("test_key").unwrap();

        assert_eq!(result, Some("test_value".to_string()));
    }

    #[test]
    fn test_set_overwrites_existing() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("key", "value1").unwrap();
        settings.set("key", "value2").unwrap();

        assert_eq!(settings.get("key").unwrap(), Some("value2".to_string()));
    }

    #[test]
    fn test_delete_existing_key() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("to_delete", "value").unwrap();
        assert!(settings.get("to_delete").unwrap().is_some());

        settings.delete("to_delete").unwrap();
        assert!(settings.get("to_delete").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_key_succeeds() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Deleting a non-existent key should not error
        let result = settings.delete("never_existed");
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_empty() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let list = settings.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_multiple_settings() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("alpha", "1").unwrap();
        settings.set("beta", "2").unwrap();
        settings.set("gamma", "3").unwrap();

        let list = settings.list().unwrap();
        assert_eq!(list.len(), 3);

        // Should be sorted by key
        assert_eq!(list[0], ("alpha".to_string(), "1".to_string()));
        assert_eq!(list[1], ("beta".to_string(), "2".to_string()));
        assert_eq!(list[2], ("gamma".to_string(), "3".to_string()));
    }

    // =========================================================================
    // get_or Tests
    // =========================================================================

    #[test]
    fn test_get_or_returns_value_when_exists() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("existing", "actual_value").unwrap();
        assert_eq!(settings.get_or("existing", "default"), "actual_value");
    }

    #[test]
    fn test_get_or_returns_default_when_missing() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert_eq!(settings.get_or("missing", "default_value"), "default_value");
    }

    // =========================================================================
    // Boolean Setting Tests
    // =========================================================================

    #[test]
    fn test_get_bool_true_variants() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        for (key, value) in [
            ("bool_true", "true"),
            ("bool_1", "1"),
            ("bool_yes", "yes"),
            ("bool_on", "on"),
            ("bool_TRUE", "TRUE"),
            ("bool_Yes", "Yes"),
            ("bool_ON", "ON"),
        ] {
            settings.set(key, value).unwrap();
            assert!(
                settings.get_bool(key).unwrap(),
                "Expected '{}' to be true",
                value
            );
        }
    }

    #[test]
    fn test_get_bool_false_variants() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        for (key, value) in [
            ("bool_false", "false"),
            ("bool_0", "0"),
            ("bool_no", "no"),
            ("bool_off", "off"),
            ("bool_random", "random"),
            ("bool_empty", ""),
        ] {
            settings.set(key, value).unwrap();
            assert!(
                !settings.get_bool(key).unwrap(),
                "Expected '{}' to be false",
                value
            );
        }
    }

    #[test]
    fn test_get_bool_missing_returns_false() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert!(!settings.get_bool("nonexistent").unwrap());
    }

    // =========================================================================
    // Convenience Accessor Tests
    // =========================================================================

    #[test]
    fn test_model_default() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert_eq!(settings.model(), "gpt-4o");
    }

    #[test]
    fn test_model_custom() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("model", "claude-3-opus").unwrap();
        assert_eq!(settings.model(), "claude-3-opus");
    }

    #[test]
    fn test_yolo_mode_default_false() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert!(!settings.yolo_mode());
    }

    #[test]
    fn test_yolo_mode_enabled() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("yolo_mode", "true").unwrap();
        assert!(settings.yolo_mode());
    }

    #[test]
    fn test_assistant_name_default() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert_eq!(settings.assistant_name(), "Stockpot");
    }

    #[test]
    fn test_assistant_name_custom() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("assistant_name", "Jarvis").unwrap();
        assert_eq!(settings.assistant_name(), "Jarvis");
    }

    #[test]
    fn test_owner_name_default() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert_eq!(settings.owner_name(), "Master");
    }

    #[test]
    fn test_owner_name_custom() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("owner_name", "Tony").unwrap();
        assert_eq!(settings.owner_name(), "Tony");
    }

    // =========================================================================
    // UserMode Tests
    // =========================================================================

    #[test]
    fn test_user_mode_default() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert_eq!(settings.user_mode(), UserMode::Normal);
    }

    #[test]
    fn test_user_mode_set_and_get() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set_user_mode(UserMode::Expert).unwrap();
        assert_eq!(settings.user_mode(), UserMode::Expert);

        settings.set_user_mode(UserMode::Developer).unwrap();
        assert_eq!(settings.user_mode(), UserMode::Developer);

        settings.set_user_mode(UserMode::Normal).unwrap();
        assert_eq!(settings.user_mode(), UserMode::Normal);
    }

    // =========================================================================
    // PdfMode Settings Tests
    // =========================================================================

    #[test]
    fn test_pdf_mode_default_setting() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert_eq!(settings.pdf_mode(), PdfMode::Image);
    }

    #[test]
    fn test_pdf_mode_set_and_get() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set_pdf_mode(PdfMode::TextExtract).unwrap();
        assert_eq!(settings.pdf_mode(), PdfMode::TextExtract);

        settings.set_pdf_mode(PdfMode::Image).unwrap();
        assert_eq!(settings.pdf_mode(), PdfMode::Image);
    }

    // =========================================================================
    // Agent Model Pin Tests
    // =========================================================================

    #[test]
    fn test_agent_pin_key_format() {
        assert_eq!(Settings::agent_pin_key("stockpot"), "agent_pin.stockpot");
        assert_eq!(Settings::agent_pin_key("explore"), "agent_pin.explore");
    }

    #[test]
    fn test_get_agent_pinned_model_none() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        assert!(settings.get_agent_pinned_model("stockpot").is_none());
    }

    #[test]
    fn test_set_and_get_agent_pinned_model() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings
            .set_agent_pinned_model("stockpot", "gpt-4o")
            .unwrap();
        assert_eq!(
            settings.get_agent_pinned_model("stockpot"),
            Some("gpt-4o".to_string())
        );
    }

    #[test]
    fn test_clear_agent_pinned_model() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings
            .set_agent_pinned_model("explore", "claude-3-haiku")
            .unwrap();
        assert!(settings.get_agent_pinned_model("explore").is_some());

        settings.clear_agent_pinned_model("explore").unwrap();
        assert!(settings.get_agent_pinned_model("explore").is_none());
    }

    #[test]
    fn test_get_all_agent_pinned_models_empty() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let pins = settings.get_all_agent_pinned_models().unwrap();
        assert!(pins.is_empty());
    }

    #[test]
    fn test_get_all_agent_pinned_models() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings
            .set_agent_pinned_model("stockpot", "gpt-4o")
            .unwrap();
        settings
            .set_agent_pinned_model("explore", "claude-3-haiku")
            .unwrap();
        settings
            .set_agent_pinned_model("planning", "gpt-4-turbo")
            .unwrap();

        let pins = settings.get_all_agent_pinned_models().unwrap();
        assert_eq!(pins.len(), 3);
        assert_eq!(pins.get("stockpot"), Some(&"gpt-4o".to_string()));
        assert_eq!(pins.get("explore"), Some(&"claude-3-haiku".to_string()));
        assert_eq!(pins.get("planning"), Some(&"gpt-4-turbo".to_string()));
    }

    #[test]
    fn test_clear_pins_for_model_single_agent() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Pin one agent to a model
        settings
            .set_agent_pinned_model("stockpot", "gpt-4o")
            .unwrap();

        // Clear pins for that model
        let cleared = settings.clear_pins_for_model("gpt-4o").unwrap();

        assert_eq!(cleared, 1);
        assert!(settings.get_agent_pinned_model("stockpot").is_none());
    }

    #[test]
    fn test_clear_pins_for_model_multiple_agents() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Pin multiple agents to the same model
        settings
            .set_agent_pinned_model("stockpot", "gpt-4o")
            .unwrap();
        settings
            .set_agent_pinned_model("explore", "gpt-4o")
            .unwrap();
        settings
            .set_agent_pinned_model("planning", "gpt-4o")
            .unwrap();

        // Clear pins for that model
        let cleared = settings.clear_pins_for_model("gpt-4o").unwrap();

        assert_eq!(cleared, 3);
        assert!(settings.get_agent_pinned_model("stockpot").is_none());
        assert!(settings.get_agent_pinned_model("explore").is_none());
        assert!(settings.get_agent_pinned_model("planning").is_none());
    }

    #[test]
    fn test_clear_pins_for_model_no_match() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Pin agents to different models
        settings
            .set_agent_pinned_model("stockpot", "claude-3-opus")
            .unwrap();
        settings
            .set_agent_pinned_model("explore", "gemini-pro")
            .unwrap();

        // Clear pins for a model that no one is using
        let cleared = settings.clear_pins_for_model("gpt-4o").unwrap();

        assert_eq!(cleared, 0);
        // Other pins should remain
        assert_eq!(
            settings.get_agent_pinned_model("stockpot"),
            Some("claude-3-opus".to_string())
        );
        assert_eq!(
            settings.get_agent_pinned_model("explore"),
            Some("gemini-pro".to_string())
        );
    }

    #[test]
    fn test_clear_pins_for_model_mixed() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Pin some agents to the target model, others to different models
        settings
            .set_agent_pinned_model("stockpot", "gpt-4o")
            .unwrap();
        settings
            .set_agent_pinned_model("explore", "claude-3-opus")
            .unwrap();
        settings
            .set_agent_pinned_model("planning", "gpt-4o")
            .unwrap();
        settings
            .set_agent_pinned_model("reviewer", "gemini-pro")
            .unwrap();

        // Clear pins for gpt-4o only
        let cleared = settings.clear_pins_for_model("gpt-4o").unwrap();

        assert_eq!(cleared, 2);
        // These should be cleared
        assert!(settings.get_agent_pinned_model("stockpot").is_none());
        assert!(settings.get_agent_pinned_model("planning").is_none());
        // These should remain
        assert_eq!(
            settings.get_agent_pinned_model("explore"),
            Some("claude-3-opus".to_string())
        );
        assert_eq!(
            settings.get_agent_pinned_model("reviewer"),
            Some("gemini-pro".to_string())
        );
    }

    #[test]
    fn test_clear_pins_for_model_empty() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // No pins exist
        let cleared = settings.clear_pins_for_model("gpt-4o").unwrap();

        assert_eq!(cleared, 0);
    }

    // =========================================================================
    // Agent MCP Attachment Tests
    // =========================================================================

    #[test]
    fn test_agent_mcp_key_format() {
        assert_eq!(Settings::agent_mcp_key("stockpot"), "agent_mcp.stockpot");
        assert_eq!(Settings::agent_mcp_key("explore"), "agent_mcp.explore");
    }

    #[test]
    fn test_get_agent_mcps_empty() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let mcps = settings.get_agent_mcps("stockpot");
        assert!(mcps.is_empty());
    }

    #[test]
    fn test_set_and_get_agent_mcps() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let mcps = vec!["github".to_string(), "slack".to_string()];
        settings.set_agent_mcps("stockpot", &mcps).unwrap();

        let result = settings.get_agent_mcps("stockpot");
        assert_eq!(result, vec!["github", "slack"]);
    }

    #[test]
    fn test_add_agent_mcp() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "github").unwrap();
        settings.add_agent_mcp("stockpot", "slack").unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert_eq!(mcps.len(), 2);
        assert!(mcps.contains(&"github".to_string()));
        assert!(mcps.contains(&"slack".to_string()));
    }

    #[test]
    fn test_add_agent_mcp_no_duplicates() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "github").unwrap();
        settings.add_agent_mcp("stockpot", "github").unwrap();
        settings.add_agent_mcp("stockpot", "github").unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert_eq!(mcps.len(), 1);
        assert_eq!(mcps[0], "github");
    }

    #[test]
    fn test_remove_agent_mcp() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "github").unwrap();
        settings.add_agent_mcp("stockpot", "slack").unwrap();
        settings.add_agent_mcp("stockpot", "jira").unwrap();

        settings.remove_agent_mcp("stockpot", "slack").unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert_eq!(mcps.len(), 2);
        assert!(mcps.contains(&"github".to_string()));
        assert!(mcps.contains(&"jira".to_string()));
        assert!(!mcps.contains(&"slack".to_string()));
    }

    #[test]
    fn test_remove_agent_mcp_nonexistent() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "github").unwrap();

        // Removing non-existent MCP should succeed without error
        settings
            .remove_agent_mcp("stockpot", "nonexistent")
            .unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert_eq!(mcps.len(), 1);
        assert_eq!(mcps[0], "github");
    }

    #[test]
    fn test_clear_agent_mcps() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "github").unwrap();
        settings.add_agent_mcp("stockpot", "slack").unwrap();

        settings.clear_agent_mcps("stockpot").unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert!(mcps.is_empty());
    }

    #[test]
    fn test_get_all_agent_mcps_empty() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let all_mcps = settings.get_all_agent_mcps().unwrap();
        assert!(all_mcps.is_empty());
    }

    #[test]
    fn test_get_all_agent_mcps() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "github").unwrap();
        settings.add_agent_mcp("stockpot", "slack").unwrap();
        settings.add_agent_mcp("explore", "jira").unwrap();

        let all_mcps = settings.get_all_agent_mcps().unwrap();
        assert_eq!(all_mcps.len(), 2);

        let stockpot_mcps = all_mcps.get("stockpot").unwrap();
        assert_eq!(stockpot_mcps.len(), 2);
        assert!(stockpot_mcps.contains(&"github".to_string()));
        assert!(stockpot_mcps.contains(&"slack".to_string()));

        let explore_mcps = all_mcps.get("explore").unwrap();
        assert_eq!(explore_mcps.len(), 1);
        assert!(explore_mcps.contains(&"jira".to_string()));
    }

    #[test]
    fn test_agent_mcps_handles_whitespace() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Manually set with spaces to test trimming
        settings
            .set("agent_mcp.stockpot", "github , slack , jira")
            .unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert_eq!(mcps, vec!["github", "slack", "jira"]);
    }

    #[test]
    fn test_agent_mcps_filters_empty_entries() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Manually set with empty entries
        settings
            .set("agent_mcp.stockpot", "github,,slack,")
            .unwrap();

        let mcps = settings.get_agent_mcps("stockpot");
        assert_eq!(mcps, vec!["github", "slack"]);
    }

    // =========================================================================
    // Edge Cases and Additional Coverage
    // =========================================================================

    #[test]
    fn test_set_empty_string_value() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("empty_key", "").unwrap();
        let result = settings.get("empty_key").unwrap();
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn test_get_or_with_empty_string_stored() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Empty string is a valid value, should return it not the default
        settings.set("empty_val", "").unwrap();
        assert_eq!(settings.get_or("empty_val", "default"), "");
    }

    #[test]
    fn test_special_characters_in_key() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let keys = [
            "key.with.dots",
            "key-with-dashes",
            "key_with_underscores",
            "key:colon",
        ];
        for key in keys {
            settings.set(key, "value").unwrap();
            assert_eq!(settings.get(key).unwrap(), Some("value".to_string()));
        }
    }

    #[test]
    fn test_special_characters_in_value() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let values = [
            "path/to/file",
            "C:\\Windows\\System32",
            "value with spaces",
            "value\twith\ttabs",
            "value\nwith\nnewlines",
            r#"{"json": "value"}"#,
            "emoji: 🚀",
        ];

        for (i, val) in values.iter().enumerate() {
            let key = format!("special_{}", i);
            settings.set(&key, val).unwrap();
            assert_eq!(settings.get(&key).unwrap(), Some(val.to_string()));
        }
    }

    #[test]
    fn test_unicode_in_keys_and_values() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("日本語キー", "日本語の値").unwrap();
        assert_eq!(
            settings.get("日本語キー").unwrap(),
            Some("日本語の値".to_string())
        );

        settings.set("emoji_key", "🎉🚀💻").unwrap();
        assert_eq!(
            settings.get("emoji_key").unwrap(),
            Some("🎉🚀💻".to_string())
        );
    }

    #[test]
    fn test_very_long_value() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let long_value = "x".repeat(10_000);
        settings.set("long_key", &long_value).unwrap();
        assert_eq!(settings.get("long_key").unwrap(), Some(long_value));
    }

    #[test]
    fn test_user_mode_invalid_falls_back_to_default() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Set an invalid user mode string directly
        settings.set("user_mode", "invalid_mode").unwrap();

        // user_mode() should fall back to default (Normal)
        assert_eq!(settings.user_mode(), UserMode::Normal);
    }

    #[test]
    fn test_user_mode_case_insensitive() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("user_mode", "EXPERT").unwrap();
        assert_eq!(settings.user_mode(), UserMode::Expert);

        settings.set("user_mode", "Developer").unwrap();
        assert_eq!(settings.user_mode(), UserMode::Developer);

        settings.set("user_mode", "NORMAL").unwrap();
        assert_eq!(settings.user_mode(), UserMode::Normal);
    }

    #[test]
    fn test_user_mode_whitespace_trimmed() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("user_mode", "  expert  ").unwrap();
        assert_eq!(settings.user_mode(), UserMode::Expert);
    }

    #[test]
    fn test_delete_then_readd() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("volatile", "first").unwrap();
        settings.delete("volatile").unwrap();
        settings.set("volatile", "second").unwrap();

        assert_eq!(
            settings.get("volatile").unwrap(),
            Some("second".to_string())
        );
    }

    #[test]
    fn test_list_excludes_deleted() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("keep", "1").unwrap();
        settings.set("delete_me", "2").unwrap();
        settings.delete("delete_me").unwrap();

        let list = settings.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "keep");
    }

    #[test]
    fn test_agent_pins_isolated_between_agents() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings
            .set_agent_pinned_model("agent_a", "model_a")
            .unwrap();
        settings
            .set_agent_pinned_model("agent_b", "model_b")
            .unwrap();

        // Clearing one shouldn't affect the other
        settings.clear_agent_pinned_model("agent_a").unwrap();

        assert!(settings.get_agent_pinned_model("agent_a").is_none());
        assert_eq!(
            settings.get_agent_pinned_model("agent_b"),
            Some("model_b".to_string())
        );
    }

    #[test]
    fn test_agent_mcps_isolated_between_agents() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("agent_a", "mcp_a").unwrap();
        settings.add_agent_mcp("agent_b", "mcp_b").unwrap();

        // Clearing one shouldn't affect the other
        settings.clear_agent_mcps("agent_a").unwrap();

        assert!(settings.get_agent_mcps("agent_a").is_empty());
        assert_eq!(settings.get_agent_mcps("agent_b"), vec!["mcp_b"]);
    }

    #[test]
    fn test_overwrite_agent_pinned_model() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings
            .set_agent_pinned_model("stockpot", "gpt-4")
            .unwrap();
        settings
            .set_agent_pinned_model("stockpot", "gpt-4o")
            .unwrap();

        assert_eq!(
            settings.get_agent_pinned_model("stockpot"),
            Some("gpt-4o".to_string())
        );

        // Should only have one entry for stockpot
        let all = settings.get_all_agent_pinned_models().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_agent_name_with_dots() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Agent names with dots could conflict with key format
        settings
            .set_agent_pinned_model("agent.with.dots", "model")
            .unwrap();
        assert_eq!(
            settings.get_agent_pinned_model("agent.with.dots"),
            Some("model".to_string())
        );

        settings
            .add_agent_mcp("agent.with.dots", "some_mcp")
            .unwrap();
        assert_eq!(settings.get_agent_mcps("agent.with.dots"), vec!["some_mcp"]);
    }

    #[test]
    fn test_get_all_agent_mcps_excludes_empty_lists() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Set one with MCPs, one with empty string
        settings.add_agent_mcp("agent_with", "mcp1").unwrap();
        settings.set("agent_mcp.agent_empty", "").unwrap();
        settings.set("agent_mcp.agent_spaces", "   ,  ,  ").unwrap();

        let all = settings.get_all_agent_mcps().unwrap();

        // Only agent_with should be present
        assert_eq!(all.len(), 1);
        assert!(all.contains_key("agent_with"));
        assert!(!all.contains_key("agent_empty"));
        assert!(!all.contains_key("agent_spaces"));
    }

    #[test]
    fn test_pdf_mode_roundtrip() {
        // Verify that display -> parse is consistent
        let modes = [PdfMode::Image, PdfMode::TextExtract];
        for mode in modes {
            let display = mode.to_string();
            let parsed: PdfMode = display.parse().unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn test_settings_error_display() {
        let db_err = SettingsError::Database(rusqlite::Error::QueryReturnedNoRows);
        assert!(db_err.to_string().contains("Database error"));

        let not_found = SettingsError::NotFound("missing_key".to_string());
        assert!(not_found.to_string().contains("Setting not found"));
        assert!(not_found.to_string().contains("missing_key"));
    }

    #[test]
    fn test_clear_nonexistent_agent_pinned_model() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Clearing a pin that never existed should succeed
        let result = settings.clear_agent_pinned_model("never_existed");
        assert!(result.is_ok());
    }

    #[test]
    fn test_clear_nonexistent_agent_mcps() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Clearing MCPs for agent that never existed should succeed
        let result = settings.clear_agent_mcps("never_existed");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pdf_mode_debug() {
        // Ensure Debug trait works
        let mode = PdfMode::Image;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Image"));

        let mode = PdfMode::TextExtract;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("TextExtract"));
    }

    #[test]
    fn test_pdf_mode_copy() {
        let mode = PdfMode::TextExtract;
        let copied = mode; // Copy
        let also_copied = mode; // Copy again - mode still valid
        assert_eq!(copied, also_copied);
        assert_eq!(mode, PdfMode::TextExtract);
    }

    #[test]
    fn test_concurrent_agent_operations() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Set up multiple agents with pins and MCPs
        for i in 0..5 {
            let agent = format!("agent_{}", i);
            settings
                .set_agent_pinned_model(&agent, &format!("model_{}", i))
                .unwrap();
            settings
                .add_agent_mcp(&agent, &format!("mcp_{}", i))
                .unwrap();
            settings
                .add_agent_mcp(&agent, &format!("mcp_{}_extra", i))
                .unwrap();
        }

        let pins = settings.get_all_agent_pinned_models().unwrap();
        assert_eq!(pins.len(), 5);

        let mcps = settings.get_all_agent_mcps().unwrap();
        assert_eq!(mcps.len(), 5);

        // Each agent should have 2 MCPs
        for i in 0..5 {
            let agent = format!("agent_{}", i);
            assert_eq!(mcps.get(&agent).unwrap().len(), 2);
        }
    }

    #[test]
    fn test_set_agent_mcps_empty_clears() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.add_agent_mcp("stockpot", "mcp1").unwrap();
        settings.add_agent_mcp("stockpot", "mcp2").unwrap();

        // Setting empty list should effectively clear
        settings.set_agent_mcps("stockpot", &[]).unwrap();

        // get_agent_mcps should return empty vec
        let mcps = settings.get_agent_mcps("stockpot");
        assert!(mcps.is_empty());
    }

    #[test]
    fn test_settings_preserves_order_in_list() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        // Insert in non-alphabetical order
        settings.set("zebra", "z").unwrap();
        settings.set("alpha", "a").unwrap();
        settings.set("beta", "b").unwrap();

        let list = settings.list().unwrap();
        let keys: Vec<_> = list.iter().map(|(k, _)| k.as_str()).collect();

        // Should be sorted alphabetically
        assert_eq!(keys, vec!["alpha", "beta", "zebra"]);
    }
}
