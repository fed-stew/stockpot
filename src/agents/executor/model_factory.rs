//! Model resolution and creation.
//!
//! Provides `get_model()` which resolves model specifications to
//! concrete model instances using the registry and available providers.

use std::sync::Arc;
use tracing::{debug, error, info, warn};

use serdes_ai_models::{infer_model, openai::OpenAIChatModel, Model};

use crate::auth;
use crate::db::Database;
use crate::models::{resolve_api_key, ModelRegistry, ModelType};

use super::ExecutorError;

/// Get a model by name, handling custom endpoints, OAuth models, and standard models.
///
/// This function checks the model registry first for custom configurations,
/// then falls back to OAuth models (by prefix), and finally to standard
/// API key-based models.
///
/// # Model Resolution Order
/// 1. Custom endpoint models (from `/add-model`)
/// 2. OAuth models by config type (ClaudeCode, ChatgptOauth)
/// 3. OAuth models by prefix (legacy: `chatgpt-*`, `claude-code-*`)
/// 4. Standard models via `infer_model()` (uses environment API keys)
pub async fn get_model(
    db: &Database,
    model_name: &str,
    registry: &ModelRegistry,
) -> Result<Arc<dyn Model>, ExecutorError> {
    debug!(model_name = %model_name, "get_model called");

    // First, check if we have a custom config for this model in the registry
    if let Some(config) = registry.get(model_name) {
        debug!(
            model_name = %model_name,
            model_type = %config.model_type,
            has_custom_endpoint = config.custom_endpoint.is_some(),
            "Found model in registry"
        );

        // Handle custom endpoint models (e.g., from /add-model)
        if let Some(endpoint) = &config.custom_endpoint {
            debug!(
                endpoint_url = %endpoint.url,
                has_api_key = endpoint.api_key.is_some(),
                "Custom endpoint details"
            );
            debug!("Using custom endpoint for model: {}", model_name);

            // Resolve the API key from database or environment
            let api_key = if let Some(ref key_template) = endpoint.api_key {
                if key_template.starts_with('$') {
                    // It's an env var reference like $API_KEY or ${API_KEY}
                    let var_name = key_template
                        .trim_start_matches('$')
                        .trim_matches(|c| c == '{' || c == '}');
                    // Check database first, then environment
                    resolve_api_key(db, var_name).ok_or_else(|| {
                        ExecutorError::Config(format!(
                            "API key {} not found. Run /add_model to configure it, or set the environment variable.",
                            var_name
                        ))
                    })?
                } else {
                    // It's a literal key
                    key_template.clone()
                }
            } else {
                return Err(ExecutorError::Config(format!(
                    "Model {} has custom endpoint but no API key configured",
                    model_name
                )));
            };

            // Get the actual model ID to send to the API
            let model_id = config.model_id.as_deref().unwrap_or(model_name);

            // Create OpenAI-compatible model with custom endpoint
            let model = OpenAIChatModel::new(model_id, api_key).with_base_url(&endpoint.url);

            info!(
                model_name = %model_name,
                endpoint = %endpoint.url,
                "Custom endpoint model ready"
            );
            return Ok(Arc::new(model));
        }

        // Handle based on model type for non-custom-endpoint models
        match config.model_type {
            ModelType::ClaudeCode => {
                debug!("Detected Claude Code OAuth model from config");
                let model = auth::get_claude_code_model(db, model_name)
                    .await
                    .map_err(|e| ExecutorError::Auth(e.to_string()))?;
                return Ok(Arc::new(model));
            }
            ModelType::ChatgptOauth => {
                debug!("Detected ChatGPT OAuth model from config");
                let model = auth::get_chatgpt_model(db, model_name)
                    .await
                    .map_err(|e| ExecutorError::Auth(e.to_string()))?;
                return Ok(Arc::new(model));
            }
            // For other types, fall through to standard handling
            _ => {}
        }
    }

    // Legacy: Check for OAuth models by prefix (backward compatibility)
    if model_name.starts_with("chatgpt-") || model_name.starts_with("chatgpt_") {
        debug!("Detected ChatGPT OAuth model by prefix");
        let model = auth::get_chatgpt_model(db, model_name).await.map_err(|e| {
            error!(error = %e, "Failed to get ChatGPT model");
            ExecutorError::Auth(e.to_string())
        })?;
        info!(model_id = %model.identifier(), "ChatGPT OAuth model ready");
        return Ok(Arc::new(model));
    }

    if model_name.starts_with("claude-code-") || model_name.starts_with("claude_code_") {
        debug!("Detected Claude Code OAuth model by prefix");
        let model = auth::get_claude_code_model(db, model_name)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to get Claude Code model");
                ExecutorError::Auth(e.to_string())
            })?;
        info!(model_id = %model.identifier(), "Claude Code OAuth model ready");
        return Ok(Arc::new(model));
    }

    // Check if this looks like a custom model (provider:model format)
    // If so, it should have been in the registry - error out
    if model_name.contains(':') && !model_name.starts_with("claude-code") {
        warn!(
            model_name = %model_name,
            registry_count = registry.len(),
            "Custom model not found in registry"
        );
        return Err(ExecutorError::Config(format!(
            "Model '{}' not found in registry. Did you add it with /add-model? Try running /add-model again.",
            model_name
        )));
    }

    // Standard model inference (uses API keys from environment)
    debug!("Using API key model inference for: {}", model_name);
    let model = infer_model(model_name).map_err(|e| {
        error!(error = %e, "Failed to infer model");
        ExecutorError::Model(e.to_string())
    })?;

    info!(model_name = %model_name, "Model ready");
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CustomEndpoint, ModelConfig, ModelType};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, db)
    }

    fn create_custom_model(name: &str, url: &str, api_key: Option<&str>) -> ModelConfig {
        ModelConfig {
            name: name.to_string(),
            model_type: ModelType::CustomOpenai,
            model_id: Some("test-model-id".to_string()),
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

    #[tokio::test]
    async fn test_custom_endpoint_with_literal_api_key() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = create_custom_model(
            "custom-literal",
            "https://api.example.com/v1",
            Some("sk-literal-key-12345"),
        );
        registry.add(model);
        let result = get_model(&db, "custom-literal", &registry).await;
        assert!(result.is_ok(), "Should succeed with literal API key");
        let model = result.unwrap();
        assert!(model.identifier().contains("test-model-id"));
    }

    #[tokio::test]
    async fn test_custom_endpoint_with_env_var_key() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let env_key = "TEST_CUSTOM_API_KEY_FOR_MODEL_FACTORY";
        std::env::set_var(env_key, "sk-from-env-var");
        let model = create_custom_model(
            "custom-env",
            "https://api.example.com/v1",
            Some(&format!("${}", env_key)),
        );
        registry.add(model);
        let result = get_model(&db, "custom-env", &registry).await;
        std::env::remove_var(env_key);
        assert!(result.is_ok(), "Should succeed with env var API key");
    }

    #[tokio::test]
    async fn test_custom_endpoint_with_env_var_from_db() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let key_name = "DB_STORED_API_KEY";
        db.save_api_key(key_name, "sk-from-database").unwrap();
        let model = create_custom_model(
            "custom-db-key",
            "https://api.example.com/v1",
            Some(&format!("${}", key_name)),
        );
        registry.add(model);
        let result = get_model(&db, "custom-db-key", &registry).await;
        assert!(result.is_ok(), "Should resolve API key from database");
    }

    #[tokio::test]
    async fn test_custom_endpoint_missing_key_returns_error() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = create_custom_model(
            "custom-no-key",
            "https://api.example.com/v1",
            Some("$NONEXISTENT_API_KEY_XYZ_123"),
        );
        registry.add(model);
        let result = get_model(&db, "custom-no-key", &registry).await;
        assert!(result.is_err(), "Should error when env var not found");
        if let Err(ExecutorError::Config(msg)) = result {
            assert!(
                msg.contains("not found"),
                "Error should mention key not found"
            );
        } else {
            panic!("Expected ExecutorError::Config");
        }
    }

    #[tokio::test]
    async fn test_custom_endpoint_no_api_key_configured() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = create_custom_model("custom-missing", "https://api.example.com/v1", None);
        registry.add(model);
        let result = get_model(&db, "custom-missing", &registry).await;
        assert!(result.is_err());
        if let Err(ExecutorError::Config(msg)) = result {
            assert!(msg.contains("no API key configured"));
        } else {
            panic!("Expected ExecutorError::Config for missing API key");
        }
    }

    #[tokio::test]
    async fn test_legacy_chatgpt_prefix_detected() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let result = get_model(&db, "chatgpt-4o-latest", &registry).await;
        assert!(result.is_err());
        assert!(
            matches!(result, Err(ExecutorError::Auth(_))),
            "Expected Auth error for chatgpt- prefix"
        );
    }

    #[tokio::test]
    async fn test_legacy_chatgpt_underscore_prefix_detected() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let result = get_model(&db, "chatgpt_4o_latest", &registry).await;
        assert!(result.is_err());
        assert!(
            matches!(result, Err(ExecutorError::Auth(_))),
            "Expected Auth error for chatgpt_ prefix"
        );
    }

    #[tokio::test]
    async fn test_legacy_claude_code_prefix_detected() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let result = get_model(&db, "claude-code-sonnet", &registry).await;
        assert!(result.is_err());
        assert!(
            matches!(result, Err(ExecutorError::Auth(_))),
            "Expected Auth error for claude-code- prefix"
        );
    }

    #[tokio::test]
    async fn test_legacy_claude_code_underscore_prefix_detected() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let result = get_model(&db, "claude_code_opus", &registry).await;
        assert!(result.is_err());
        assert!(
            matches!(result, Err(ExecutorError::Auth(_))),
            "Expected Auth error for claude_code_ prefix"
        );
    }

    #[tokio::test]
    async fn test_custom_model_not_in_registry_returns_error() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let result = get_model(&db, "openrouter:mistral-7b", &registry).await;
        assert!(result.is_err());
        if let Err(ExecutorError::Config(msg)) = result {
            assert!(msg.contains("not found in registry"));
            assert!(msg.contains("/add-model"));
        } else {
            panic!("Expected ExecutorError::Config for custom model not in registry");
        }
    }

    #[tokio::test]
    async fn test_custom_model_with_colon_not_in_registry() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        for model_name in [
            "together:llama-3.1",
            "groq:mixtral",
            "local:phi-3",
            "custom:my-model",
        ] {
            let result = get_model(&db, model_name, &registry).await;
            assert!(
                matches!(result, Err(ExecutorError::Config(_))),
                "Model {} should fail when not in registry",
                model_name
            );
        }
    }

    #[tokio::test]
    async fn test_claude_code_colon_not_treated_as_custom() {
        let (_temp, db) = setup_test_db();
        let registry = ModelRegistry::new();
        let result = get_model(&db, "claude-code:sonnet-4", &registry).await;
        match result {
            Err(ExecutorError::Auth(_)) => {}
            Err(ExecutorError::Config(ref msg)) if msg.contains("not found in registry") => {
                panic!("claude-code: should not be treated as custom model format")
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_claude_code_model_type_triggers_oauth() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = ModelConfig {
            name: "my-claude".to_string(),
            model_type: ModelType::ClaudeCode,
            ..Default::default()
        };
        registry.add(model);
        let result = get_model(&db, "my-claude", &registry).await;
        assert!(matches!(result, Err(ExecutorError::Auth(_))));
    }

    #[tokio::test]
    async fn test_chatgpt_oauth_model_type_triggers_oauth() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = ModelConfig {
            name: "my-chatgpt".to_string(),
            model_type: ModelType::ChatgptOauth,
            ..Default::default()
        };
        registry.add(model);
        let result = get_model(&db, "my-chatgpt", &registry).await;
        assert!(matches!(result, Err(ExecutorError::Auth(_))));
    }

    #[tokio::test]
    async fn test_env_var_with_braces() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let env_key = "TEST_BRACED_KEY_MODEL_FACTORY";
        std::env::set_var(env_key, "sk-braced-key");
        let model = create_custom_model(
            "custom-braced",
            "https://api.example.com/v1",
            Some(&format!("${{{}}}", env_key)),
        );
        registry.add(model);
        let result = get_model(&db, "custom-braced", &registry).await;
        std::env::remove_var(env_key);
        assert!(result.is_ok(), "Should resolve braced env var syntax");
    }

    #[tokio::test]
    async fn test_model_id_used_when_present() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = ModelConfig {
            name: "friendly-name".to_string(),
            model_type: ModelType::CustomOpenai,
            model_id: Some("actual-api-model-id".to_string()),
            custom_endpoint: Some(CustomEndpoint {
                url: "https://api.example.com/v1".to_string(),
                api_key: Some("sk-test".to_string()),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            ..Default::default()
        };
        registry.add(model);
        let result = get_model(&db, "friendly-name", &registry).await;
        assert!(result.is_ok());
        let model = result.unwrap();
        assert!(model.identifier().contains("actual-api-model-id"));
    }

    #[tokio::test]
    async fn test_model_name_used_when_no_model_id() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = ModelConfig {
            name: "fallback-name".to_string(),
            model_type: ModelType::CustomOpenai,
            model_id: None,
            custom_endpoint: Some(CustomEndpoint {
                url: "https://api.example.com/v1".to_string(),
                api_key: Some("sk-test".to_string()),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            ..Default::default()
        };
        registry.add(model);
        let result = get_model(&db, "fallback-name", &registry).await;
        assert!(result.is_ok());
        let model = result.unwrap();
        assert!(model.identifier().contains("fallback-name"));
    }
}
