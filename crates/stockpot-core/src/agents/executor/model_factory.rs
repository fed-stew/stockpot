//! Model resolution and creation.
//!
//! Provides `get_model()` which resolves model specifications to
//! concrete model instances using the registry and available providers.

use std::sync::Arc;
use tracing::{debug, error, info, warn};

use serdes_ai_models::{infer_model, openai::OpenAIChatModel, Model};

use crate::auth;
use crate::db::Database;
use crate::models::settings::ModelSettings as SpotModelSettings;
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
    model_settings: Option<&SpotModelSettings>,
) -> Result<Arc<dyn Model>, ExecutorError> {
    debug!(model_name = %model_name, ?model_settings, "get_model called");

    // Calculate thinking budget - default to enabled for models that support it
    let thinking_budget = if registry
        .get(model_name)
        .map(|c| c.supports_thinking)
        .unwrap_or(false)
    {
        // Model supports thinking - enable by default unless explicitly disabled
        let explicitly_disabled = model_settings
            .map(|s| s.extended_thinking == Some(false))
            .unwrap_or(false);

        if explicitly_disabled {
            None
        } else {
            // Use configured budget or default to 10000
            Some(
                model_settings
                    .and_then(|s| s.budget_tokens.map(|b| b as u64))
                    .unwrap_or(10000),
            )
        }
    } else {
        // Model doesn't support thinking - only enable if explicitly configured
        model_settings
            .filter(|s| s.is_extended_thinking())
            .and_then(|s| s.budget_tokens.map(|b| b as u64))
    };

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
                let model = auth::get_claude_code_model(db, model_name, thinking_budget)
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
            ModelType::GoogleVertex => {
                debug!("Detected Google Vertex (Antigravity) OAuth model from config");
                let model = auth::get_google_model(db, model_name)
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
        let model = auth::get_claude_code_model(db, model_name, thinking_budget)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to get Claude Code model");
                ExecutorError::Auth(e.to_string())
            })?;
        info!(model_id = %model.identifier(), "Claude Code OAuth model ready");
        return Ok(Arc::new(model));
    }

    if model_name.starts_with("google-") {
        debug!("Detected Google Vertex (Antigravity) OAuth model by prefix");
        let model = auth::get_google_model(db, model_name).await.map_err(|e| {
            error!(error = %e, "Failed to get Google model");
            ExecutorError::Auth(e.to_string())
        })?;
        info!(model_id = %model.identifier(), "Google Vertex OAuth model ready");
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

/// Information about the current API key being used
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct KeyInfo {
    /// Provider name (e.g., "openai", "anthropic")
    pub provider: String,
    /// Key ID in the database (for tracking)
    pub key_id: i64,
}

/// Create a model using a specific API key (for key rotation on rate limits).
///
/// This is used when rotating API keys after a rate limit error.
/// It creates a new model instance with the specified key.
///
/// # Parameters
/// - `model_name`: The model name (used to look up config in registry)
/// - `registry`: The model registry for config lookup
/// - `api_key`: The specific API key to use
///
/// # Returns
/// The model instance or an error if the model can't be created.
#[allow(dead_code)]
pub fn create_model_with_key(
    model_name: &str,
    registry: &crate::models::ModelRegistry,
    api_key: &str,
) -> Result<Arc<dyn Model>, ExecutorError> {
    // Look up model config
    let config = registry.get(model_name).ok_or_else(|| {
        ExecutorError::Config(format!(
            "Model '{}' not found in registry for key rotation",
            model_name
        ))
    })?;

    // Get custom endpoint URL
    let endpoint = config.custom_endpoint.as_ref().ok_or_else(|| {
        ExecutorError::Config(format!(
            "Model '{}' doesn't have a custom endpoint - key rotation only works with custom endpoint models",
            model_name
        ))
    })?;

    // Get the model ID (use configured or fall back to name)
    let model_id = config.model_id.as_deref().unwrap_or(model_name);

    // Create the model with the rotated key
    let model = OpenAIChatModel::new(model_id, api_key).with_base_url(&endpoint.url);

    info!(
        model_name = %model_name,
        endpoint = %endpoint.url,
        "Created model with rotated API key"
    );

    Ok(Arc::new(model))
}

/// Get a model with optional retry handler for rate limit handling.
///
/// If the provider has multiple API keys configured, this function
/// can use the retry handler to get keys from the pool instead of
/// the configured key.
///
/// # Returns
/// - The model instance
/// - Optional key info (if using pool keys)
///
/// # Example
/// ```ignore
/// let retry_handler = RetryHandler::new(db.clone());
/// let (model, key_info) = get_model_with_retry(
///     &db,
///     "gpt-4",
///     &registry,
///     None,
///     Some(&retry_handler),
/// ).await?;
/// ```
#[allow(dead_code)]
pub async fn get_model_with_retry(
    db: &Database,
    model_name: &str,
    registry: &ModelRegistry,
    model_settings: Option<&SpotModelSettings>,
    retry_handler: Option<&super::RetryHandler>,
) -> Result<(Arc<dyn Model>, Option<KeyInfo>), ExecutorError> {
    // Check if this is a custom endpoint model with an API key
    if let Some(config) = registry.get(model_name) {
        if let Some(ref endpoint) = config.custom_endpoint {
            if let Some(ref _key_template) = endpoint.api_key {
                // Extract the provider name from the model config
                let provider = extract_provider_from_config(model_name, config);

                // Check if we should use pool keys
                if let Some(handler) = retry_handler {
                    if handler.should_use_rotation(&provider) {
                        // Load the provider's key pool
                        let _ = handler.load_provider(&provider);

                        // Try to get a key from the pool
                        if let Some((pool_key, key_id)) = handler.get_current_key(&provider) {
                            debug!(
                                model = %model_name,
                                provider = %provider,
                                key_id = key_id,
                                "Using pool key for model"
                            );

                            // Create the model with the pool key
                            let model_id = config.model_id.as_deref().unwrap_or(model_name);
                            let model = OpenAIChatModel::new(model_id, pool_key)
                                .with_base_url(&endpoint.url);

                            return Ok((Arc::new(model), Some(KeyInfo { provider, key_id })));
                        }
                    }
                }

                // Fall through to normal key resolution if:
                // - No retry handler provided
                // - Provider doesn't have pool keys
                // - Couldn't get a pool key
            }
        }
    }

    // Use standard model resolution
    let model = get_model(db, model_name, registry, model_settings).await?;
    Ok((model, None))
}

/// Convert a provider name to its env var format.
///
/// e.g., "cerebras" → "CEREBRAS_API_KEY", "openai" → "OPENAI_API_KEY"
/// If already in env var format (contains '_' and is uppercase), return as-is.
#[allow(dead_code)]
fn provider_to_env_var(provider: &str) -> String {
    // If it already looks like an env var name, return as-is
    if provider.contains('_') && provider == provider.to_uppercase() {
        return provider.to_string();
    }
    // Otherwise, convert: "cerebras" → "CEREBRAS_API_KEY"
    format!("{}_API_KEY", provider.to_uppercase())
}

/// Extract provider name from model configuration.
///
/// The provider is determined by:
/// 1. For custom endpoints: The full env var name (e.g., "CEREBRAS_API_KEY" from "$CEREBRAS_API_KEY")
///    This matches how keys are stored in the api_key_pools table via the UI.
/// 2. For known model types: The type name (e.g., "openai", "anthropic")
/// 3. Fallback: The model name prefix before ":" or the full name
#[allow(dead_code)]
fn extract_provider_from_config(model_name: &str, config: &crate::models::ModelConfig) -> String {
    // Try to extract from API key env var - return the FULL env var name
    // This ensures it matches what's stored in api_key_pools.provider_name
    if let Some(ref endpoint) = config.custom_endpoint {
        if let Some(ref key_template) = endpoint.api_key {
            if key_template.starts_with('$') {
                // Return the full env var name (without $ prefix) to match how
                // keys are stored via the "Manage API Keys" UI dialog
                let var_name = key_template
                    .trim_start_matches('$')
                    .trim_matches(|c| c == '{' || c == '}');
                return var_name.to_string();
            }
        }
    }

    // Use model type - always return env var format for consistency
    match config.model_type {
        ModelType::ClaudeCode => provider_to_env_var("anthropic"),
        ModelType::ChatgptOauth => provider_to_env_var("openai"),
        ModelType::GoogleVertex => provider_to_env_var("google"),
        ModelType::CustomOpenai => {
            // Try to extract from model name (e.g., "cerebras:model" -> "CEREBRAS_API_KEY")
            if let Some(pos) = model_name.find(':') {
                return provider_to_env_var(&model_name[..pos]);
            }
            provider_to_env_var("custom")
        }
        _ => provider_to_env_var("unknown"),
    }
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
        let result = get_model(&db, "custom-literal", &registry, None).await;
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
        let result = get_model(&db, "custom-env", &registry, None).await;
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
        let result = get_model(&db, "custom-db-key", &registry, None).await;
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
        let result = get_model(&db, "custom-no-key", &registry, None).await;
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
        let result = get_model(&db, "custom-missing", &registry, None).await;
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
        let result = get_model(&db, "chatgpt-4o-latest", &registry, None).await;
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
        let result = get_model(&db, "chatgpt_4o_latest", &registry, None).await;
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
        let result = get_model(&db, "claude-code-sonnet", &registry, None).await;
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
        let result = get_model(&db, "claude_code_opus", &registry, None).await;
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
        let result = get_model(&db, "openrouter:mistral-7b", &registry, None).await;
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
            let result = get_model(&db, model_name, &registry, None).await;
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
        let result = get_model(&db, "claude-code:sonnet-4", &registry, None).await;
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
        let result = get_model(&db, "my-claude", &registry, None).await;
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
        let result = get_model(&db, "my-chatgpt", &registry, None).await;
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
        let result = get_model(&db, "custom-braced", &registry, None).await;
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
        let result = get_model(&db, "friendly-name", &registry, None).await;
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
        let result = get_model(&db, "fallback-name", &registry, None).await;
        assert!(result.is_ok());
        let model = result.unwrap();
        assert!(model.identifier().contains("fallback-name"));
    }

    // =========================================================================
    // get_model_with_retry Tests
    // =========================================================================

    use super::super::RetryHandler;
    use std::sync::Arc;

    fn add_pool_key(db: &Database, provider: &str, api_key: &str, priority: i32) -> i64 {
        db.save_pool_key(provider, api_key, None, Some(priority))
            .unwrap()
    }

    #[tokio::test]
    async fn test_get_model_with_retry_no_handler() {
        let (_temp, db) = setup_test_db();
        let mut registry = ModelRegistry::new();
        let model = create_custom_model(
            "test-model",
            "https://api.example.com/v1",
            Some("sk-test-key"),
        );
        registry.add(model);

        let result = get_model_with_retry(&db, "test-model", &registry, None, None).await;
        assert!(result.is_ok());
        let (model, key_info) = result.unwrap();
        assert!(model.identifier().contains("test-model-id"));
        assert!(key_info.is_none(), "No key info when no retry handler");
    }

    #[allow(clippy::arc_with_non_send_sync)]
    fn setup_test_db_arc() -> (TempDir, Arc<Database>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, Arc::new(db))
    }

    #[tokio::test]
    async fn test_get_model_with_retry_no_pool_keys() {
        let (_temp, db) = setup_test_db_arc();
        let mut registry = ModelRegistry::new();
        let model = create_custom_model(
            "test-model",
            "https://api.example.com/v1",
            Some("sk-test-key"),
        );
        registry.add(model);

        let handler = RetryHandler::new(Arc::clone(&db));
        let result = get_model_with_retry(&db, "test-model", &registry, None, Some(&handler)).await;
        assert!(result.is_ok());
        let (model, key_info) = result.unwrap();
        assert!(model.identifier().contains("test-model-id"));
        assert!(
            key_info.is_none(),
            "No key info when no pool keys configured"
        );
    }

    #[tokio::test]
    async fn test_get_model_with_retry_uses_pool_key() {
        let (_temp, db) = setup_test_db_arc();
        let mut registry = ModelRegistry::new();
        let model = create_custom_model(
            "test-model",
            "https://api.example.com/v1",
            Some("$TEST_API_KEY"),
        );
        registry.add(model);

        // Add pool keys for "TEST_API_KEY" provider (matches the env var name)
        let key_id = add_pool_key(&db, "TEST_API_KEY", "sk-pool-key-1", 1);
        add_pool_key(&db, "TEST_API_KEY", "sk-pool-key-2", 2);

        let handler = RetryHandler::new(Arc::clone(&db));
        let result = get_model_with_retry(&db, "test-model", &registry, None, Some(&handler)).await;
        assert!(result.is_ok());
        let (model, key_info) = result.unwrap();
        assert!(model.identifier().contains("test-model-id"));
        assert!(key_info.is_some(), "Should have key info from pool");
        let info = key_info.unwrap();
        assert_eq!(info.provider, "TEST_API_KEY");
        assert_eq!(info.key_id, key_id);
    }

    // =========================================================================
    // extract_provider_from_config Tests
    // =========================================================================

    #[test]
    fn test_extract_provider_from_env_var() {
        let config =
            create_custom_model("test", "https://api.example.com", Some("$OPENAI_API_KEY"));
        let provider = extract_provider_from_config("test", &config);
        // Now returns the full env var name to match how keys are stored in DB
        assert_eq!(provider, "OPENAI_API_KEY");
    }

    #[test]
    fn test_extract_provider_from_env_var_with_braces() {
        let config =
            create_custom_model("test", "https://api.example.com", Some("${ANTHROPIC_KEY}"));
        let provider = extract_provider_from_config("test", &config);
        // Now returns the full env var name to match how keys are stored in DB
        assert_eq!(provider, "ANTHROPIC_KEY");
    }

    #[test]
    fn test_extract_provider_from_model_type_claude() {
        let config = ModelConfig {
            name: "test-claude".to_string(),
            model_type: ModelType::ClaudeCode,
            ..Default::default()
        };
        let provider = extract_provider_from_config("test-claude", &config);
        // Now returns env var format to match how keys are stored in DB
        assert_eq!(provider, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_extract_provider_from_model_type_chatgpt() {
        let config = ModelConfig {
            name: "test-gpt".to_string(),
            model_type: ModelType::ChatgptOauth,
            ..Default::default()
        };
        let provider = extract_provider_from_config("test-gpt", &config);
        // Now returns env var format to match how keys are stored in DB
        assert_eq!(provider, "OPENAI_API_KEY");
    }

    #[test]
    fn test_extract_provider_from_model_type_google() {
        let config = ModelConfig {
            name: "test-google".to_string(),
            model_type: ModelType::GoogleVertex,
            ..Default::default()
        };
        let provider = extract_provider_from_config("test-google", &config);
        // Now returns env var format to match how keys are stored in DB
        assert_eq!(provider, "GOOGLE_API_KEY");
    }

    #[test]
    fn test_extract_provider_from_custom_model_name() {
        let config = ModelConfig {
            name: "openrouter:mixtral".to_string(),
            model_type: ModelType::CustomOpenai,
            custom_endpoint: Some(CustomEndpoint {
                url: "https://api.openrouter.ai/v1".to_string(),
                api_key: Some("sk-literal".to_string()), // Literal key, not env var
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            ..Default::default()
        };
        let provider = extract_provider_from_config("openrouter:mixtral", &config);
        // Now returns env var format to match how keys are stored in DB
        assert_eq!(provider, "OPENROUTER_API_KEY");
    }

    // =========================================================================
    // provider_to_env_var Tests
    // =========================================================================

    #[test]
    fn test_provider_to_env_var_simple_name() {
        assert_eq!(provider_to_env_var("cerebras"), "CEREBRAS_API_KEY");
        assert_eq!(provider_to_env_var("openai"), "OPENAI_API_KEY");
        assert_eq!(provider_to_env_var("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(provider_to_env_var("google"), "GOOGLE_API_KEY");
        assert_eq!(provider_to_env_var("openrouter"), "OPENROUTER_API_KEY");
    }

    #[test]
    fn test_provider_to_env_var_already_formatted() {
        // If already in env var format, return as-is
        assert_eq!(provider_to_env_var("CEREBRAS_API_KEY"), "CEREBRAS_API_KEY");
        assert_eq!(provider_to_env_var("OPENAI_API_KEY"), "OPENAI_API_KEY");
        assert_eq!(provider_to_env_var("ANTHROPIC_KEY"), "ANTHROPIC_KEY");
    }

    #[test]
    fn test_key_info_debug() {
        let info = KeyInfo {
            provider: "openai".to_string(),
            key_id: 42,
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("openai"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_key_info_clone() {
        let info = KeyInfo {
            provider: "anthropic".to_string(),
            key_id: 123,
        };
        let cloned = info.clone();
        assert_eq!(cloned.provider, info.provider);
        assert_eq!(cloned.key_id, info.key_id);
    }
}
