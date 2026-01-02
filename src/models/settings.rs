//! Per-model settings stored in SQLite.
//!
//! Model settings are stored with keys prefixed by `model_settings.<model_name>.<key>`.
//! This allows each model to have its own temperature, max_tokens, etc.

use crate::db::Database;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur with model settings.
#[derive(Debug, Error)]
pub enum ModelSettingsError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Invalid setting value: {0}")]
    InvalidValue(String),
    #[error("Setting parse error: {0}")]
    ParseError(String),
}

/// Per-model settings that can be customized.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Temperature for sampling (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Random seed for reproducible outputs
    pub seed: Option<i64>,
    /// Maximum tokens to generate
    pub max_tokens: Option<i32>,
    /// Enable extended thinking mode (Anthropic)
    pub extended_thinking: Option<bool>,
    /// Budget tokens for thinking (Anthropic)
    pub budget_tokens: Option<i32>,
    /// Enable interleaved thinking (Anthropic)
    pub interleaved_thinking: Option<bool>,
    /// Reasoning effort level (OpenAI o1 models: low, medium, high)
    pub reasoning_effort: Option<String>,
    /// Verbosity level (0-3)
    pub verbosity: Option<i32>,
}

impl ModelSettings {
    /// Create empty settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load settings for a model from the database.
    pub fn load(db: &Database, model_name: &str) -> Result<Self, ModelSettingsError> {
        let prefix = format!("model_settings.{}.", model_name);
        let mut settings = Self::new();

        // Query all settings for this model
        let mut stmt = db
            .conn()
            .prepare("SELECT key, value FROM settings WHERE key LIKE ?")?;

        let pattern = format!("{}%", prefix);
        let rows = stmt.query_map([&pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (key, value) = row?;
            let setting_key = key.strip_prefix(&prefix).unwrap_or(&key);
            settings.apply_setting(setting_key, &value)?;
        }

        Ok(settings)
    }

    /// Apply a setting by key name.
    fn apply_setting(&mut self, key: &str, value: &str) -> Result<(), ModelSettingsError> {
        match key {
            "temperature" => {
                self.temperature = Some(value.parse().map_err(|_| {
                    ModelSettingsError::ParseError(format!("Invalid temperature: {}", value))
                })?);
            }
            "seed" => {
                self.seed = Some(value.parse().map_err(|_| {
                    ModelSettingsError::ParseError(format!("Invalid seed: {}", value))
                })?);
            }
            "max_tokens" => {
                self.max_tokens = Some(value.parse().map_err(|_| {
                    ModelSettingsError::ParseError(format!("Invalid max_tokens: {}", value))
                })?);
            }
            "extended_thinking" => {
                self.extended_thinking = Some(parse_bool(value));
            }
            "budget_tokens" => {
                self.budget_tokens = Some(value.parse().map_err(|_| {
                    ModelSettingsError::ParseError(format!("Invalid budget_tokens: {}", value))
                })?);
            }
            "interleaved_thinking" => {
                self.interleaved_thinking = Some(parse_bool(value));
            }
            "reasoning_effort" => {
                let effort = value.to_lowercase();
                if !matches!(
                    effort.as_str(),
                    "minimal" | "low" | "medium" | "high" | "xhigh"
                ) {
                    return Err(ModelSettingsError::InvalidValue(
                        "reasoning_effort must be minimal, low, medium, high, or xhigh".to_string(),
                    ));
                }
                self.reasoning_effort = Some(effort);
            }
            "verbosity" => {
                let v: i32 = value.parse().map_err(|_| {
                    ModelSettingsError::ParseError(format!("Invalid verbosity: {}", value))
                })?;
                if !(0..=3).contains(&v) {
                    return Err(ModelSettingsError::InvalidValue(
                        "verbosity must be 0-3".to_string(),
                    ));
                }
                self.verbosity = Some(v);
            }
            _ => {
                // Ignore unknown settings for forward compatibility
            }
        }
        Ok(())
    }

    /// Save a single setting to the database.
    pub fn save_setting(
        db: &Database,
        model_name: &str,
        key: &str,
        value: &str,
    ) -> Result<(), ModelSettingsError> {
        // Validate the setting first
        let mut temp = Self::new();
        temp.apply_setting(key, value)?;

        let full_key = format!("model_settings.{}.{}", model_name, key);
        db.conn().execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            [&full_key, value],
        )?;
        Ok(())
    }

    /// Clear a setting from the database.
    pub fn clear_setting(
        db: &Database,
        model_name: &str,
        key: &str,
    ) -> Result<(), ModelSettingsError> {
        let full_key = format!("model_settings.{}.{}", model_name, key);
        db.conn()
            .execute("DELETE FROM settings WHERE key = ?", [&full_key])?;
        Ok(())
    }

