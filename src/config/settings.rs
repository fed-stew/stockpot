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
            .map(|s| s.split(',').map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).collect())
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
}
