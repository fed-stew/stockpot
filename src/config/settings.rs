//! Settings management via SQLite.

use std::collections::HashMap;

use crate::db::Database;
use thiserror::Error;

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
        let result: Result<String, _> = self.db.conn().query_row(
            "SELECT value FROM settings WHERE key = ?",
            [key],
            |row| row.get(0),
        );
        
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SettingsError::Database(e)),
        }
    }

    /// Get a setting value or return a default.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).ok().flatten().unwrap_or_else(|| default.to_string())
    }

    /// Get a boolean setting.
    pub fn get_bool(&self, key: &str) -> Result<bool, SettingsError> {
        match self.get(key)? {
            Some(v) => Ok(matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on")),
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
        self.db.conn().execute("DELETE FROM settings WHERE key = ?", [key])?;
        Ok(())
    }

    /// List all settings.
    pub fn list(&self) -> Result<Vec<(String, String)>, SettingsError> {
        let mut stmt = self.db.conn().prepare("SELECT key, value FROM settings ORDER BY key")?;
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

    /// Get YOLO mode status.
    pub fn yolo_mode(&self) -> bool {
        self.get_bool("yolo_mode").unwrap_or(false)
    }

    /// Get the assistant name.
    pub fn assistant_name(&self) -> String {
        self.get_or("assistant_name", "Stockpot")
    }

    /// Get the owner name.
    pub fn owner_name(&self) -> String {
        self.get_or("owner_name", "Master")
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
    pub fn set_agent_pinned_model(&self, agent_name: &str, model_name: &str) -> Result<(), SettingsError> {
        self.set(&Self::agent_pin_key(agent_name), model_name)
    }

    /// Clear the pinned model for an agent.
    pub fn clear_agent_pinned_model(&self, agent_name: &str) -> Result<(), SettingsError> {
        self.delete(&Self::agent_pin_key(agent_name))
    }

    /// Get all agent->model pin mappings.
    pub fn get_all_agent_pinned_models(&self) -> Result<HashMap<String, String>, SettingsError> {
        let prefix = "agent_pin.";
        let mut stmt = self.db.conn().prepare(
            "SELECT key, value FROM settings WHERE key LIKE ? ORDER BY key"
        )?;
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
}
