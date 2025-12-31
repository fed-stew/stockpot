//! Default model configurations.
//!
//! These are bundled models that work out-of-the-box with API keys.
//! On first run, these are written to ~/.stockpot/models.json

use crate::models::config::{ModelConfig, ModelType};

/// Get default model configurations as a vector.
pub fn default_models() -> Vec<ModelConfig> {
    vec![
        // OpenAI models
        ModelConfig {
            name: "openai:gpt-4o".to_string(),
            model_type: ModelType::Openai,
            model_id: Some("gpt-4o".to_string()),
            context_length: 128_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("GPT-4o - OpenAI's flagship multimodal model".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "openai:gpt-4o-mini".to_string(),
            model_type: ModelType::Openai,
            model_id: Some("gpt-4o-mini".to_string()),
            context_length: 128_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("GPT-4o Mini - Fast and affordable".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "openai:gpt-4-turbo".to_string(),
            model_type: ModelType::Openai,
            model_id: Some("gpt-4-turbo".to_string()),
            context_length: 128_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("GPT-4 Turbo - Fast GPT-4".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "openai:o1".to_string(),
            model_type: ModelType::Openai,
            model_id: Some("o1".to_string()),
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("O1 - OpenAI's reasoning model".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "openai:o1-mini".to_string(),
            model_type: ModelType::Openai,
            model_id: Some("o1-mini".to_string()),
            context_length: 128_000,
            supports_thinking: true,
            supports_tools: true,
            description: Some("O1 Mini - Efficient reasoning".to_string()),
            ..Default::default()
        },
        // Anthropic models
        ModelConfig {
            name: "anthropic:claude-sonnet-4".to_string(),
            model_type: ModelType::Anthropic,
            model_id: Some("claude-sonnet-4-20250514".to_string()),
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Claude Sonnet 4 - Balanced performance".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "anthropic:claude-opus-4".to_string(),
            model_type: ModelType::Anthropic,
            model_id: Some("claude-opus-4-20250514".to_string()),
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Claude Opus 4 - Most capable Claude".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "anthropic:claude-3-5-sonnet".to_string(),
            model_type: ModelType::Anthropic,
            model_id: Some("claude-3-5-sonnet-20241022".to_string()),
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Claude 3.5 Sonnet - Previous generation".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "anthropic:claude-3-5-haiku".to_string(),
            model_type: ModelType::Anthropic,
            model_id: Some("claude-3-5-haiku-20241022".to_string()),
            context_length: 200_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("Claude 3.5 Haiku - Fast and affordable".to_string()),
            ..Default::default()
        },
        // Gemini models
        ModelConfig {
            name: "gemini:gemini-2.0-flash".to_string(),
            model_type: ModelType::Gemini,
            model_id: Some("gemini-2.0-flash-exp".to_string()),
            context_length: 1_000_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("Gemini 2.0 Flash - Fast with 1M context".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "gemini:gemini-1.5-pro".to_string(),
            model_type: ModelType::Gemini,
            model_id: Some("gemini-1.5-pro".to_string()),
            context_length: 2_000_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("Gemini 1.5 Pro - 2M context window".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "gemini:gemini-2.5-pro".to_string(),
            model_type: ModelType::Gemini,
            model_id: Some("gemini-2.5-pro-preview-06-05".to_string()),
            context_length: 1_000_000,
            supports_thinking: true,
            supports_vision: true,
            supports_tools: true,
            description: Some("Gemini 2.5 Pro - Latest with thinking".to_string()),
            ..Default::default()
        },
    ]
}

/// Generate default models.json content.
pub fn default_models_json() -> String {
    serde_json::to_string_pretty(&default_models()).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_models_not_empty() {
        let models = default_models();
        assert!(!models.is_empty());
        assert!(models.len() >= 10);
    }

    #[test]
    fn test_default_models_have_names() {
        for model in default_models() {
            assert!(!model.name.is_empty());
            assert!(model.name.contains(':'), "Model name should have provider prefix: {}", model.name);
        }
    }

    #[test]
    fn test_default_models_json_valid() {
        let json = default_models_json();
        let parsed: Vec<ModelConfig> = serde_json::from_str(&json).expect("Should parse");
        assert!(!parsed.is_empty());
    }
}
