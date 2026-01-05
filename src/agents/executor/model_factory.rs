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
