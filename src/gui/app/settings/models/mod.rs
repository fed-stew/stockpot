//! Models settings tab
//!
//! Model management, add model dialog, and provider configuration.
//! Split into submodules for maintainability.

mod config_panel;
mod dialog;
mod provider_list;

use std::collections::BTreeMap;

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};
use gpui_component::input::{Input, InputState};

use crate::config::Settings;
use crate::gui::app::ChatApp;
use crate::models::settings::ModelSettings as SpotModelSettings;
use crate::models::ModelType;

impl ChatApp {
    /// Render the models settings tab content.
    pub(crate) fn render_settings_models(&self, cx: &Context<Self>) -> impl IntoElement {
        let available_models = self.available_models.clone();
        let current_default_model = self.current_model.clone();

        // Group models by provider type for display
        let by_type = self.group_models_by_type(&available_models);

        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(self.render_add_model_button(cx))
            .child(self.render_oauth_accounts(cx))
            .child(self.render_available_models_header(cx))
            .child(self.render_models_list(by_type, &current_default_model, cx))
            .into_any_element()
    }

    /// Group available models by their provider type.
    fn group_models_by_type(
        &self,
        available_models: &[String],
    ) -> BTreeMap<String, Vec<(String, Option<String>)>> {
        let mut by_type: BTreeMap<String, Vec<(String, Option<String>)>> = BTreeMap::new();

        for name in available_models {
            if let Some(config) = self.model_registry.get(name) {
                let label = Self::type_label_for(name, config.model_type);
                by_type
                    .entry(label)
                    .or_default()
                    .push((name.clone(), config.description.clone()));
            } else {
                by_type
                    .entry("Unknown".to_string())
                    .or_default()
                    .push((name.clone(), None));
            }
        }

        // Sort models within each type
        for models in by_type.values_mut() {
            models.sort_by(|a, b| a.0.cmp(&b.0));
        }

        by_type
    }

