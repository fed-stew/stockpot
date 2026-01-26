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
    /// Top-p (nucleus) sampling (0.0 - 1.0)
    pub top_p: Option<f32>,
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
            "top_p" => {
                self.top_p = Some(value.parse().map_err(|_| {
                    ModelSettingsError::ParseError(format!("Invalid top_p: {}", value))
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

    /// Get effective top_p (with default).
    pub fn effective_top_p(&self) -> f32 {
        self.top_p.unwrap_or(1.0)
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
            "top_p",
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
            && self.top_p.is_none()
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

    // =========================================================================
    // Test Setup
    // =========================================================================

    fn setup_test_db() -> (TempDir, Database) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let db = Database::open_at(path).unwrap();
        db.migrate().unwrap();
        (tmp, db)
    }

    // =========================================================================
    // Default Settings Tests
    // =========================================================================

    #[test]
    fn test_model_settings_defaults() {
        let settings = ModelSettings::new();
        assert!(settings.temperature.is_none());
        assert_eq!(settings.effective_temperature(), 0.7);
        assert_eq!(settings.effective_max_tokens(), 16384);
    }

    #[test]
    fn test_model_settings_default_trait() {
        let settings = ModelSettings::default();
        assert!(settings.temperature.is_none());
        assert!(settings.seed.is_none());
        assert!(settings.max_tokens.is_none());
        assert!(settings.extended_thinking.is_none());
        assert!(settings.budget_tokens.is_none());
        assert!(settings.interleaved_thinking.is_none());
        assert!(settings.reasoning_effort.is_none());
        assert!(settings.verbosity.is_none());
    }

    #[test]
    fn test_is_empty_all_none() {
        let settings = ModelSettings::new();
        assert!(settings.is_empty());
    }

    #[test]
    fn test_is_empty_with_one_set() {
        let mut settings = ModelSettings::new();
        settings.temperature = Some(0.5);
        assert!(!settings.is_empty());
    }

    #[test]
    fn test_is_empty_all_fields_checked() {
        let mut settings = ModelSettings::new();

        settings.temperature = Some(0.5);
        assert!(!settings.is_empty());
        settings.temperature = None;

        settings.seed = Some(42);
        assert!(!settings.is_empty());
        settings.seed = None;

        settings.max_tokens = Some(1000);
        assert!(!settings.is_empty());
        settings.max_tokens = None;

        settings.extended_thinking = Some(true);
        assert!(!settings.is_empty());
        settings.extended_thinking = None;

        settings.budget_tokens = Some(5000);
        assert!(!settings.is_empty());
        settings.budget_tokens = None;

        settings.interleaved_thinking = Some(false);
        assert!(!settings.is_empty());
        settings.interleaved_thinking = None;

        settings.reasoning_effort = Some("high".to_string());
        assert!(!settings.is_empty());
        settings.reasoning_effort = None;

        settings.verbosity = Some(2);
        assert!(!settings.is_empty());
    }

    // =========================================================================
    // Effective Value Tests
    // =========================================================================

    #[test]
    fn test_effective_temperature_with_value() {
        let mut settings = ModelSettings::new();
        settings.temperature = Some(0.3);
        assert_eq!(settings.effective_temperature(), 0.3);
    }

    #[test]
    fn test_effective_max_tokens_with_value() {
        let mut settings = ModelSettings::new();
        settings.max_tokens = Some(8192);
        assert_eq!(settings.effective_max_tokens(), 8192);
    }

    #[test]
    fn test_is_extended_thinking_default_false() {
        let settings = ModelSettings::new();
        assert!(!settings.is_extended_thinking());
    }

    #[test]
    fn test_is_extended_thinking_when_set() {
        let mut settings = ModelSettings::new();
        settings.extended_thinking = Some(true);
        assert!(settings.is_extended_thinking());

        settings.extended_thinking = Some(false);
        assert!(!settings.is_extended_thinking());
    }

    #[test]
    fn test_is_interleaved_thinking_default_false() {
        let settings = ModelSettings::new();
        assert!(!settings.is_interleaved_thinking());
    }

    #[test]
    fn test_is_interleaved_thinking_when_set() {
        let mut settings = ModelSettings::new();
        settings.interleaved_thinking = Some(true);
        assert!(settings.is_interleaved_thinking());
    }

    // =========================================================================
    // Save and Load Tests
    // =========================================================================

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
    fn test_save_and_load_all_setting_types() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test", "temperature", "0.9").unwrap();
        ModelSettings::save_setting(&db, "test", "seed", "12345").unwrap();
        ModelSettings::save_setting(&db, "test", "max_tokens", "8192").unwrap();
        ModelSettings::save_setting(&db, "test", "extended_thinking", "true").unwrap();
        ModelSettings::save_setting(&db, "test", "budget_tokens", "10000").unwrap();
        ModelSettings::save_setting(&db, "test", "interleaved_thinking", "yes").unwrap();
        ModelSettings::save_setting(&db, "test", "reasoning_effort", "high").unwrap();
        ModelSettings::save_setting(&db, "test", "verbosity", "2").unwrap();

        let settings = ModelSettings::load(&db, "test").unwrap();

        assert_eq!(settings.temperature, Some(0.9));
        assert_eq!(settings.seed, Some(12345));
        assert_eq!(settings.max_tokens, Some(8192));
        assert_eq!(settings.extended_thinking, Some(true));
        assert_eq!(settings.budget_tokens, Some(10000));
        assert_eq!(settings.interleaved_thinking, Some(true));
        assert_eq!(settings.reasoning_effort, Some("high".to_string()));
        assert_eq!(settings.verbosity, Some(2));
    }

    #[test]
    fn test_load_nonexistent_model() {
        let (_tmp, db) = setup_test_db();
        let settings = ModelSettings::load(&db, "nonexistent").unwrap();
        assert!(settings.is_empty());
    }

    #[test]
    fn test_save_overwrites_existing() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "test", "temperature", "0.9").unwrap();

        let settings = ModelSettings::load(&db, "test").unwrap();
        assert_eq!(settings.temperature, Some(0.9));
    }

    // =========================================================================
    // Clear Settings Tests
    // =========================================================================

    #[test]
    fn test_clear_setting() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "gpt-4o", "temperature", "0.5").unwrap();
        ModelSettings::clear_setting(&db, "gpt-4o", "temperature").unwrap();

        let settings = ModelSettings::load(&db, "gpt-4o").unwrap();
        assert!(settings.temperature.is_none());
    }

    #[test]
    fn test_clear_nonexistent_setting() {
        let (_tmp, db) = setup_test_db();
        // Should not error when clearing a setting that doesn't exist
        let result = ModelSettings::clear_setting(&db, "test", "temperature");
        assert!(result.is_ok());
    }

    #[test]
    fn test_clear_all_settings() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "test", "max_tokens", "4096").unwrap();
        ModelSettings::save_setting(&db, "test", "seed", "123").unwrap();

        ModelSettings::clear_all(&db, "test").unwrap();

        let settings = ModelSettings::load(&db, "test").unwrap();
        assert!(settings.is_empty());
    }

    #[test]
    fn test_clear_all_only_affects_target_model() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "model1", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "model2", "temperature", "0.8").unwrap();

        ModelSettings::clear_all(&db, "model1").unwrap();

        let settings1 = ModelSettings::load(&db, "model1").unwrap();
        let settings2 = ModelSettings::load(&db, "model2").unwrap();

        assert!(settings1.is_empty());
        assert_eq!(settings2.temperature, Some(0.8));
    }

    // =========================================================================
    // List Settings Tests
    // =========================================================================

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
    fn test_list_settings_empty() {
        let (_tmp, db) = setup_test_db();
        let list = ModelSettings::list(&db, "nonexistent").unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_settings_only_target_model() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "model1", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "model2", "temperature", "0.8").unwrap();

        let list = ModelSettings::list(&db, "model1").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], ("temperature".to_string(), "0.5".to_string()));
    }

    // =========================================================================
    // Validation Tests - Temperature
    // =========================================================================

    #[test]
    fn test_temperature_valid_values() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test", "temperature", "0").unwrap();
        ModelSettings::save_setting(&db, "test", "temperature", "1.0").unwrap();
        ModelSettings::save_setting(&db, "test", "temperature", "2.0").unwrap();
        ModelSettings::save_setting(&db, "test", "temperature", "0.5").unwrap();
    }

    #[test]
    fn test_temperature_invalid_not_a_number() {
        let (_tmp, db) = setup_test_db();
        let result = ModelSettings::save_setting(&db, "test", "temperature", "abc");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ModelSettingsError::ParseError(_)
        ));
    }

    // =========================================================================
    // Validation Tests - Seed
    // =========================================================================

    #[test]
    fn test_seed_valid_values() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test", "seed", "0").unwrap();
        ModelSettings::save_setting(&db, "test", "seed", "42").unwrap();
        ModelSettings::save_setting(&db, "test", "seed", "-1").unwrap();
        ModelSettings::save_setting(&db, "test", "seed", "9999999999").unwrap();
    }

    #[test]
    fn test_seed_invalid_not_a_number() {
        let (_tmp, db) = setup_test_db();
        let result = ModelSettings::save_setting(&db, "test", "seed", "not_a_number");
        assert!(result.is_err());
    }

    // =========================================================================
    // Validation Tests - Max Tokens
    // =========================================================================

    #[test]
    fn test_max_tokens_valid() {
        let (_tmp, db) = setup_test_db();
        ModelSettings::save_setting(&db, "test", "max_tokens", "4096").unwrap();
    }

    #[test]
    fn test_max_tokens_invalid() {
        let (_tmp, db) = setup_test_db();
        let result = ModelSettings::save_setting(&db, "test", "max_tokens", "many");
        assert!(result.is_err());
    }

    // =========================================================================
    // Validation Tests - Reasoning Effort
    // =========================================================================

    #[test]
    fn test_reasoning_effort_all_valid_values() {
        let (_tmp, db) = setup_test_db();

        for effort in ["minimal", "low", "medium", "high", "xhigh"] {
            ModelSettings::save_setting(&db, "test", "reasoning_effort", effort).unwrap();
        }
    }

    #[test]
    fn test_reasoning_effort_case_insensitive() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test", "reasoning_effort", "HIGH").unwrap();
        let settings = ModelSettings::load(&db, "test").unwrap();
        assert_eq!(settings.reasoning_effort, Some("high".to_string()));

        ModelSettings::save_setting(&db, "test", "reasoning_effort", "Medium").unwrap();
        let settings = ModelSettings::load(&db, "test").unwrap();
        assert_eq!(settings.reasoning_effort, Some("medium".to_string()));
    }

    #[test]
    fn test_invalid_reasoning_effort() {
        let (_tmp, db) = setup_test_db();

        let result = ModelSettings::save_setting(&db, "o1", "reasoning_effort", "super_high");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ModelSettingsError::InvalidValue(_)
        ));
    }

    // =========================================================================
    // Validation Tests - Verbosity
    // =========================================================================

    #[test]
    fn test_verbosity_valid_range() {
        let (_tmp, db) = setup_test_db();

        for v in 0..=3 {
            ModelSettings::save_setting(&db, "test", "verbosity", &v.to_string()).unwrap();
        }
    }

    #[test]
    fn test_verbosity_out_of_range() {
        let (_tmp, db) = setup_test_db();

        let result = ModelSettings::save_setting(&db, "test", "verbosity", "4");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ModelSettingsError::InvalidValue(_)
        ));

        let result = ModelSettings::save_setting(&db, "test", "verbosity", "-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_verbosity_not_a_number() {
        let (_tmp, db) = setup_test_db();
        let result = ModelSettings::save_setting(&db, "test", "verbosity", "verbose");
        assert!(result.is_err());
    }

    // =========================================================================
    // Validation Tests - Budget Tokens
    // =========================================================================

    #[test]
    fn test_budget_tokens_valid() {
        let (_tmp, db) = setup_test_db();
        ModelSettings::save_setting(&db, "test", "budget_tokens", "10000").unwrap();
    }

    #[test]
    fn test_budget_tokens_invalid() {
        let (_tmp, db) = setup_test_db();
        let result = ModelSettings::save_setting(&db, "test", "budget_tokens", "lots");
        assert!(result.is_err());
    }

    // =========================================================================
    // Boolean Parsing Tests
    // =========================================================================

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
    fn test_parse_bool_edge_cases() {
        assert!(!parse_bool(""));
        assert!(!parse_bool("tru"));
        assert!(!parse_bool("truee"));
        assert!(!parse_bool(" true"));
        assert!(!parse_bool("true "));
        assert!(!parse_bool("yess"));
        assert!(!parse_bool("off"));
        assert!(!parse_bool("disabled"));
    }

    #[test]
    fn test_extended_thinking_boolean_parsing() {
        let (_tmp, db) = setup_test_db();

        for (value, expected) in [
            ("true", true),
            ("1", true),
            ("yes", true),
            ("on", true),
            ("enabled", true),
            ("false", false),
            ("0", false),
            ("no", false),
        ] {
            ModelSettings::save_setting(&db, "test", "extended_thinking", value).unwrap();
            let settings = ModelSettings::load(&db, "test").unwrap();
            assert_eq!(
                settings.extended_thinking,
                Some(expected),
                "Failed for value: {}",
                value
            );
        }
    }

    // =========================================================================
    // Valid Keys Tests
    // =========================================================================

    #[test]
    fn test_valid_keys() {
        assert!(ModelSettings::is_valid_key("temperature"));
        assert!(ModelSettings::is_valid_key("reasoning_effort"));
        assert!(!ModelSettings::is_valid_key("invalid_key"));
    }

    #[test]
    fn test_valid_keys_list() {
        let keys = ModelSettings::valid_keys();
        assert!(keys.contains(&"temperature"));
        assert!(keys.contains(&"seed"));
        assert!(keys.contains(&"max_tokens"));
        assert!(keys.contains(&"extended_thinking"));
        assert!(keys.contains(&"budget_tokens"));
        assert!(keys.contains(&"interleaved_thinking"));
        assert!(keys.contains(&"reasoning_effort"));
        assert!(keys.contains(&"verbosity"));
        assert_eq!(keys.len(), 9);
    }

    #[test]
    fn test_is_valid_key_all_keys() {
        for key in ModelSettings::valid_keys() {
            assert!(ModelSettings::is_valid_key(key));
        }
    }

    // =========================================================================
    // Unknown Key Handling Tests
    // =========================================================================

    #[test]
    fn test_unknown_key_ignored_in_apply_setting() {
        let mut settings = ModelSettings::new();
        // Unknown keys should be silently ignored for forward compatibility
        let result = settings.apply_setting("future_setting", "value");
        assert!(result.is_ok());
        assert!(settings.is_empty());
    }

    // =========================================================================
    // Error Display Tests
    // =========================================================================

    #[test]
    fn test_error_display_database() {
        let err = ModelSettingsError::Database(rusqlite::Error::InvalidQuery);
        let msg = format!("{}", err);
        assert!(msg.contains("Database error"));
    }

    #[test]
    fn test_error_display_invalid_value() {
        let err = ModelSettingsError::InvalidValue("verbosity must be 0-3".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid setting value"));
    }

    #[test]
    fn test_error_display_parse_error() {
        let err = ModelSettingsError::ParseError("Invalid temperature: abc".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("parse error"));
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_model_settings_serde() {
        let mut settings = ModelSettings::new();
        settings.temperature = Some(0.8);
        settings.max_tokens = Some(4096);
        settings.extended_thinking = Some(true);

        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ModelSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.temperature, Some(0.8));
        assert_eq!(parsed.max_tokens, Some(4096));
        assert_eq!(parsed.extended_thinking, Some(true));
    }

    #[test]
    fn test_model_settings_serde_empty() {
        let settings = ModelSettings::new();
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ModelSettings = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_empty());
    }

    // =========================================================================
    // Model Name Edge Cases
    // =========================================================================

    #[test]
    fn test_model_name_with_special_characters() {
        let (_tmp, db) = setup_test_db();

        // Model names can contain dots, hyphens, underscores
        ModelSettings::save_setting(&db, "gpt-4o-2024-08-06", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "claude_3.5_sonnet", "temperature", "0.6").unwrap();

        let settings1 = ModelSettings::load(&db, "gpt-4o-2024-08-06").unwrap();
        let settings2 = ModelSettings::load(&db, "claude_3.5_sonnet").unwrap();

        assert_eq!(settings1.temperature, Some(0.5));
        assert_eq!(settings2.temperature, Some(0.6));
    }

    #[test]
    fn test_model_isolation() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "gpt-4o", "temperature", "0.5").unwrap();
        ModelSettings::save_setting(&db, "claude", "temperature", "0.9").unwrap();

        let gpt_settings = ModelSettings::load(&db, "gpt-4o").unwrap();
        let claude_settings = ModelSettings::load(&db, "claude").unwrap();

        assert_eq!(gpt_settings.temperature, Some(0.5));
        assert_eq!(claude_settings.temperature, Some(0.9));
    }

    // =========================================================================
    // apply_setting Direct Tests
    // =========================================================================

    #[test]
    fn test_apply_setting_temperature_direct() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("temperature", "0.5").unwrap();
        assert_eq!(settings.temperature, Some(0.5));
    }

    #[test]
    fn test_apply_setting_seed_direct() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("seed", "42").unwrap();
        assert_eq!(settings.seed, Some(42));
    }

    #[test]
    fn test_apply_setting_max_tokens_direct() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("max_tokens", "8192").unwrap();
        assert_eq!(settings.max_tokens, Some(8192));
    }

    #[test]
    fn test_apply_setting_extended_thinking_direct() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("extended_thinking", "true").unwrap();
        assert_eq!(settings.extended_thinking, Some(true));
    }

    #[test]
    fn test_apply_setting_budget_tokens_direct() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("budget_tokens", "5000").unwrap();
        assert_eq!(settings.budget_tokens, Some(5000));
    }

    #[test]
    fn test_apply_setting_interleaved_thinking_direct() {
        let mut settings = ModelSettings::new();
        settings
            .apply_setting("interleaved_thinking", "yes")
            .unwrap();
        assert_eq!(settings.interleaved_thinking, Some(true));
    }

    #[test]
    fn test_apply_setting_reasoning_effort_direct() {
        let mut settings = ModelSettings::new();
        settings
            .apply_setting("reasoning_effort", "medium")
            .unwrap();
        assert_eq!(settings.reasoning_effort, Some("medium".to_string()));
    }

    #[test]
    fn test_apply_setting_verbosity_direct() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("verbosity", "2").unwrap();
        assert_eq!(settings.verbosity, Some(2));
    }

    // =========================================================================
    // Numeric Edge Cases
    // =========================================================================

    #[test]
    fn test_temperature_negative() {
        let mut settings = ModelSettings::new();
        // Negative temps parse but may be invalid for API - validation is loose here
        settings.apply_setting("temperature", "-0.5").unwrap();
        assert_eq!(settings.temperature, Some(-0.5));
    }

    #[test]
    fn test_temperature_very_small() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("temperature", "0.0001").unwrap();
        assert_eq!(settings.temperature, Some(0.0001));
    }

    #[test]
    fn test_temperature_very_large() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("temperature", "100.0").unwrap();
        assert_eq!(settings.temperature, Some(100.0));
    }

    #[test]
    fn test_seed_negative() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("seed", "-999").unwrap();
        assert_eq!(settings.seed, Some(-999));
    }

    #[test]
    fn test_seed_zero() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("seed", "0").unwrap();
        assert_eq!(settings.seed, Some(0));
    }

    #[test]
    fn test_seed_large() {
        let mut settings = ModelSettings::new();
        settings
            .apply_setting("seed", "9223372036854775807")
            .unwrap(); // i64::MAX
        assert_eq!(settings.seed, Some(i64::MAX));
    }

    #[test]
    fn test_max_tokens_zero() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("max_tokens", "0").unwrap();
        assert_eq!(settings.max_tokens, Some(0));
    }

    #[test]
    fn test_max_tokens_negative() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("max_tokens", "-100").unwrap();
        assert_eq!(settings.max_tokens, Some(-100));
    }

    #[test]
    fn test_budget_tokens_zero() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("budget_tokens", "0").unwrap();
        assert_eq!(settings.budget_tokens, Some(0));
    }

    #[test]
    fn test_budget_tokens_negative() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("budget_tokens", "-1000").unwrap();
        assert_eq!(settings.budget_tokens, Some(-1000));
    }

    // =========================================================================
    // Empty String Handling
    // =========================================================================

    #[test]
    fn test_temperature_empty_string() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("temperature", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_seed_empty_string() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("seed", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_max_tokens_empty_string() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("max_tokens", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_tokens_empty_string() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("budget_tokens", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_verbosity_empty_string() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("verbosity", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_reasoning_effort_empty_string() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("reasoning_effort", "");
        assert!(result.is_err());
    }

    // =========================================================================
    // Clone Trait Tests
    // =========================================================================

    #[test]
    fn test_model_settings_clone() {
        let mut original = ModelSettings::new();
        original.temperature = Some(0.7);
        original.max_tokens = Some(4096);
        original.extended_thinking = Some(true);
        original.reasoning_effort = Some("high".to_string());

        let cloned = original.clone();

        assert_eq!(cloned.temperature, Some(0.7));
        assert_eq!(cloned.max_tokens, Some(4096));
        assert_eq!(cloned.extended_thinking, Some(true));
        assert_eq!(cloned.reasoning_effort, Some("high".to_string()));
    }

    #[test]
    fn test_model_settings_clone_independence() {
        let mut original = ModelSettings::new();
        original.temperature = Some(0.5);

        let mut cloned = original.clone();
        cloned.temperature = Some(0.9);

        assert_eq!(original.temperature, Some(0.5));
        assert_eq!(cloned.temperature, Some(0.9));
    }

    // =========================================================================
    // Whitespace and Formatting Edge Cases
    // =========================================================================

    #[test]
    fn test_temperature_with_whitespace() {
        let mut settings = ModelSettings::new();
        // Leading/trailing whitespace should fail parse
        let result = settings.apply_setting("temperature", " 0.5 ");
        assert!(result.is_err());
    }

    #[test]
    fn test_reasoning_effort_with_whitespace() {
        let mut settings = ModelSettings::new();
        // Whitespace should fail - "high " != "high"
        let result = settings.apply_setting("reasoning_effort", " high ");
        assert!(result.is_err());
    }

    #[test]
    fn test_extended_thinking_with_whitespace() {
        let mut settings = ModelSettings::new();
        // parse_bool doesn't trim, so " true" != "true"
        settings
            .apply_setting("extended_thinking", " true")
            .unwrap();
        assert_eq!(settings.extended_thinking, Some(false)); // Won't match
    }

    // =========================================================================
    // Reasoning Effort All Variants
    // =========================================================================

    #[test]
    fn test_reasoning_effort_minimal() {
        let mut settings = ModelSettings::new();
        settings
            .apply_setting("reasoning_effort", "minimal")
            .unwrap();
        assert_eq!(settings.reasoning_effort, Some("minimal".to_string()));
    }

    #[test]
    fn test_reasoning_effort_xhigh() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("reasoning_effort", "xhigh").unwrap();
        assert_eq!(settings.reasoning_effort, Some("xhigh".to_string()));
    }

    #[test]
    fn test_reasoning_effort_invalid_variations() {
        let mut settings = ModelSettings::new();

        for invalid in ["max", "extreme", "ultra", "highest", "lowest", "mid", ""] {
            let result = settings.apply_setting("reasoning_effort", invalid);
            assert!(result.is_err(), "Expected error for: {}", invalid);
        }
    }

    // =========================================================================
    // Verbosity Boundary Tests
    // =========================================================================

    #[test]
    fn test_verbosity_boundary_0() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("verbosity", "0").unwrap();
        assert_eq!(settings.verbosity, Some(0));
    }

    #[test]
    fn test_verbosity_boundary_3() {
        let mut settings = ModelSettings::new();
        settings.apply_setting("verbosity", "3").unwrap();
        assert_eq!(settings.verbosity, Some(3));
    }

    #[test]
    fn test_verbosity_just_under_min() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("verbosity", "-1");
        assert!(matches!(result, Err(ModelSettingsError::InvalidValue(_))));
    }

    #[test]
    fn test_verbosity_just_over_max() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("verbosity", "4");
        assert!(matches!(result, Err(ModelSettingsError::InvalidValue(_))));
    }

    // =========================================================================
    // Serialization Edge Cases
    // =========================================================================

    #[test]
    fn test_serde_all_fields_set() {
        let mut settings = ModelSettings::new();
        settings.temperature = Some(0.5);
        settings.seed = Some(42);
        settings.max_tokens = Some(8192);
        settings.extended_thinking = Some(true);
        settings.budget_tokens = Some(5000);
        settings.interleaved_thinking = Some(false);
        settings.reasoning_effort = Some("high".to_string());
        settings.verbosity = Some(2);

        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ModelSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.temperature, Some(0.5));
        assert_eq!(parsed.seed, Some(42));
        assert_eq!(parsed.max_tokens, Some(8192));
        assert_eq!(parsed.extended_thinking, Some(true));
        assert_eq!(parsed.budget_tokens, Some(5000));
        assert_eq!(parsed.interleaved_thinking, Some(false));
        assert_eq!(parsed.reasoning_effort, Some("high".to_string()));
        assert_eq!(parsed.verbosity, Some(2));
    }

    #[test]
    fn test_serde_deserialize_partial() {
        let json = r#"{"temperature": 0.8}"#;
        let parsed: ModelSettings = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.temperature, Some(0.8));
        assert!(parsed.seed.is_none());
        assert!(parsed.max_tokens.is_none());
    }

    #[test]
    fn test_serde_deserialize_unknown_fields() {
        // With default serde behavior, unknown fields are ignored
        let json = r#"{"temperature": 0.8, "unknown_field": "value"}"#;
        let parsed: ModelSettings = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.temperature, Some(0.8));
    }

    // =========================================================================
    // Model Name Edge Cases
    // =========================================================================

    #[test]
    fn test_model_name_empty() {
        let (_tmp, db) = setup_test_db();

        // Empty model name should still work
        ModelSettings::save_setting(&db, "", "temperature", "0.5").unwrap();
        let settings = ModelSettings::load(&db, "").unwrap();
        assert_eq!(settings.temperature, Some(0.5));
    }

    #[test]
    fn test_model_name_with_colons() {
        let (_tmp, db) = setup_test_db();

        // Provider:model format
        ModelSettings::save_setting(&db, "anthropic:claude-3-opus", "temperature", "0.5").unwrap();
        let settings = ModelSettings::load(&db, "anthropic:claude-3-opus").unwrap();
        assert_eq!(settings.temperature, Some(0.5));
    }

    #[test]
    fn test_model_name_with_slashes() {
        let (_tmp, db) = setup_test_db();

        // Path-like model names
        ModelSettings::save_setting(&db, "org/model/v1", "temperature", "0.5").unwrap();
        let settings = ModelSettings::load(&db, "org/model/v1").unwrap();
        assert_eq!(settings.temperature, Some(0.5));
    }

    // =========================================================================
    // DB Key Format Verification
    // =========================================================================

    #[test]
    fn test_list_returns_stripped_keys() {
        let (_tmp, db) = setup_test_db();

        ModelSettings::save_setting(&db, "test-model", "temperature", "0.5").unwrap();
        let list = ModelSettings::list(&db, "test-model").unwrap();

        // Keys should be stripped of prefix
        assert_eq!(list[0].0, "temperature");
        // Not "model_settings.test-model.temperature"
    }

    // =========================================================================
    // Debug Trait Test
    // =========================================================================

    #[test]
    fn test_model_settings_debug() {
        let settings = ModelSettings::new();
        let debug_str = format!("{:?}", settings);
        assert!(debug_str.contains("ModelSettings"));
    }

    #[test]
    fn test_model_settings_error_debug() {
        let err = ModelSettingsError::InvalidValue("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidValue"));
    }

    // =========================================================================
    // Special Float Values
    // =========================================================================

    #[test]
    fn test_temperature_infinity() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("temperature", "inf");
        // "inf" should parse as infinity in Rust
        assert!(result.is_ok());
        assert!(settings.temperature.unwrap().is_infinite());
    }

    #[test]
    fn test_temperature_nan() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("temperature", "NaN");
        assert!(result.is_ok());
        assert!(settings.temperature.unwrap().is_nan());
    }

    // =========================================================================
    // Multiple Settings Interaction
    // =========================================================================

    #[test]
    fn test_apply_multiple_settings_sequence() {
        let mut settings = ModelSettings::new();

        settings.apply_setting("temperature", "0.5").unwrap();
        settings.apply_setting("max_tokens", "4096").unwrap();
        settings.apply_setting("verbosity", "2").unwrap();

        assert_eq!(settings.temperature, Some(0.5));
        assert_eq!(settings.max_tokens, Some(4096));
        assert_eq!(settings.verbosity, Some(2));
    }

    #[test]
    fn test_apply_setting_overwrites() {
        let mut settings = ModelSettings::new();

        settings.apply_setting("temperature", "0.5").unwrap();
        settings.apply_setting("temperature", "0.9").unwrap();

        assert_eq!(settings.temperature, Some(0.9));
    }

    // =========================================================================
    // Error Source Chain
    // =========================================================================

    #[test]
    fn test_parse_error_contains_value() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("temperature", "not_a_float");

        match result {
            Err(ModelSettingsError::ParseError(msg)) => {
                assert!(msg.contains("not_a_float"));
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_invalid_value_error_message() {
        let mut settings = ModelSettings::new();
        let result = settings.apply_setting("verbosity", "5");

        match result {
            Err(ModelSettingsError::InvalidValue(msg)) => {
                assert!(msg.contains("0-3"));
            }
            _ => panic!("Expected InvalidValue"),
        }
    }

    // =========================================================================
    // Boolean Parse False Cases
    // =========================================================================

    #[test]
    fn test_interleaved_thinking_false_values() {
        let test_cases = [
            ("false", false),
            ("False", false),
            ("FALSE", false),
            ("0", false),
            ("no", false),
            ("off", false),
            ("disabled", false),
            ("nope", false),
            ("n", false),
        ];

        for (value, expected) in test_cases {
            let mut settings = ModelSettings::new();
            settings
                .apply_setting("interleaved_thinking", value)
                .unwrap();
            assert_eq!(
                settings.interleaved_thinking,
                Some(expected),
                "Failed for: {}",
                value
            );
        }
    }
}
