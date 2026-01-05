//! Models settings tab
//!
//! Model management, add model dialog, and provider configuration.

use std::collections::BTreeMap;

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::config::Settings;
use crate::gui::app::ChatApp;
use crate::gui::components::scrollbar;
use crate::models::ModelType;

impl ChatApp {
    pub(crate) fn render_settings_models(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let available_models = self.available_models.clone();
        let current_default_model = self.current_model.clone();

        let type_label_for = |name: &str, model_type: ModelType| -> String {
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
        };

        let mut by_type: BTreeMap<String, Vec<(String, Option<String>)>> = BTreeMap::new();
        for name in &available_models {
            if let Some(config) = self.model_registry.get(name) {
                let label = type_label_for(name, config.model_type);
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
        for models in by_type.values_mut() {
            models.sort_by(|a, b| a.0.cmp(&b.0));
        }

        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
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
                ),
            )
            .child(
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
                    .child(self.render_oauth_status("chatgpt", "ChatGPT", cx)),
            )
            .child(
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
                    ),
            )
            .child(
                div()
                    .id("settings-models-scroll")
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .children(by_type.into_iter().map(|(type_label, models)| {
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
                                    .child({
                                        let mut inner = div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .flex_1()
                                            .child(Self::truncate_model_name(&model));

                                        if !desc.is_empty() {
                                            inner = inner.child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(if is_selected {
                                                        rgb(0xffffff)
                                                    } else {
                                                        theme.text_muted
                                                    })
                                                    .child(desc),
                                            );
                                        }

                                        inner
                                    })
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "delete-model-{}",
                                                model_name_for_delete
                                            )))
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
                                            .hover(|s| {
                                                s.text_color(rgb(0xff6b6b)).bg(rgba(0xff6b6b22))
                                            })
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation();
                                            })
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.delete_model(&model_name_for_delete, cx);
                                                }),
                                            )
                                            .child("×"),
                                    )
                            }))
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn render_add_model_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_add_model_dialog;

        div().when(show, |d| {
            d.absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x000000aa))
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .w(px(700.))
                        .h(px(500.))
                        .bg(theme.panel_background)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(12.))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(20.))
                                .py(px(14.))
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .text_size(px(15.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.text)
                                        .child("Add API Key based Models"),
                                )
                                .child(
                                    div()
                                        .id("close-add-model")
                                        .px(px(8.))
                                        .py(px(4.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .text_color(theme.text_muted)
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, window, cx| {
                                                this.show_add_model_dialog = false;
                                                this.add_model_selected_provider = None;
                                                this.add_model_selected_model = None;
                                                this.add_model_models.clear();
                                                if let Some(input) =
                                                    &this.add_model_api_key_input_entity
                                                {
                                                    input.update(cx, |state, cx| {
                                                        state.set_value("", window, cx)
                                                    });
                                                }
                                                this.add_model_error = None;
                                                cx.notify();
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.))
                                .flex()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .w(px(250.))
                                        .min_h(px(0.))
                                        .border_r_1()
                                        .border_color(theme.border)
                                        .flex()
                                        .flex_col()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .px(px(16.))
                                                .py(px(12.))
                                                .border_b_1()
                                                .border_color(theme.border)
                                                .text_size(px(12.))
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.text_muted)
                                                .child("Providers"),
                                        )
                                        .child(self.render_provider_list(cx)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_h(px(0.))
                                        .flex()
                                        .flex_col()
                                        .overflow_hidden()
                                        .child(self.render_model_config_panel(cx)),
                                ),
                        ),
                )
        })
    }

    fn render_provider_list(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let theme = self.theme.clone();

        if self.add_model_loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_size(px(20.)).child("⏳"))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.text_muted)
                                .child("Loading providers..."),
                        ),
                )
                .into_any_element();
        }

        if let Some(error) = &self.add_model_error {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(16.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0xff6b6b))
                        .child(error.clone()),
                )
                .child(
                    div()
                        .id("retry-fetch")
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .bg(theme.tool_card)
                        .text_color(theme.text)
                        .text_size(px(12.))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.fetch_providers(cx);
                            }),
                        )
                        .child("Retry"),
                )
                .into_any_element();
        }

        div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .id("provider-list-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.add_model_providers_scroll_handle)
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| {
                        cx.notify();
                    }))
                    .children(self.add_model_providers.iter().map(|provider| {
                        let provider_id = provider.id.clone();
                        let is_selected =
                            self.add_model_selected_provider.as_ref() == Some(&provider_id);
                        let name = if provider.name.is_empty() {
                            provider.id.clone()
                        } else {
                            provider.name.clone()
                        };
                        let model_count = provider.models.len();

                        div()
                            .id(SharedString::from(format!("provider-{}", provider_id)))
                            .px(px(16.))
                            .py(px(10.))
                            .cursor_pointer()
                            .bg(if is_selected {
                                theme.accent
                            } else {
                                theme.panel_background
                            })
                            .text_color(if is_selected {
                                rgb(0xffffff)
                            } else {
                                theme.text
                            })
                            .hover(move |s| {
                                if is_selected {
                                    s
                                } else {
                                    s.bg(theme.tool_card)
                                }
                            })
                            .border_b_1()
                            .border_color(theme.border)
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.add_model_selected_provider = Some(provider_id.clone());
                                    if let Some(p) = this
                                        .add_model_providers
                                        .iter()
                                        .find(|p| p.id == provider_id)
                                    {
                                        this.add_model_models =
                                            p.models.values().cloned().collect();
                                        this.add_model_models.sort_by(|a, b| a.id.cmp(&b.id));
                                    }
                                    if let Some(input) = &this.add_model_api_key_input_entity {
                                        input.update(cx, |state, cx| {
                                            state.set_value("", window, cx)
                                        });
                                    }
                                    this.add_model_error = None;
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(div().text_size(px(13.)).child(name))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(if is_selected {
                                                rgba(0xffffffaa)
                                            } else {
                                                theme.text_muted
                                            })
                                            .child(format!("{} models", model_count)),
                                    ),
                            )
                    })),
            )
            .child(scrollbar(
                self.add_model_providers_scroll_handle.clone(),
                self.add_model_providers_scrollbar_drag.clone(),
                theme.clone(),
            ))
            .into_any_element()
    }

    fn render_model_config_panel(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let theme = self.theme.clone();

        let Some(provider_id) = &self.add_model_selected_provider else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme.text_muted)
                        .child("← Select a provider"),
                )
                .into_any_element();
        };

        let provider = self
            .add_model_providers
            .iter()
            .find(|p| &p.id == provider_id);
        let env_var = provider
            .and_then(|p| p.env.first())
            .map(|s| s.as_str())
            .unwrap_or("API_KEY");

        let has_existing_key = self.db.has_api_key(env_var) || std::env::var(env_var).is_ok();
        let has_key_input = self
            .add_model_api_key_input_entity
            .as_ref()
            .map(|e| !e.read(cx).value().is_empty())
            .unwrap_or(false);
        let can_add_models = has_existing_key || has_key_input;

        let provider_id = provider_id.clone();
        let env_var = env_var.to_string();

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.text_muted)
                                    .child(format!("API Key ({})", env_var)),
                            )
                            .when(has_existing_key, |d| {
                                d.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(0x4ade80))
                                        .child("✓ Key configured"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(div().flex_1().min_h(px(44.)).when_some(
                                self.add_model_api_key_input_entity.as_ref(),
                                |d, input| {
                                    d.child(gpui_component::input::Input::new(input).flex_1())
                                },
                            ))
                            .child(
                                div()
                                    .id("paste-api-key")
                                    .px(px(12.))
                                    .py(px(8.))
                                    .rounded(px(6.))
                                    .bg(theme.tool_card)
                                    .text_color(theme.text)
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.8))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            if let Some(text) =
                                                cx.read_from_clipboard().and_then(|i| i.text())
                                            {
                                                if let Some(input) =
                                                    &this.add_model_api_key_input_entity
                                                {
                                                    input.update(cx, |state, cx| {
                                                        state.set_value(
                                                            text.to_string(),
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    .child("Paste"),
                            ),
                    ),
            )
            .child(
                div()
                    .px(px(16.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child(format!("Models ({})", self.add_model_models.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .child(
                        div()
                            .id("models-list-scroll")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .track_scroll(&self.add_model_models_scroll_handle)
                            .on_scroll_wheel(cx.listener(|_, _, _, cx| {
                                cx.notify();
                            }))
                            .children(self.add_model_models.iter().map(|model| {
                                let model_id = model.id.clone();
                                let model_name =
                                    model.name.clone().unwrap_or_else(|| model.id.clone());
                                let provider_id = provider_id.clone();
                                let env_var = env_var.clone();
                                let can_add = can_add_models;

                                let ctx_info = model
                                    .context_length
                                    .map(|c| format!("{}k", c / 1000))
                                    .unwrap_or_default();

                                div()
                                    .id(SharedString::from(format!("model-{}", model_id)))
                                    .px(px(16.))
                                    .py(px(10.))
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .text_color(theme.text)
                                                    .child(model_name),
                                            )
                                            .when(!ctx_info.is_empty(), |d| {
                                                d.child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_muted)
                                                        .child(ctx_info),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "add-model-{}",
                                                model_id
                                            )))
                                            .px(px(10.))
                                            .py(px(6.))
                                            .rounded(px(6.))
                                            .bg(if can_add {
                                                theme.accent
                                            } else {
                                                theme.tool_card
                                            })
                                            .text_color(if can_add {
                                                rgb(0xffffff)
                                            } else {
                                                theme.text_muted
                                            })
                                            .text_size(px(12.))
                                            .cursor(if can_add {
                                                gpui::CursorStyle::PointingHand
                                            } else {
                                                gpui::CursorStyle::Arrow
                                            })
                                            .when(can_add, |d| d.hover(|s| s.opacity(0.8)))
                                            .when(can_add, |d| {
                                                d.on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.add_single_model(
                                                            &provider_id,
                                                            &model_id,
                                                            &env_var,
                                                            cx,
                                                        );
                                                    }),
                                                )
                                            })
                                            .child("+"),
                                    )
                            })),
                    )
                    .child(scrollbar(
                        self.add_model_models_scroll_handle.clone(),
                        self.add_model_models_scrollbar_drag.clone(),
                        theme.clone(),
                    )),
            )
            .into_any_element()
    }
}