    /// Clear all settings for a model.
    pub fn clear_all(db: &Database, model_name: &str) -> Result<(), ModelSettingsError> {
        let pattern = format!("model_settings.{}.%", model_name);
        db.conn()
            .execute("DELETE FROM settings WHERE key LIKE ?", [&pattern])?;
        Ok(())
    }

    /// List all settings for a model.
    pub fn list(
        db: &Database,
        model_name: &str,
    ) -> Result<Vec<(String, String)>, ModelSettingsError> {
        let prefix = format!("model_settings.{}.", model_name);
        let mut stmt = db
            .conn()
            .prepare("SELECT key, value FROM settings WHERE key LIKE ? ORDER BY key")?;

        let pattern = format!("{}%", prefix);
        let rows = stmt.query_map([&pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = Vec::new();
        for row in rows {
            let (key, value) = row?;
            let setting_key = key.strip_prefix(&prefix).unwrap_or(&key).to_string();
            settings.push((setting_key, value));
        }
        Ok(settings)
    }

    /// Get effective temperature (with default).
    pub fn effective_temperature(&self) -> f32 {
        self.temperature.unwrap_or(0.7)
    }

    /// Get effective max_tokens (with default).
    pub fn effective_max_tokens(&self) -> i32 {
        self.max_tokens.unwrap_or(16384)
    }

    /// Check if extended thinking is enabled.
    pub fn is_extended_thinking(&self) -> bool {
        self.extended_thinking.unwrap_or(false)
    }

    /// Check if interleaved thinking is enabled.
    pub fn is_interleaved_thinking(&self) -> bool {
        self.interleaved_thinking.unwrap_or(false)
    }

    /// Get a list of all valid setting keys.
    pub fn valid_keys() -> &'static [&'static str] {
        &[
            "temperature",
            "seed",
            "max_tokens",
            "extended_thinking",
            "budget_tokens",
            "interleaved_thinking",
            "reasoning_effort",
            "verbosity",
        ]
    }

    /// Check if a key is a valid model setting.
    pub fn is_valid_key(key: &str) -> bool {
        Self::valid_keys().contains(&key)
    }

    /// Check if all settings are at their defaults (None).
    pub fn is_empty(&self) -> bool {
        self.temperature.is_none()
            && self.seed.is_none()
            && self.max_tokens.is_none()
            && self.extended_thinking.is_none()
            && self.budget_tokens.is_none()
            && self.interleaved_thinking.is_none()
            && self.reasoning_effort.is_none()
            && self.verbosity.is_none()
    }
}

/// Parse a boolean from various string representations.
fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "true" | "1" | "yes" | "on" | "enabled"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let db = Database::open_at(path).unwrap();
        db.migrate().unwrap();
        (tmp, db)
    }

    #[test]
    fn test_model_settings_defaults() {
        let settings = ModelSettings::new();
        assert!(settings.temperature.is_none());
        assert_eq!(settings.effective_temperature(), 0.7);
        assert_eq!(settings.effective_max_tokens(), 16384);
    }

    #[test]
    fn test_save_and_load_settings() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "gpt-4o", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "gpt-4o", "max_tokens", "4096").unwrap();

        let settings = ModelSettings::load(&db, "gpt-4o").unwrap();
        assert_eq!(settings.temperature, Some(0.5));
        assert_eq!(settings.max_tokens, Some(4096));
    }

    #[test]
    fn test_clear_setting() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "gpt-4o", "temperature", "0.5").unwrap();
        ModelSettings::clear_setting(&db, "gpt-4o", "temperature").unwrap();

        let settings = ModelSettings::load(&db, "gpt-4o").unwrap();
        assert!(settings.temperature.is_none());
    }

    #[test]
    fn test_invalid_reasoning_effort() {
        let (_tmp, db) = setup_test_db();

        let result = ModelSettings::save_setting(&db, "o1", "reasoning_effort", "super_high");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_settings() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "claude", "temperature", "0.8").unwrap();
        ModelSettings::save_setting(&db, "claude", "extended_thinking", "true").unwrap();

        let list = ModelSettings::list(&db, "claude").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|(k, _)| k == "temperature"));
        assert!(list.iter().any(|(k, _)| k == "extended_thinking"));
    }

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool("true"));
        assert!(parse_bool("True"));
        assert!(parse_bool("TRUE"));
        assert!(parse_bool("1"));
        assert!(parse_bool("yes"));
        assert!(parse_bool("on"));
        assert!(parse_bool("enabled"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("0"));
        assert!(!parse_bool("no"));
        assert!(!parse_bool("random"));
    }

    #[test]
    fn test_valid_keys() {
        assert!(ModelSettings::is_valid_key("temperature"));
        assert!(ModelSettings::is_valid_key("reasoning_effort"));
        assert!(!ModelSettings::is_valid_key("invalid_key"));
    }
}
