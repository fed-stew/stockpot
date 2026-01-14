//! Model management for ChatApp
//!
//! This module handles model and provider operations:
//! - `refresh_models()` - Refresh the available models list
//! - `fetch_providers()` - Fetch available providers for add-model dialog
//! - `add_single_model()` - Add a new model configuration
//! - `delete_model()` - Remove a model configuration
//! - `start_oauth_flow()` - Initiate OAuth for a provider
//! - `refresh_api_keys_list()` - Refresh stored API keys

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AsyncApp, Context, WeakEntity};

use crate::models::ModelRegistry;

use super::ChatApp;

impl ChatApp {
    pub(super) fn refresh_models(&mut self) {
        tracing::debug!("refresh_models: starting");
        match ModelRegistry::load_from_db(&self.db) {
            Ok(registry) => {
                let total_in_registry = registry.len();
                self.available_models = registry.list_available(&self.db);
                let available_count = self.available_models.len();
                tracing::debug!(
                    total_in_registry = total_in_registry,
                    available_count = available_count,
                    models = ?self.available_models,
                    "refresh_models: complete"
                );
                self.model_registry = Arc::new(registry);
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to refresh model registry");
            }
        }
    }

    pub(super) fn fetch_providers(&mut self, cx: &mut Context<Self>) {
        self.add_model_loading = true;
        self.add_model_error = None;
        self.add_model_providers.clear();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            match crate::models::catalog::fetch_providers().await {
                Ok(providers) => {
                    this.update(cx, |app, cx| {
                        app.add_model_providers = providers.into_values().collect();
                        // Sort by name
                        app.add_model_providers.sort_by(|a, b| a.name.cmp(&b.name));
                        app.add_model_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |app, cx| {
                        app.add_model_error = Some(format!("Failed to load providers: {}", e));
                        app.add_model_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Add a single model from the Add Models dialog
    pub(super) fn add_single_model(
        &mut self,
        provider_id: &str,
        model_id: &str,
        env_var: &str,
        cx: &mut Context<Self>,
    ) {
        use crate::models::{CustomEndpoint, ModelConfig, ModelType};

        self.add_model_error = None;

        let api_key_value = self
            .add_model_api_key_input_entity
            .as_ref()
            .map(|e| e.read(cx).value().to_string())
            .unwrap_or_default();

        if !api_key_value.is_empty() {
            if let Err(e) = self.db.save_api_key(env_var, &api_key_value) {
                self.add_model_error = Some(format!("Failed to save API key: {}", e));
                cx.notify();
                return;
            }
        }

        let provider = self
            .add_model_providers
            .iter()
            .find(|p| p.id == provider_id);
        let model = self.add_model_models.iter().find(|m| m.id == model_id);

        let api_url = provider
            .and_then(|p| p.api.clone())
            .unwrap_or_else(|| match provider_id {
                "cerebras" => "https://api.cerebras.ai/v1".to_string(),
                "together" => "https://api.together.xyz/v1".to_string(),
                "groq" => "https://api.groq.com/openai/v1".to_string(),
                "fireworks" => "https://api.fireworks.ai/inference/v1".to_string(),
                "deepseek" => "https://api.deepseek.com/v1".to_string(),
                "mistral" => "https://api.mistral.ai/v1".to_string(),
                "perplexity" => "https://api.perplexity.ai".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });

        let model_name = format!("{}:{}", provider_id, model_id);
        let context_length = model.and_then(|m| m.context_length).unwrap_or(128_000) as usize;
        let description = model
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| model_id.to_string());

        let config = ModelConfig {
            name: model_name.clone(),
            model_type: ModelType::CustomOpenai,
            model_id: Some(model_id.to_string()),
            context_length,
            supports_thinking: false,
            supports_vision: false,
            supports_tools: true,
            description: Some(description),
            custom_endpoint: Some(CustomEndpoint {
                url: api_url,
                api_key: Some(format!("${}", env_var)),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        if let Err(e) = ModelRegistry::add_model_to_db(&self.db, &config) {
            self.add_model_error = Some(format!("Failed to save model: {}", e));
            cx.notify();
            return;
        }

        let registry = ModelRegistry::load_from_db(&self.db).unwrap_or_default();
        self.available_models = registry.list_available(&self.db);
        self.model_registry = Arc::new(registry);

        cx.notify();
    }

    /// Delete a model from the registry
    pub(super) fn delete_model(&mut self, model_name: &str, cx: &mut Context<Self>) {
        if self.current_model == model_name {
            if let Some(other) = self
                .available_models
                .iter()
                .find(|m| m.as_str() != model_name)
            {
                self.current_model = other.clone();
                let settings = crate::config::Settings::new(&self.db);
                let _ = settings.set("model", &self.current_model);
            }
        }

        if let Err(e) = ModelRegistry::remove_model_from_db(&self.db, model_name) {
            tracing::warn!("Failed to delete model {}: {}", model_name, e);
            return;
        }

        let registry = ModelRegistry::load_from_db(&self.db).unwrap_or_default();
        self.available_models = registry.list_available(&self.db);
        self.model_registry = Arc::new(registry);

        cx.notify();
    }

    /// Start OAuth authentication flow
    pub(super) fn start_oauth_flow(&mut self, provider: &'static str, cx: &mut Context<Self>) {
        let db = self.db.clone();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            let result = match provider {
                "chatgpt" => crate::auth::run_chatgpt_auth(&db)
                    .await
                    .map_err(|e| e.to_string()),
                "claude-code" => crate::auth::run_claude_code_auth(&db)
                    .await
                    .map_err(|e| e.to_string()),
                "google" => crate::auth::run_google_auth(&db)
                    .await
                    .map_err(|e| e.to_string()),
                _ => Err(format!("Unknown provider: {}", provider)),
            };

            this.update(cx, |app, cx| {
                match result {
                    Ok(_) => {
                        // Refresh models to pick up newly registered OAuth models
                        app.refresh_models();
                    }
                    Err(e) => {
                        app.error_message = Some(format!("OAuth failed: {}", e));
                    }
                }
                cx.notify();
            })
            .map_err(|e| tracing::error!("this.update() failed: {:?}", e))
            .ok();
        })
        .detach();
    }

    /// Refresh the API keys list from database
    pub(super) fn refresh_api_keys_list(&mut self) {
        self.api_keys_list = self.db.list_api_keys().unwrap_or_default();
    }
}
