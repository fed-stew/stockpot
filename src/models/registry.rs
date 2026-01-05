//! Model registry for loading and managing model configurations.
//!
//! This module provides `ModelRegistry` which handles:
//! - Loading models from the database
//! - Adding/removing models from the database
//! - Listing available models based on provider availability

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::params;

use crate::db::Database;

use super::model_config::ModelConfig;
use super::types::{CustomEndpoint, ModelConfigError, ModelType};
use super::utils::{build_custom_endpoint, has_api_key, has_oauth_tokens, parse_model_type};

/// Registry of available models loaded from configuration files.
#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: HashMap<String, ModelConfig>,
}

impl ModelRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load models with in-memory defaults only.
    /// **Deprecated**: Use `load_from_db()` instead for database-backed storage.
    #[deprecated(note = "Use load_from_db() instead")]
    pub fn load() -> Result<Self, ModelConfigError> {
        // Returns empty registry - models come from database now
        Ok(Self::new())
    }

    /// Load models from the database.
    ///
    /// Models are added explicitly via `/add_model` or OAuth flows.
    /// The models.dev catalog provides available models, and users
    /// choose which ones to configure.
    pub fn load_from_db(db: &Database) -> Result<Self, ModelConfigError> {
        let mut registry = Self::new();

        // Load all models from database
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT name, model_type, model_id, context_length, supports_thinking,
                        supports_vision, supports_tools, description, api_endpoint,
                        api_key_env, headers, azure_deployment, azure_api_version
                 FROM models ORDER BY name",
            )
            .map_err(|e| ModelConfigError::Io(std::io::Error::other(e.to_string())))?;

        let rows = stmt
            .query_map([], |row| {
                let model_type_str: String = row.get(1)?;
                let headers_json: Option<String> = row.get(10)?;

                Ok(ModelConfig {
                    name: row.get(0)?,
                    model_type: parse_model_type(&model_type_str),
                    model_id: row.get(2)?,
                    context_length: row.get::<_, i64>(3)? as usize,
                    supports_thinking: row.get::<_, i64>(4)? != 0,
                    supports_vision: row.get::<_, i64>(5)? != 0,
                    supports_tools: row.get::<_, i64>(6)? != 0,
                    description: row.get(7)?,
                    custom_endpoint: build_custom_endpoint(
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        headers_json,
                    ),
                    azure_deployment: row.get(11)?,
                    azure_api_version: row.get(12)?,
                    round_robin_models: Vec::new(),
                })
            })
            .map_err(|e| ModelConfigError::Io(std::io::Error::other(e.to_string())))?;

        for config in rows.flatten() {
            tracing::debug!(
                model = %config.name,
                model_type = %config.model_type,
                "Loaded model from database"
            );
            registry.models.insert(config.name.clone(), config);
        }

        tracing::debug!(
            total_models = registry.models.len(),
            "ModelRegistry loaded from database"
        );

        Ok(registry)
    }

    /// Add a model to the database.
    ///
    /// For backwards compatibility, this defaults to source="custom".
    /// Use `add_model_to_db_with_source` to specify the source explicitly.
    pub fn add_model_to_db(db: &Database, config: &ModelConfig) -> Result<(), ModelConfigError> {
        // Infer source from model type
        let source = match config.model_type {
            ModelType::ClaudeCode | ModelType::ChatgptOauth => "oauth",
            _ if config.custom_endpoint.is_some() => "custom",
            _ => "catalog",
        };
        Self::add_model_to_db_with_source(db, config, source)
    }

    /// Add a model to the database with explicit source tracking.
    ///
    /// Source values:
    /// - "catalog" - From the build-time models.dev catalog
    /// - "oauth" - From OAuth authentication (ChatGPT, Claude Code)
    /// - "custom" - User-added custom endpoint
    pub fn add_model_to_db_with_source(
        db: &Database,
        config: &ModelConfig,
        source: &str,
    ) -> Result<(), ModelConfigError> {
        tracing::debug!(
            model = %config.name,
            model_type = %config.model_type,
            source = %source,
            "Saving model to database"
        );

        let headers_json = config
            .custom_endpoint
            .as_ref()
            .map(|e| serde_json::to_string(&e.headers).unwrap_or_default());

        let result = db.conn().execute(
            "INSERT OR REPLACE INTO models (name, model_type, model_id, context_length,
                supports_thinking, supports_vision, supports_tools, description,
                api_endpoint, api_key_env, headers, is_builtin, source, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, unixepoch())",
            params![
                &config.name,
                config.model_type.to_string(),
                &config.model_id,
                config.context_length as i64,
                config.supports_thinking as i64,
                config.supports_vision as i64,
                config.supports_tools as i64,
                &config.description,
                config.custom_endpoint.as_ref().map(|e| &e.url),
                config
                    .custom_endpoint
                    .as_ref()
                    .and_then(|e| e.api_key.as_ref()),
                headers_json,
                source,
            ],
        );

        match &result {
            Ok(rows) => tracing::debug!(rows_affected = rows, "Model saved successfully"),
            Err(e) => tracing::error!(error = %e, model = %config.name, "Failed to save model"),
        }

        result.map_err(|e| ModelConfigError::Io(std::io::Error::other(e.to_string())))?;

        Ok(())
    }

    /// Remove a custom model from the database.
    pub fn remove_model_from_db(db: &Database, name: &str) -> Result<(), ModelConfigError> {
        db.conn()
            .execute(
                "DELETE FROM models WHERE name = ? AND is_builtin = 0",
                params![name],
            )
            .map_err(|e| ModelConfigError::Io(std::io::Error::other(e.to_string())))?;
        Ok(())
    }

    /// Reload the registry from database.
    pub fn reload_from_db(&mut self, db: &Database) -> Result<(), ModelConfigError> {
        self.models.clear();
        let fresh = Self::load_from_db(db)?;
        self.models = fresh.models;
        Ok(())
    }

    /// Load models from a specific JSON file.
    pub fn load_file(&mut self, path: &PathBuf) -> Result<(), ModelConfigError> {
        let content = std::fs::read_to_string(path)?;
        let models: Vec<ModelConfig> = serde_json::from_str(&content)?;

        for model in models {
            self.models.insert(model.name.clone(), model);
        }

        Ok(())
    }

    /// Add a model to the registry.
    pub fn add(&mut self, config: ModelConfig) {
        self.models.insert(config.name.clone(), config);
    }

    /// Get a model by name.
    pub fn get(&self, name: &str) -> Option<&ModelConfig> {
        self.models.get(name)
    }

    /// Check if a model exists.
    pub fn contains(&self, name: &str) -> bool {
        self.models.contains_key(name)
    }

    /// Get all model names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(|s| s.as_str())
    }

    /// Get all model names as a sorted vector.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.models.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Get all models.
    pub fn all(&self) -> impl Iterator<Item = &ModelConfig> {
        self.models.values()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Number of models in the registry.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Reload the registry with in-memory defaults only.
    /// **Deprecated**: Use `reload_from_db()` instead for database-backed storage.
    #[deprecated(note = "Use reload_from_db() instead")]
    pub fn reload(&mut self) -> Result<(), ModelConfigError> {
        // Just clear - models come from database now
        self.models.clear();
        Ok(())
    }

    /// Get the config directory path.
    pub fn config_dir() -> Result<PathBuf, ModelConfigError> {
        let home = dirs::home_dir().ok_or(ModelConfigError::ConfigDirNotFound)?;
        Ok(home.join(".stockpot"))
    }

    /// List only models that have valid provider configuration.
    /// Checks for API keys in database or environment, or OAuth tokens.
    pub fn list_available(&self, db: &Database) -> Vec<String> {
        tracing::debug!(
            total_in_registry = self.models.len(),
            "list_available: checking models"
        );

        let mut available: Vec<String> = self
            .models
            .iter()
            .filter(|(name, config)| {
                let is_available = self.is_provider_available(db, name, config);
                tracing::debug!(
                    model = %name,
                    model_type = %config.model_type,
                    available = is_available,
                    "Provider availability check"
                );
                is_available
            })
            .map(|(name, _)| name.clone())
            .collect();
        available.sort();

        tracing::debug!(
            available_count = available.len(),
            "list_available: filtered result"
        );

        available
    }

    /// Check if a model's provider is available (has API key in DB/env or OAuth tokens).
    fn is_provider_available(&self, db: &Database, _name: &str, config: &ModelConfig) -> bool {
        match config.model_type {
            ModelType::Openai => has_api_key(db, "OPENAI_API_KEY"),
            ModelType::Anthropic => has_api_key(db, "ANTHROPIC_API_KEY"),
            ModelType::Gemini => {
                has_api_key(db, "GEMINI_API_KEY") || has_api_key(db, "GOOGLE_API_KEY")
            }
            ModelType::ClaudeCode => {
                // Check if we have valid OAuth tokens
                has_oauth_tokens(db, "claude-code")
            }
            ModelType::ChatgptOauth => {
                // Check if we have valid OAuth tokens
                has_oauth_tokens(db, "chatgpt")
            }
            ModelType::AzureOpenai => {
                has_api_key(db, "AZURE_OPENAI_API_KEY") || has_api_key(db, "AZURE_OPENAI_ENDPOINT")
            }
            ModelType::CustomOpenai | ModelType::CustomAnthropic => {
                // Custom endpoints - check if API key is configured
                // The api_key can be a literal or $ENV_VAR reference
                config
                    .custom_endpoint
                    .as_ref()
                    .map(|e| {
                        e.api_key.as_ref().is_some_and(|key| {
                            if key.starts_with('$') {
                                // It's an env var reference, check DB then env
                                let var_name = key
                                    .trim_start_matches('$')
                                    .trim_matches(|c| c == '{' || c == '}');
                                has_api_key(db, var_name)
                            } else {
                                // It's a literal key
                                !key.is_empty()
                            }
                        })
                    })
                    .unwrap_or(false)
            }
            ModelType::Openrouter => has_api_key(db, "OPENROUTER_API_KEY"),
            ModelType::RoundRobin => true, // Round robin is always "available" if it exists
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_starts_empty() {
        let registry = ModelRegistry::new();
        assert!(registry.is_empty());
        // Models are now added explicitly via /add_model or OAuth
        // The catalog provides available models, database stores configured ones
    }
}