    /// Get a human-readable label for a model type.
    fn type_label_for(name: &str, model_type: ModelType) -> String {
        match model_type {
            ModelType::Openai => "OpenAI".to_string(),
            ModelType::Anthropic => "Anthropic".to_string(),
            ModelType::Gemini => "Google Gemini".to_string(),
            ModelType::ClaudeCode => "Claude Code (OAuth)".to_string(),
            ModelType::ChatgptOauth => "ChatGPT (OAuth)".to_string(),
            ModelType::AzureOpenai => "Azure OpenAI".to_string(),
            ModelType::Openrouter => "OpenRouter".to_string(),
            ModelType::RoundRobin => "Round Robin".to_string(),
            ModelType::CustomOpenai | ModelType::CustomAnthropic => {
                if let Some(idx) = name.find(':') {
                    let provider = &name[..idx];
                    let mut chars = provider.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().chain(chars).collect(),
                        None => "Custom".to_string(),
                    }
                } else {
                    "Custom".to_string()
                }
            }
        }
    }

    /// Render the "Add API Key based Models" button.
    fn render_add_model_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div().child(
            div()
                .id("add-model-btn")
                .px(px(16.))
                .py(px(10.))
                .rounded(px(8.))
                .bg(theme.accent)
                .text_color(rgb(0xffffff))
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.show_add_model_dialog = true;
                        this.add_model_selected_provider = None;
                        this.add_model_selected_model = None;
                        this.add_model_models.clear();
                        this.add_model_error = None;

                        if this.add_model_api_key_input_entity.is_none() {
                            this.add_model_api_key_input_entity = Some(cx.new(|cx| {
                                gpui_component::input::InputState::new(window, cx)
                                    .placeholder("Enter API key...")
                            }));
                        }

                        if let Some(input) = &this.add_model_api_key_input_entity {
                            input.update(cx, |state, cx| state.set_value("", window, cx));
                        }

                        this.fetch_providers(cx);
                        cx.notify();
                    }),
                )
                .child("➕ Add API Key based Models"),
        )
    }

    /// Render the OAuth accounts section.
    fn render_oauth_accounts(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("OAuth Accounts"),
            )
            .child(self.render_oauth_status("claude-code", "Claude Code", cx))
            .child(self.render_oauth_status("chatgpt", "ChatGPT", cx))
    }

    /// Render the "Available Models" header with refresh button.
    fn render_available_models_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child("Available Models"),
            )
            .child(
                div()
                    .id("refresh-models-btn")
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(theme.tool_card)
                    .text_color(theme.text_muted)
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.accent).text_color(rgb(0xffffff)))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.refresh_models();
                            cx.notify();
                        }),
                    )
                    .child("🔄 Refresh"),
            )
    }

    /// Render the list of models grouped by type.
    fn render_models_list(
        &self,
        by_type: BTreeMap<String, Vec<(String, Option<String>)>>,
        current_default_model: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("settings-models-scroll")
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(by_type.into_iter().map(|(type_label, models)| {
                self.render_model_type_group(type_label, models, current_default_model, cx)
            }))
    }

    /// Render a group of models for a specific type.
    fn render_model_type_group(
        &self,
        type_label: String,
        models: Vec<(String, Option<String>)>,
        current_default_model: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(type_label),
            )
            .children(models.into_iter().map(|(model, desc)| {
                self.render_model_item(model, desc, current_default_model, cx)
            }))
    }

    /// Render a single model item (with expandable settings for non-OAuth models).
    fn render_model_item(
        &self,
        model: String,
        desc: Option<String>,
        current_default_model: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let is_selected = model == current_default_model;
        let model_name = model.clone();
        let model_name_for_delete = model.clone();
        let model_name_for_expand = model.clone();
        
        // Don't show description for OAuth models - the name is already descriptive
        let desc = if model.starts_with("claude-code-") || model.starts_with("chatgpt-") {
            String::new()
        } else {
            desc.unwrap_or_default()
        };

        // Check if this model is expandable (non-OAuth models)
        let is_expandable = self
            .model_registry
            .get(&model)
            .map(|c| !c.is_oauth())
            .unwrap_or(false);

        let is_expanded = self
            .expanded_settings_model
            .as_ref()
            .map(|m| m == &model)
            .unwrap_or(false);

        // Chevron indicator
        let chevron = if is_expandable {
            if is_expanded {
                "▼"
            } else {
                "▶"
            }
        } else {
            ""
        };

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(SharedString::from(format!("default-model-{}", model)))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded_t(px(8.))
                    .when(!is_expanded, |d| d.rounded_b(px(8.)))
                    .bg(if is_selected {
                        theme.accent
                    } else {
                        theme.tool_card
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        theme.text
                    })
                    .overflow_hidden()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .flex()
                    .items_center()
                    .justify_between()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            if is_expandable {
                                // Toggle expansion
                                if this.expanded_settings_model.as_ref()
                                    == Some(&model_name_for_expand)
                                {
                                    // Collapse
                                    this.expanded_settings_model = None;
                                    this.model_temp_input_entity = None;
                                    this.model_top_p_input_entity = None;
                                    this.model_api_key_input_entity = None;
                                    this.model_settings_save_success = None;
                                } else {
                                    // Expand and initialize inputs (also resets save success)
                                    this.expanded_settings_model =
                                        Some(model_name_for_expand.clone());
                                    this.model_settings_save_success = None;
                                    this.initialize_model_settings_inputs(
                                        &model_name_for_expand,
                                        window,
                                        cx,
                                    );
                                }
                            } else {
                                // Non-expandable: just set as default
                                this.current_model = model_name.clone();
                                let settings = Settings::new(&this.db);
                                let _ = settings.set("model", &model_name);
                                this.update_context_usage();
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .when(is_expandable, |d| {
                                d.child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(if is_selected {
                                            rgba(0xffffffaa)
                                        } else {
                                            theme.text_muted
                                        })
                                        .child(chevron),
                                )
                            })
                            .child(self.render_model_item_content(&model, &desc, is_selected)),
                    )
                    .child(self.render_model_delete_button(
                        &model_name_for_delete,
                        is_selected,
                        cx,
                    )),
            )
            // Expanded settings panel
            .when(is_expanded, |d| {
                d.child(self.render_model_settings_panel(&model, cx))
            })
    }

    /// Render the content (name + description) for a model item.
    fn render_model_item_content(
        &self,
        model: &str,
        desc: &str,
        is_selected: bool,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let mut inner = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .text_size(px(13.))
                    .overflow_hidden()
                    .child(Self::truncate_model_name(model)),
            );

        if !desc.is_empty() {
            inner = inner.child(
                div()
                    .text_size(px(11.))
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        theme.text_muted
                    })
                    .child(desc.to_string()),
            );
        }

        inner
    }

    /// Render the delete button for a model item.
    fn render_model_delete_button(
        &self,
        model_name: &str,
        is_selected: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let model_name = model_name.to_string();

        div()
            .id(SharedString::from(format!("delete-model-{}", model_name)))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(4.))
            .text_size(px(12.))
            .text_color(if is_selected {
                rgba(0xffffffaa)
            } else {
                theme.text_muted
            })
            .cursor_pointer()
            .hover(|s| s.text_color(rgb(0xff6b6b)).bg(rgba(0xff6b6b22)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.delete_model(&model_name, cx);
                }),
            )
            .child("×")
    }

    // =========================================================================
    // Expanded Model Settings Panel
    // =========================================================================

    /// Render the expanded configuration panel for a model.
    fn render_model_settings_panel(
        &self,
        model_name: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();

        // Get the model config to find the api_key_env
        let api_key_env = self
            .model_registry
            .get(model_name)
            .and_then(|c| c.custom_endpoint.as_ref())
            .and_then(|e| e.api_key.as_ref())
            .and_then(|k| {
                if k.starts_with('$') {
                    Some(
                        k.trim_start_matches('$')
                            .trim_matches(|c| c == '{' || c == '}')
                            .to_string(),
                    )
                } else {
                    None
                }
            });

        let model_name_for_save = model_name.to_string();
        let api_key_env_for_save = api_key_env.clone();

        div()
            .px(px(12.))
            .py(px(12.))
            .rounded_b(px(8.))
            .bg(theme.panel_background)
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(12.))
            // API Key section (only if model has api_key_env)
            .when_some(api_key_env.clone(), |d, env_var| {
                d.child(self.render_api_key_setting_row(&env_var, cx))
            })
            // Temperature
            .child(self.render_temp_input(cx))
            // Top P
            .child(self.render_top_p_input(cx))
            // Save button
            .child(self.render_save_settings_button(&model_name_for_save, api_key_env_for_save, cx))
    }

    /// Render the temperature input row.
    fn render_temp_input(&self, _cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("Temperature (0.0 - 2.0)"),
            )
            .child(
                div()
                    .w(px(80.))
                    .when_some(self.model_temp_input_entity.as_ref(), |d, input| {
                        d.child(Input::new(input))
                    }),
            )
    }

    /// Render the top_p input row.
    fn render_top_p_input(&self, _cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("Top P (0.0 - 1.0)"),
            )
            .child(
                div()
                    .w(px(80.))
                    .when_some(self.model_top_p_input_entity.as_ref(), |d, input| {
                        d.child(Input::new(input))
                    }),
            )
    }

    /// Render the API key setting row.
    fn render_api_key_setting_row(&self, env_var: &str, _cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let has_key = self.db.has_api_key(env_var) || std::env::var(env_var).is_ok();

        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .child(format!("API Key ({})", env_var)),
                    )
                    .when(has_key, |d| {
                        d.child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(0x4ade80))
                                .child("✓"),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(200.))
                    .when_some(self.model_api_key_input_entity.as_ref(), |d, input| {
                        d.child(Input::new(input))
                    }),
            )
    }

    /// Render the save settings button.
    fn render_save_settings_button(
        &self,
        model_name: &str,
        api_key_env: Option<String>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let model_name = model_name.to_string();

        // Check if save was recent (within 2 seconds)
        let is_save_success = self
            .model_settings_save_success
            .map(|t| t.elapsed() < std::time::Duration::from_secs(2))
            .unwrap_or(false);

        let (bg_color, button_text) = if is_save_success {
            (rgb(0x22c55e), "Saved ✓")
        } else {
            (theme.accent, "Save")
        };

        div().flex().justify_end().child(
            div()
                .id("save-model-settings")
                .px(px(12.))
                .py(px(6.))
                .rounded(px(6.))
                .bg(bg_color)
                .text_color(rgb(0xffffff))
                .text_size(px(12.))
                .cursor_pointer()
                .when(!is_save_success, |d| d.hover(|s| s.opacity(0.9)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.save_model_settings(&model_name, api_key_env.clone(), window, cx);
                    }),
                )
                .child(button_text),
        )
    }

    /// Initialize input entities for model settings.
    pub(crate) fn initialize_model_settings_inputs(
        &mut self,
        model_name: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        // Load current settings
        let settings = SpotModelSettings::load(&self.db, model_name).unwrap_or_default();

        // Create temperature input
        let temp_value = settings
            .temperature
            .map(|t| format!("{:.1}", t))
            .unwrap_or_else(|| "0.7".to_string());
        self.model_temp_input_entity = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0.7")
                .default_value(temp_value)
        }));

        // Create top_p input (use enough precision for values like 0.951)
        let top_p_value = settings
            .top_p
            .map(|t| {
                // Format with up to 3 decimal places, trimming trailing zeros
                let s = format!("{:.3}", t);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            })
            .unwrap_or_else(|| "1.0".to_string());
        self.model_top_p_input_entity = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("1.0")
                .default_value(top_p_value)
        }));

        // Create API key input - show masked value if key exists
        let api_key_env = self
            .model_registry
            .get(model_name)
            .and_then(|c| c.custom_endpoint.as_ref())
            .and_then(|e| e.api_key.as_ref())
            .and_then(|k| {
                if k.starts_with('$') {
                    Some(
                        k.trim_start_matches('$')
                            .trim_matches(|c| c == '{' || c == '}')
                            .to_string(),
                    )
                } else {
                    None
                }
            });

        let has_existing_key = api_key_env
            .as_ref()
            .map(|env| self.db.has_api_key(env) || std::env::var(env).is_ok())
            .unwrap_or(false);

        self.model_api_key_input_entity = Some(cx.new(|cx| {
            let input = InputState::new(window, cx);
            if has_existing_key {
                input.placeholder("••••••••••••  (enter new key to replace)")
            } else {
                input.placeholder("Enter API key...")
            }
        }));
    }

    /// Save model settings from the input fields.
    pub(crate) fn save_model_settings(
        &mut self,
        model_name: &str,
        api_key_env: Option<String>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let mut had_error = false;

        // Save temperature
        if let Some(input) = &self.model_temp_input_entity {
            let value = input.read(cx).value().to_string();
            if !value.is_empty() {
                if let Err(e) =
                    SpotModelSettings::save_setting(&self.db, model_name, "temperature", &value)
                {
                    self.error_message = Some(format!("Failed to save temperature: {}", e));
                    had_error = true;
                }
            }
        }

        // Save top_p
        if let Some(input) = &self.model_top_p_input_entity {
            let value = input.read(cx).value().to_string();
            if !value.is_empty() {
                if let Err(e) =
                    SpotModelSettings::save_setting(&self.db, model_name, "top_p", &value)
                {
                    self.error_message = Some(format!("Failed to save top_p: {}", e));
                    had_error = true;
                }
            }
        }

        // Save API key if provided
        if let Some(env_var) = api_key_env {
            if let Some(input) = &self.model_api_key_input_entity {
                let value = input.read(cx).value().to_string();
                if !value.is_empty() {
                    if let Err(e) = self.db.save_api_key(&env_var, &value) {
                        self.error_message = Some(format!("Failed to save API key: {}", e));
                        had_error = true;
                    } else {
                        // Clear the input after saving
                        input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                    }
                }
            }
        }

        // Show success feedback if no errors
        if !had_error {
            self.model_settings_save_success = Some(std::time::Instant::now());

            // Schedule a re-render after 2 seconds to reset the button
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                this.update(cx, |_this, cx| {
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }

        cx.notify();
    }
}
