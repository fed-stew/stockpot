//! Model configuration types and registry.
//!
//! This module provides:
//! - `ModelType` enum for different AI providers
//! - `CustomEndpoint` for custom API configurations
//! - `ModelConfig` for per-model settings
//! - `ModelRegistry` for loading and managing model configs

use crate::db::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during model configuration.
#[derive(Debug, Error)]
pub enum ModelConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Config directory not found")]
    ConfigDirNotFound,
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),
}

/// Supported model provider types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    /// OpenAI API (GPT models)
    #[default]
    Openai,
    /// Anthropic API (Claude models)
    Anthropic,
    /// Google Gemini API
    Gemini,
    /// Custom endpoint with OpenAI-compatible API
    CustomOpenai,
    /// Custom endpoint with Anthropic-compatible API
    CustomAnthropic,
    /// Claude Code OAuth-authenticated
    ClaudeCode,
    /// ChatGPT OAuth-authenticated
    ChatgptOauth,
    /// Azure OpenAI Service
    AzureOpenai,
    /// OpenRouter API
    Openrouter,
    /// Round-robin load balancing across models
    RoundRobin,
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelType::Openai => write!(f, "openai"),
            ModelType::Anthropic => write!(f, "anthropic"),
            ModelType::Gemini => write!(f, "gemini"),
            ModelType::CustomOpenai => write!(f, "custom_openai"),
            ModelType::CustomAnthropic => write!(f, "custom_anthropic"),
            ModelType::ClaudeCode => write!(f, "claude_code"),
            ModelType::ChatgptOauth => write!(f, "chatgpt_oauth"),
            ModelType::AzureOpenai => write!(f, "azure_openai"),
            ModelType::Openrouter => write!(f, "openrouter"),
            ModelType::RoundRobin => write!(f, "round_robin"),
        }
    }
}

/// Custom endpoint configuration for non-standard API providers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomEndpoint {
    /// Base URL for the API endpoint
    pub url: String,
    /// Additional headers to include in requests
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// API key (can use $VAR or ${VAR} syntax for env vars)
    #[serde(default)]
    pub api_key: Option<String>,
    /// Path to CA certificates for custom SSL verification
    #[serde(default)]
    pub ca_certs_path: Option<String>,
}

/// Configuration for a specific model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Display name / identifier for the model
    pub name: String,
    /// Provider type
    #[serde(default)]
    pub model_type: ModelType,
    /// Maximum context length in tokens
    #[serde(default = "default_context_length")]
    pub context_length: usize,
    /// Custom endpoint configuration (for custom providers)
    #[serde(default)]
    pub custom_endpoint: Option<CustomEndpoint>,
    /// The actual model ID to use in API calls (if different from name)
    #[serde(default)]
    pub model_id: Option<String>,
    /// Whether this model supports extended/deep thinking
    #[serde(default)]
    pub supports_thinking: bool,
    /// Whether this model supports vision/images
    #[serde(default)]
    pub supports_vision: bool,
    /// Whether this model supports tool use/function calling
    #[serde(default = "default_true")]
    pub supports_tools: bool,
    /// Description of the model
    #[serde(default)]
    pub description: Option<String>,
    /// For Azure OpenAI: the deployment name
    #[serde(default)]
    pub azure_deployment: Option<String>,
    /// For Azure OpenAI: the API version
    #[serde(default)]
    pub azure_api_version: Option<String>,
    /// For round-robin: list of model names to cycle through
    #[serde(default)]
    pub round_robin_models: Vec<String>,
}

fn default_context_length() -> usize {
    128_000
}

fn default_true() -> bool {
    true
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            name: "gpt-4o".to_string(),
            model_type: ModelType::Openai,
            context_length: default_context_length(),
            custom_endpoint: None,
            model_id: None,
            supports_thinking: false,
            supports_vision: true,
            supports_tools: true,
            description: None,
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        }
    }
}

impl ModelConfig {
    /// Get the effective model ID for API calls.
    pub fn effective_model_id(&self) -> &str {
        self.model_id.as_deref().unwrap_or(&self.name)
    }

    /// Check if this is an OAuth-based model.
    pub fn is_oauth(&self) -> bool {
        matches!(
            self.model_type,
            ModelType::ClaudeCode | ModelType::ChatgptOauth
        )
    }

