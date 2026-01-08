//! Models settings tab
//!
//! Model management, add model dialog, and provider configuration.
//! Split into submodules for maintainability.

mod config_panel;
mod dialog;
mod provider_list;

use std::collections::BTreeMap;

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::config::Settings;
use crate::gui::app::ChatApp;
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
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
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
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(type_label),
            )
            .children(models.into_iter().map(|(model, desc)| {
                self.render_model_item(model, desc, current_default_model, cx)
            }))
    }

    /// Render a single model item.
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
        let desc = desc.unwrap_or_default();

        div()
            .id(SharedString::from(format!("default-model-{}", model)))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
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
            .cursor_pointer()
            .hover(|s| s.opacity(0.9))
            .flex()
            .items_center()
            .justify_between()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.current_model = model_name.clone();
                    let settings = Settings::new(&this.db);
                    let _ = settings.set("model", &model_name);
                    this.update_context_usage();
                    cx.notify();
                }),
            )
            .child(self.render_model_item_content(&model, &desc, is_selected))
            .child(self.render_model_delete_button(&model_name_for_delete, is_selected, cx))
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
            .child(Self::truncate_model_name(model));

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
}
