//! Core model type definitions.
//!
//! This module provides the fundamental types for model configuration:
//! - `ModelConfigError` - Errors that can occur during configuration
//! - `ModelType` - Supported AI provider types
//! - `CustomEndpoint` - Custom API endpoint configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Google Vertex AI (OAuth)
    GoogleVertex,
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
            ModelType::GoogleVertex => write!(f, "google_vertex"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_display() {
        assert_eq!(ModelType::Openai.to_string(), "openai");
        assert_eq!(ModelType::Anthropic.to_string(), "anthropic");
        assert_eq!(ModelType::ClaudeCode.to_string(), "claude_code");
    }
}
