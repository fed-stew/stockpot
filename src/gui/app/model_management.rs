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

use gpui::{AppContext, AsyncApp, Context, WeakEntity};

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
            // Save to the unified api_key_pools table (not the legacy api_keys table)
            // This ensures all keys are managed through the pool system
            if let Err(e) = self
                .db
                .save_pool_key(env_var, &api_key_value, None, Some(0))
            {
                // Handle duplicate key error gracefully - key already exists
                let err_str = e.to_string();
                if err_str.contains("UNIQUE constraint") {
                    // Key already exists, that's fine - just continue
                    tracing::debug!(env_var = %env_var, "API key already exists in pool, skipping save");
                } else {
                    self.add_model_error = Some(format!("Failed to save API key: {}", e));
                    cx.notify();
                    return;
                }
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

    // =========================================================================
    // Key Pool Dialog Methods
    // =========================================================================

    /// Open the key pool dialog for a specific provider
    pub fn open_key_pool_dialog(
        &mut self,
        provider: &str,
        display_name: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_component::input::InputState;

        self.show_key_pool_dialog = true;
        self.key_pool_provider = Some(provider.to_string());
        self.key_pool_provider_display = Some(display_name.to_string());
        self.refresh_key_pool_list();

        // Create input entities
        self.key_pool_new_key_input = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Paste API key here...")
                .masked(true)
        }));
        self.key_pool_new_label_input = Some(cx.new(|cx| {
            InputState::new(window, cx).placeholder("Label (optional, e.g., 'Personal')")
        }));

        cx.notify();
    }

    /// Close the key pool dialog
    pub fn close_key_pool_dialog(&mut self, cx: &mut Context<Self>) {
        self.show_key_pool_dialog = false;
        self.key_pool_provider = None;
        self.key_pool_provider_display = None;
        self.key_pool_keys.clear();
        self.key_pool_new_key_input = None;
        self.key_pool_new_label_input = None;
        cx.notify();
    }

    /// Refresh the key list from database
    pub fn refresh_key_pool_list(&mut self) {
        if let Some(provider) = &self.key_pool_provider {
            self.key_pool_keys = self.db.get_pool_keys(provider).unwrap_or_default();
        }
    }

    /// Add a new key to the pool
    pub fn add_key_to_pool(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let Some(provider) = &self.key_pool_provider.clone() else {
            return;
        };

        let key_value = self
            .key_pool_new_key_input
            .as_ref()
            .map(|e| e.read(cx).value().to_string())
            .unwrap_or_default();

        let label = self
            .key_pool_new_label_input
            .as_ref()
            .map(|e| e.read(cx).value().to_string())
            .filter(|s| !s.is_empty());

        if key_value.is_empty() {
            return;
        }

        let priority = self.key_pool_keys.len() as i32; // Add at end
        let _ = self
            .db
            .save_pool_key(&provider, &key_value, label.as_deref(), Some(priority));

        // Clear inputs
        if let Some(input) = &self.key_pool_new_key_input {
            input.update(cx, |state, cx| state.set_value("".to_string(), window, cx));
        }
        if let Some(input) = &self.key_pool_new_label_input {
            input.update(cx, |state, cx| state.set_value("".to_string(), window, cx));
        }

        self.refresh_key_pool_list();
        cx.notify();
    }

    /// Delete a key from the pool
    pub fn delete_pool_key(&mut self, key_id: i64, cx: &mut Context<Self>) {
        let _ = self.db.delete_pool_key(key_id);
        self.refresh_key_pool_list();
        cx.notify();
    }

    /// Toggle key active status
    pub fn toggle_pool_key_active(&mut self, key_id: i64, is_active: bool, cx: &mut Context<Self>) {
        let _ = self.db.set_key_active(key_id, !is_active);
        self.refresh_key_pool_list();
        cx.notify();
    }

    /// Move a key up in priority (lower priority number = higher priority)
    pub fn move_key_up(&mut self, key_id: i64, cx: &mut Context<Self>) {
        if let Some(idx) = self.key_pool_keys.iter().position(|k| k.id == key_id) {
            if idx > 0 {
                let new_priority = self.key_pool_keys[idx - 1].priority;
                let old_priority = self.key_pool_keys[idx].priority;
                let _ = self.db.update_key_priority(key_id, new_priority);
                let _ = self
                    .db
                    .update_key_priority(self.key_pool_keys[idx - 1].id, old_priority);
                self.refresh_key_pool_list();
                cx.notify();
            }
        }
    }

    /// Move a key down in priority
    pub fn move_key_down(&mut self, key_id: i64, cx: &mut Context<Self>) {
        if let Some(idx) = self.key_pool_keys.iter().position(|k| k.id == key_id) {
            if idx < self.key_pool_keys.len() - 1 {
                let new_priority = self.key_pool_keys[idx + 1].priority;
                let old_priority = self.key_pool_keys[idx].priority;
                let _ = self.db.update_key_priority(key_id, new_priority);
                let _ = self
                    .db
                    .update_key_priority(self.key_pool_keys[idx + 1].id, old_priority);
                self.refresh_key_pool_list();
                cx.notify();
            }
        }
    }
}