    /// Check if this requires a custom endpoint.
    pub fn requires_custom_endpoint(&self) -> bool {
        matches!(
            self.model_type,
            ModelType::CustomOpenai | ModelType::CustomAnthropic
        )
    }
}

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

    /// Load models from all standard config locations (legacy JSON files).
    /// Prefer `load_from_db()` for new code.
    pub fn load() -> Result<Self, ModelConfigError> {
        let config_dir = Self::config_dir()?;
        let mut registry = Self::new();

        // Create config dir if needed
        std::fs::create_dir_all(&config_dir)?;

        // Create default models.json if it doesn't exist
        let models_path = config_dir.join("models.json");
        if !models_path.exists() {
            let defaults = crate::models::defaults::default_models_json();
            std::fs::write(&models_path, defaults)?;
        }

        // Load built-in models
        if models_path.exists() {
            registry.load_file(&models_path)?;
        }

        // Load ChatGPT OAuth models
        let chatgpt_path = config_dir.join("chatgpt_models.json");
        if chatgpt_path.exists() {
            registry.load_file(&chatgpt_path)?;
        }

        // Load Claude Code OAuth models
        let claude_path = config_dir.join("claude_models.json");
        if claude_path.exists() {
            registry.load_file(&claude_path)?;
        }

        // Fallback: add in-memory defaults if still empty
        if registry.is_empty() {
            registry.add_builtin_defaults();
        }

        Ok(registry)
    }

    /// Load models from the database.
    /// Also adds built-in defaults if they don't exist.
    pub fn load_from_db(db: &Database) -> Result<Self, ModelConfigError> {
        let mut registry = Self::new();

        // First, ensure built-in models exist in DB
        registry.ensure_builtin_models(db)?;

        // Load all models from database
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT name, model_type, model_id, context_length, supports_thinking,
                        supports_vision, supports_tools, description, api_endpoint,
                        api_key_env, headers, azure_deployment, azure_api_version
                 FROM models ORDER BY name",
            )
            .map_err(|e| {
                ModelConfigError::Io(std::io::Error::other(e.to_string()))
            })?;

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
            .map_err(|e| {
                ModelConfigError::Io(std::io::Error::other(e.to_string()))
            })?;

        for config in rows.flatten() {
            registry.models.insert(config.name.clone(), config);
        }

        // Also load legacy JSON files (for OAuth models, etc.)
        registry.load_legacy_json_files()?;

        Ok(registry)
    }

    /// Load legacy JSON files (OAuth models, etc.).
    fn load_legacy_json_files(&mut self) -> Result<(), ModelConfigError> {
        let config_dir = Self::config_dir()?;

        // Load ChatGPT OAuth models
        let chatgpt_path = config_dir.join("chatgpt_models.json");
        if chatgpt_path.exists() {
            self.load_file(&chatgpt_path)?;
        }

        // Load Claude Code OAuth models
        let claude_path = config_dir.join("claude_models.json");
        if claude_path.exists() {
            self.load_file(&claude_path)?;
        }

        Ok(())
    }

    /// Ensure built-in default models exist in the database.
    fn ensure_builtin_models(&self, db: &Database) -> Result<(), ModelConfigError> {
        let defaults = crate::models::defaults::default_models();

        for model in defaults {
            // Insert if not exists (don't overwrite user customizations)
            db.conn()
                .execute(
                    "INSERT OR IGNORE INTO models (name, model_type, model_id, context_length,
                        supports_thinking, supports_vision, supports_tools, description, is_builtin)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)",
                    params![
                        &model.name,
                        model.model_type.to_string(),
                        &model.model_id,
                        model.context_length as i64,
                        model.supports_thinking as i64,
                        model.supports_vision as i64,
                        model.supports_tools as i64,
                        &model.description,
                    ],
                )
                .map_err(|e| {
                    ModelConfigError::Io(std::io::Error::other(e.to_string()))
                })?;
        }

        Ok(())
    }

    /// Add a custom model to the database.
    pub fn add_model_to_db(db: &Database, config: &ModelConfig) -> Result<(), ModelConfigError> {
        let headers_json = config
            .custom_endpoint
            .as_ref()
            .map(|e| serde_json::to_string(&e.headers).unwrap_or_default());

        db.conn()
            .execute(
                "INSERT OR REPLACE INTO models (name, model_type, model_id, context_length,
                    supports_thinking, supports_vision, supports_tools, description,
                    api_endpoint, api_key_env, headers, is_builtin, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, unixepoch())",
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
                    config.custom_endpoint.as_ref().and_then(|e| e.api_key.as_ref()),
                    headers_json,
                ],
            )
            .map_err(|e| {
                ModelConfigError::Io(std::io::Error::other(e.to_string()))
            })?;

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

    /// Reload the registry from config files (legacy).
    /// Call this after OAuth auth to pick up new models.
    /// For database-backed registries, use `reload_from_db()` instead.
    pub fn reload(&mut self) -> Result<(), ModelConfigError> {
        self.models.clear();

        let config_dir = Self::config_dir()?;

        // Load built-in models
        let models_path = config_dir.join("models.json");
        if models_path.exists() {
            self.load_file(&models_path)?;
        }

        // Load ChatGPT OAuth models
        let chatgpt_path = config_dir.join("chatgpt_models.json");
        if chatgpt_path.exists() {
            self.load_file(&chatgpt_path)?;
        }

        // Load Claude Code OAuth models
        let claude_path = config_dir.join("claude_models.json");
        if claude_path.exists() {
            self.load_file(&claude_path)?;
        }

        Ok(())
    }

    /// Get the config directory path.
    pub fn config_dir() -> Result<PathBuf, ModelConfigError> {
        let home = dirs::home_dir().ok_or(ModelConfigError::ConfigDirNotFound)?;
        Ok(home.join(".stockpot"))
    }

    /// Add default built-in models.
    fn add_builtin_defaults(&mut self) {
        // OpenAI models
        self.add(ModelConfig {
            name: "gpt-4o".to_string(),
            model_type: ModelType::Openai,
            context_length: 128_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("GPT-4o - OpenAI's flagship model".to_string()),
            ..Default::default()
        });

        self.add(ModelConfig {
            name: "gpt-4o-mini".to_string(),
            model_type: ModelType::Openai,
            context_length: 128_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("GPT-4o Mini - Fast and affordable".to_string()),
            ..Default::default()
        });

        self.add(ModelConfig {
            name: "o1".to_string(),
            model_type: ModelType::Openai,
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("O1 - OpenAI's reasoning model".to_string()),
            ..Default::default()
        });

        self.add(ModelConfig {
            name: "o1-mini".to_string(),
            model_type: ModelType::Openai,
            context_length: 128_000,
            supports_thinking: true,
            supports_vision: false,
            supports_tools: true,
            description: Some("O1 Mini - Efficient reasoning".to_string()),
            ..Default::default()
        });

        // Anthropic models
        self.add(ModelConfig {
            name: "claude-sonnet-4-20250514".to_string(),
            model_type: ModelType::Anthropic,
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Claude Sonnet 4 - Balanced capability".to_string()),
            ..Default::default()
        });

        self.add(ModelConfig {
            name: "claude-opus-4-20250514".to_string(),
            model_type: ModelType::Anthropic,
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Claude Opus 4 - Most capable".to_string()),
            ..Default::default()
        });

        // Gemini models
        self.add(ModelConfig {
            name: "gemini-2.0-flash".to_string(),
            model_type: ModelType::Gemini,
            context_length: 1_000_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("Gemini 2.0 Flash - Fast and capable".to_string()),
            ..Default::default()
        });

        self.add(ModelConfig {
            name: "gemini-2.5-pro".to_string(),
            model_type: ModelType::Gemini,
            context_length: 1_000_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Gemini 2.5 Pro - Most capable".to_string()),
            ..Default::default()
        });
    }

    /// List only models that have valid provider configuration.
    /// Checks for API keys in database or environment, or OAuth tokens.
    pub fn list_available(&self, db: &Database) -> Vec<String> {
        let mut available: Vec<String> = self
            .models
            .iter()
            .filter(|(name, config)| self.is_provider_available(db, name, config))
            .map(|(name, _)| name.clone())
            .collect();
        available.sort();
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
                // Claude Code OAuth models are only in registry if auth succeeded
                true
            }
            ModelType::ChatgptOauth => {
                // Same - if loaded, auth worked
                true
            }
            ModelType::AzureOpenai => {
                has_api_key(db, "AZURE_OPENAI_API_KEY")
                    || has_api_key(db, "AZURE_OPENAI_ENDPOINT")
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

/// Parse a model type string from the database.
fn parse_model_type(s: &str) -> ModelType {
    match s {
        "openai" => ModelType::Openai,
        "anthropic" => ModelType::Anthropic,
        "gemini" => ModelType::Gemini,
        "custom_openai" => ModelType::CustomOpenai,
        "custom_anthropic" => ModelType::CustomAnthropic,
        "claude_code" => ModelType::ClaudeCode,
        "chatgpt_oauth" => ModelType::ChatgptOauth,
        "azure_openai" => ModelType::AzureOpenai,
        "openrouter" => ModelType::Openrouter,
        "round_robin" => ModelType::RoundRobin,
        _ => ModelType::CustomOpenai,
    }
}

/// Build a CustomEndpoint from database fields.
fn build_custom_endpoint(
    url: Option<String>,
    api_key: Option<String>,
    headers_json: Option<String>,
) -> Option<CustomEndpoint> {
    let url = url?;
    Some(CustomEndpoint {
        url,
        api_key,
        headers: headers_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
        ca_certs_path: None,
    })
}

/// Check if an API key is available (in database or environment).
pub fn has_api_key(db: &Database, key_name: &str) -> bool {
    db.has_api_key(key_name) || std::env::var(key_name).is_ok()
}

/// Resolve an API key, checking database first, then environment.
/// Returns None if the key is not found in either location.
pub fn resolve_api_key(db: &Database, key_name: &str) -> Option<String> {
    // First check database
    if let Ok(Some(key)) = db.get_api_key(key_name) {
        return Some(key);
    }
    // Fall back to environment variable
    std::env::var(key_name).ok()
}

/// Resolve environment variable references in a string.
///
/// Supports both `$VAR` and `${VAR}` syntax.
///
/// # Examples
/// ```ignore
/// let resolved = resolve_env_var("Bearer $API_KEY").unwrap();
/// let resolved = resolve_env_var("${HOME}/config").unwrap();
/// ```
pub fn resolve_env_var(input: &str) -> Result<String, ModelConfigError> {
    // Use shellexpand which handles both $VAR and ${VAR}
    shellexpand::full(input)
        .map(|s| s.into_owned())
        .map_err(|e| ModelConfigError::EnvVarNotFound(e.var_name))
}

/// Resolve all environment variables in a CustomEndpoint.
pub fn resolve_endpoint_env_vars(
    endpoint: &CustomEndpoint,
) -> Result<CustomEndpoint, ModelConfigError> {
    let mut resolved = endpoint.clone();

    resolved.url = resolve_env_var(&endpoint.url)?;

    if let Some(ref api_key) = endpoint.api_key {
        resolved.api_key = Some(resolve_env_var(api_key)?);
    }

    if let Some(ref ca_path) = endpoint.ca_certs_path {
        resolved.ca_certs_path = Some(resolve_env_var(ca_path)?);
    }

    let mut resolved_headers = HashMap::new();
    for (key, value) in &endpoint.headers {
        resolved_headers.insert(key.clone(), resolve_env_var(value)?);
    }
    resolved.headers = resolved_headers;

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_display() {
        assert_eq!(ModelType::Openai.to_string(), "openai");
        assert_eq!(ModelType::Anthropic.to_string(), "anthropic");
        assert_eq!(ModelType::ClaudeCode.to_string(), "claude_code");
    }

    #[test]
    fn test_model_config_defaults() {
        let config = ModelConfig::default();
        assert_eq!(config.name, "gpt-4o");
        assert_eq!(config.context_length, 128_000);
        assert!(config.supports_tools);
    }

    #[test]
    fn test_effective_model_id() {
        let mut config = ModelConfig::default();
        assert_eq!(config.effective_model_id(), "gpt-4o");

        config.model_id = Some("gpt-4-turbo-preview".to_string());
        assert_eq!(config.effective_model_id(), "gpt-4-turbo-preview");
    }

    #[test]
    fn test_is_oauth() {
        let mut config = ModelConfig::default();
        assert!(!config.is_oauth());

        config.model_type = ModelType::ClaudeCode;
        assert!(config.is_oauth());

        config.model_type = ModelType::ChatgptOauth;
        assert!(config.is_oauth());
    }

    #[test]
    fn test_resolve_env_var() {
        std::env::set_var("PUPPY_TEST_VAR", "woof");
        
        // Test ${VAR} syntax (recommended for embedding)
        let result = resolve_env_var("prefix_${PUPPY_TEST_VAR}_suffix");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "prefix_woof_suffix");
        
        // Test $VAR at end of string
        let result = resolve_env_var("bark_$PUPPY_TEST_VAR");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "bark_woof");
        
        // Test non-existent var returns error
        let result = resolve_env_var("${NONEXISTENT_PUPPY_VAR_XYZ}");
        assert!(result.is_err());
        
        std::env::remove_var("PUPPY_TEST_VAR");
    }

    #[test]
    fn test_registry_defaults() {
        let mut registry = ModelRegistry::new();
        assert!(registry.is_empty());
        
        registry.add_builtin_defaults();
        assert!(!registry.is_empty());
        assert!(registry.contains("gpt-4o"));
        assert!(registry.contains("claude-sonnet-4-20250514"));
        assert!(registry.contains("gemini-2.0-flash"));
    }

    #[test]
    fn test_model_config_serialization() {
        let config = ModelConfig {
            name: "test-model".to_string(),
            model_type: ModelType::CustomOpenai,
            context_length: 8192,
            custom_endpoint: Some(CustomEndpoint {
                url: "https://api.example.com/v1".to_string(),
                api_key: Some("$MY_API_KEY".to_string()),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: ModelConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.name, "test-model");
        assert_eq!(parsed.model_type, ModelType::CustomOpenai);
        assert!(parsed.custom_endpoint.is_some());
    }
}