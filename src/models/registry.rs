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
use super::types::{ModelConfigError, ModelType};
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
                api_endpoint, api_key_env, headers, azure_deployment, azure_api_version,
                is_builtin, source, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, unixepoch())",
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
                &config.azure_deployment,
                &config.azure_api_version,
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
    use crate::models::types::CustomEndpoint;
    use std::collections::HashMap;
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

    fn create_test_model(name: &str) -> ModelConfig {
        ModelConfig {
            name: name.to_string(),
            model_type: ModelType::Openai,
            model_id: Some(format!("{}-id", name)),
            context_length: 128_000,
            supports_thinking: false,
            supports_vision: true,
            supports_tools: true,
            description: Some(format!("Test model: {}", name)),
            custom_endpoint: None,
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        }
    }

    fn create_custom_model(name: &str, url: &str, api_key: Option<&str>) -> ModelConfig {
        ModelConfig {
            name: name.to_string(),
            model_type: ModelType::CustomOpenai,
            model_id: None,
            context_length: 8192,
            supports_thinking: false,
            supports_vision: false,
            supports_tools: true,
            description: None,
            custom_endpoint: Some(CustomEndpoint {
                url: url.to_string(),
                api_key: api_key.map(|s| s.to_string()),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        }
    }

    // =========================================================================
    // Basic Registry Operations Tests
    // =========================================================================

    #[test]
    fn test_registry_starts_empty() {
        let registry = ModelRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_add_and_get_model() {
        let mut registry = ModelRegistry::new();
        let model = create_test_model("test-model");

        registry.add(model.clone());

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        let retrieved = registry.get("test-model");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-model");
    }

    #[test]
    fn test_contains() {
        let mut registry = ModelRegistry::new();
        registry.add(create_test_model("exists"));

        assert!(registry.contains("exists"));
        assert!(!registry.contains("does-not-exist"));
    }

    #[test]
    fn test_names_iterator() {
        let mut registry = ModelRegistry::new();
        registry.add(create_test_model("alpha"));
        registry.add(create_test_model("beta"));

        let names: Vec<&str> = registry.names().collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn test_list_returns_sorted() {
        let mut registry = ModelRegistry::new();
        registry.add(create_test_model("zebra"));
        registry.add(create_test_model("alpha"));
        registry.add(create_test_model("monkey"));

        let list = registry.list();
        assert_eq!(list, vec!["alpha", "monkey", "zebra"]);
    }

    #[test]
    fn test_all_iterator() {
        let mut registry = ModelRegistry::new();
        registry.add(create_test_model("one"));
        registry.add(create_test_model("two"));

        let configs: Vec<_> = registry.all().collect();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_add_overwrites_existing() {
        let mut registry = ModelRegistry::new();

        let model1 = ModelConfig {
            name: "same-name".to_string(),
            description: Some("first".to_string()),
            ..Default::default()
        };
        let model2 = ModelConfig {
            name: "same-name".to_string(),
            description: Some("second".to_string()),
            ..Default::default()
        };

        registry.add(model1);
        registry.add(model2);

        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.get("same-name").unwrap().description,
            Some("second".to_string())
        );
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let registry = ModelRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    // =========================================================================
    // Database Operations Tests
    // =========================================================================

    #[test]
    fn test_load_from_db_empty() {
        let (_temp, db) = setup_test_db();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_add_model_to_db_and_load() {
        let (_temp, db) = setup_test_db();
        let model = create_test_model("db-model");

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("db-model"));

        let loaded = registry.get("db-model").unwrap();
        assert_eq!(loaded.model_id, Some("db-model-id".to_string()));
        assert_eq!(loaded.context_length, 128_000);
    }

    #[test]
    fn test_add_model_to_db_with_source() {
        let (_temp, db) = setup_test_db();
        let model = create_test_model("catalog-model");

        ModelRegistry::add_model_to_db_with_source(&db, &model, "catalog").unwrap();

        // Verify source was saved
        let source: String = db
            .conn()
            .query_row(
                "SELECT source FROM models WHERE name = ?",
                params!["catalog-model"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "catalog");
    }

    #[test]
    fn test_add_model_infers_oauth_source() {
        let (_temp, db) = setup_test_db();
        let model = ModelConfig {
            name: "claude-code-model".to_string(),
            model_type: ModelType::ClaudeCode,
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let source: String = db
            .conn()
            .query_row(
                "SELECT source FROM models WHERE name = ?",
                params!["claude-code-model"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "oauth");
    }

    #[test]
    fn test_add_model_infers_custom_source() {
        let (_temp, db) = setup_test_db();
        let model = create_custom_model("custom-model", "https://api.example.com", Some("key"));

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let source: String = db
            .conn()
            .query_row(
                "SELECT source FROM models WHERE name = ?",
                params!["custom-model"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "custom");
    }

    #[test]
    fn test_add_model_to_db_upserts() {
        let (_temp, db) = setup_test_db();

        let model1 = ModelConfig {
            name: "upsert-test".to_string(),
            description: Some("first".to_string()),
            ..Default::default()
        };
        let model2 = ModelConfig {
            name: "upsert-test".to_string(),
            description: Some("second".to_string()),
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model1).unwrap();
        ModelRegistry::add_model_to_db(&db, &model2).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.get("upsert-test").unwrap().description,
            Some("second".to_string())
        );
    }

    #[test]
    fn test_remove_model_from_db() {
        let (_temp, db) = setup_test_db();
        let model = create_test_model("to-remove");

        ModelRegistry::add_model_to_db(&db, &model).unwrap();
        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert!(registry.contains("to-remove"));

        ModelRegistry::remove_model_from_db(&db, "to-remove").unwrap();
        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert!(!registry.contains("to-remove"));
    }

    #[test]
    fn test_remove_nonexistent_model_succeeds() {
        let (_temp, db) = setup_test_db();
        // Should not error
        ModelRegistry::remove_model_from_db(&db, "nonexistent").unwrap();
    }

    #[test]
    fn test_reload_from_db() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();

        // Add model directly to registry (not db)
        registry.add(create_test_model("in-memory-only"));
        assert!(registry.contains("in-memory-only"));

        // Add model to db
        ModelRegistry::add_model_to_db(&db, &create_test_model("in-db")).unwrap();

        // Reload - should clear in-memory and load from db
        registry.reload_from_db(&db).unwrap();

        assert!(!registry.contains("in-memory-only"));
        assert!(registry.contains("in-db"));
    }

    #[test]
    fn test_load_multiple_models_from_db() {
        let (_temp, db) = setup_test_db();

        for i in 0..5 {
            let model = create_test_model(&format!("model-{}", i));
            ModelRegistry::add_model_to_db(&db, &model).unwrap();
        }

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert_eq!(registry.len(), 5);
    }

    #[test]
    fn test_load_model_with_custom_endpoint() {
        let (_temp, db) = setup_test_db();
        let model = create_custom_model("custom-ep", "https://api.test.com/v1", Some("secret"));

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("custom-ep").unwrap();

        assert!(loaded.custom_endpoint.is_some());
        let endpoint = loaded.custom_endpoint.as_ref().unwrap();
        assert_eq!(endpoint.url, "https://api.test.com/v1");
        assert_eq!(endpoint.api_key, Some("secret".to_string()));
    }

    #[test]
    fn test_load_model_with_azure_config() {
        let (_temp, db) = setup_test_db();
        let model = ModelConfig {
            name: "azure-model".to_string(),
            model_type: ModelType::AzureOpenai,
            azure_deployment: Some("my-deployment".to_string()),
            azure_api_version: Some("2024-01-01".to_string()),
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("azure-model").unwrap();

        assert_eq!(loaded.azure_deployment, Some("my-deployment".to_string()));
        assert_eq!(loaded.azure_api_version, Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_load_model_preserves_capabilities() {
        let (_temp, db) = setup_test_db();
        let model = ModelConfig {
            name: "capable-model".to_string(),
            supports_thinking: true,
            supports_vision: true,
            supports_tools: false,
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("capable-model").unwrap();

        assert!(loaded.supports_thinking);
        assert!(loaded.supports_vision);
        assert!(!loaded.supports_tools);
    }

    // =========================================================================
    // File Operations Tests
    // =========================================================================

    #[test]
    fn test_load_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("models.json");

        let json = r#"[
            {
                "name": "file-model-1",
                "model_type": "openai",
                "context_length": 4096
            },
            {
                "name": "file-model-2",
                "model_type": "anthropic",
                "context_length": 200000
            }
        ]"#;
        std::fs::write(&file_path, json).unwrap();

        let mut registry = ModelRegistry::new();
        registry.load_file(&file_path).unwrap();

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("file-model-1"));
        assert!(registry.contains("file-model-2"));
    }

    #[test]
    fn test_load_file_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.json");
        std::fs::write(&file_path, "not valid json").unwrap();

        let mut registry = ModelRegistry::new();
        let result = registry.load_file(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_file_not_found() {
        let mut registry = ModelRegistry::new();
        let result = registry.load_file(&PathBuf::from("/nonexistent/path/models.json"));
        assert!(result.is_err());
    }

    // =========================================================================
    // Config Directory Tests
    // =========================================================================

    #[test]
    fn test_config_dir_returns_stockpot_path() {
        let result = ModelRegistry::config_dir();
        // Should succeed if home dir exists
        if let Ok(path) = result {
            assert!(path.ends_with(".stockpot"));
        }
    }

    // =========================================================================
    // Provider Availability Tests
    // =========================================================================

    #[test]
    fn test_list_available_empty_registry() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();

        let available = registry.list_available(&db);
        assert!(available.is_empty());
    }

    #[test]
    fn test_list_available_with_api_key() {
        let (_temp, db) = setup_test_db();

        // Save API key to db
        db.save_api_key("OPENAI_API_KEY", "test-key").unwrap();

        // Add OpenAI model
        let model = create_test_model("gpt-test");
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"gpt-test".to_string()));
    }

    #[test]
    fn test_list_available_without_api_key() {
        let (_temp, db) = setup_test_db();

        // Add OpenAI model but no API key
        let model = create_test_model("gpt-no-key");
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(!available.contains(&"gpt-no-key".to_string()));
    }

    #[test]
    fn test_list_available_anthropic() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("ANTHROPIC_API_KEY", "test-key").unwrap();

        let model = ModelConfig {
            name: "claude-test".to_string(),
            model_type: ModelType::Anthropic,
            ..Default::default()
        };
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"claude-test".to_string()));
    }

    #[test]
    fn test_list_available_gemini_with_google_key() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("GOOGLE_API_KEY", "test-key").unwrap();

        let model = ModelConfig {
            name: "gemini-test".to_string(),
            model_type: ModelType::Gemini,
            ..Default::default()
        };
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"gemini-test".to_string()));
    }

    #[test]
    fn test_list_available_custom_with_literal_key() {
        let (_temp, db) = setup_test_db();

        let model = create_custom_model(
            "custom-literal",
            "https://api.test.com",
            Some("literal-key"),
        );
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"custom-literal".to_string()));
    }

    #[test]
    fn test_list_available_custom_with_env_var_key() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("CUSTOM_API_KEY", "test-key").unwrap();

        let model = create_custom_model(
            "custom-env",
            "https://api.test.com",
            Some("$CUSTOM_API_KEY"),
        );
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"custom-env".to_string()));
    }

    #[test]
    fn test_list_available_custom_with_env_var_braces() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("BRACED_KEY", "test-key").unwrap();

        let model = create_custom_model(
            "custom-braced",
            "https://api.test.com",
            Some("${BRACED_KEY}"),
        );
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"custom-braced".to_string()));
    }

    #[test]
    fn test_list_available_custom_no_key() {
        let (_temp, db) = setup_test_db();

        let model = create_custom_model("custom-no-key", "https://api.test.com", None);
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(!available.contains(&"custom-no-key".to_string()));
    }

    #[test]
    fn test_list_available_round_robin_always_available() {
        let (_temp, db) = setup_test_db();

        let model = ModelConfig {
            name: "round-robin-test".to_string(),
            model_type: ModelType::RoundRobin,
            round_robin_models: vec!["model1".to_string(), "model2".to_string()],
            ..Default::default()
        };
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"round-robin-test".to_string()));
    }

    #[test]
    fn test_list_available_azure() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("AZURE_OPENAI_API_KEY", "test-key").unwrap();

        let model = ModelConfig {
            name: "azure-test".to_string(),
            model_type: ModelType::AzureOpenai,
            ..Default::default()
        };
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"azure-test".to_string()));
    }

    #[test]
    fn test_list_available_openrouter() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("OPENROUTER_API_KEY", "test-key").unwrap();

        let model = ModelConfig {
            name: "openrouter-test".to_string(),
            model_type: ModelType::Openrouter,
            ..Default::default()
        };
        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"openrouter-test".to_string()));
    }

    #[test]
    fn test_list_available_mixed_providers() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("OPENAI_API_KEY", "key1").unwrap();
        // No Anthropic key

        let openai_model = create_test_model("openai-model");
        let anthropic_model = ModelConfig {
            name: "anthropic-model".to_string(),
            model_type: ModelType::Anthropic,
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &openai_model).unwrap();
        ModelRegistry::add_model_to_db(&db, &anthropic_model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert!(available.contains(&"openai-model".to_string()));
        assert!(!available.contains(&"anthropic-model".to_string()));
    }

    #[test]
    fn test_list_available_sorted() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("OPENAI_API_KEY", "key").unwrap();

        for name in ["zebra", "alpha", "monkey"] {
            let model = create_test_model(name);
            ModelRegistry::add_model_to_db(&db, &model).unwrap();
        }

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let available = registry.list_available(&db);

        assert_eq!(available, vec!["alpha", "monkey", "zebra"]);
    }


    // =========================================================================
    // Edge Cases Tests
    // =========================================================================

    #[test]
    fn test_model_with_special_characters_in_name() {
        let (_temp, db) = setup_test_db();
        let model = create_test_model("model-with_special.chars:v1");

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        assert!(registry.contains("model-with_special.chars:v1"));
    }

    #[test]
    fn test_model_with_unicode_description() {
        let (_temp, db) = setup_test_db();
        let model = ModelConfig {
            name: "unicode-model".to_string(),
            description: Some("模型描述 🤖 émoji".to_string()),
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("unicode-model").unwrap();
        assert_eq!(loaded.description, Some("模型描述 🤖 émoji".to_string()));
    }

    #[test]
    fn test_model_with_empty_optional_fields() {
        let (_temp, db) = setup_test_db();
        let model = ModelConfig {
            name: "minimal-model".to_string(),
            model_type: ModelType::Openai,
            model_id: None,
            context_length: 1000,
            supports_thinking: false,
            supports_vision: false,
            supports_tools: false,
            description: None,
            custom_endpoint: None,
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("minimal-model").unwrap();

        assert_eq!(loaded.model_id, None);
        assert_eq!(loaded.description, None);
        assert!(loaded.custom_endpoint.is_none());
    }

    #[test]
    fn test_model_with_headers() {
        let (_temp, db) = setup_test_db();
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "value".to_string());
        headers.insert("Authorization".to_string(), "Bearer xyz".to_string());

        let model = ModelConfig {
            name: "headers-model".to_string(),
            model_type: ModelType::CustomOpenai,
            custom_endpoint: Some(CustomEndpoint {
                url: "https://api.test.com".to_string(),
                api_key: Some("key".to_string()),
                headers,
                ca_certs_path: None,
            }),
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("headers-model").unwrap();
        let endpoint = loaded.custom_endpoint.as_ref().unwrap();

        assert_eq!(
            endpoint.headers.get("X-Custom-Header"),
            Some(&"value".to_string())
        );
        assert_eq!(
            endpoint.headers.get("Authorization"),
            Some(&"Bearer xyz".to_string())
        );
    }

    #[test]
    fn test_large_context_length() {
        let (_temp, db) = setup_test_db();
        let model = ModelConfig {
            name: "large-context".to_string(),
            context_length: 2_000_000, // 2M tokens
            ..Default::default()
        };

        ModelRegistry::add_model_to_db(&db, &model).unwrap();

        let registry = ModelRegistry::load_from_db(&db).unwrap();
        let loaded = registry.get("large-context").unwrap();
        assert_eq!(loaded.context_length, 2_000_000);
    }
}
