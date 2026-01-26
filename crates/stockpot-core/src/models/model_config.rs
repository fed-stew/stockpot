//! Model configuration struct and implementation.
//!
//! This module provides the `ModelConfig` struct for per-model settings.

use serde::{Deserialize, Serialize};

use super::types::{CustomEndpoint, ModelType};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
