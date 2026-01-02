use gpui::{
    anchored, deferred, div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString,
    StatefulInteractiveElement, Styled,
};

use super::ChatApp;
use crate::agents::UserMode;
use crate::config::Settings;
use crate::gui::components::scrollbar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsTab {
    PinnedAgents,
    Models,
    General,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            Self::PinnedAgents => "Pinned Agents",
            Self::Models => "Models",
            Self::General => "General",
        }
    }
}

impl ChatApp {
    pub(super) fn render_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_settings;
        let tab = self.settings_tab;

        div().when(show, |d| {
            d.absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(theme.background)
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if this.show_default_model_dropdown {
                            this.show_default_model_dropdown = false;
                            cx.notify();
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .child(
                            // Header
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(20.))
                                .py(px(14.))
                                .border_b_1()
                                .border_color(theme.border)
                                .bg(theme.panel_background)
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.text)
                                        .child("Settings"),
                                )
                                .child(
                                    div()
                                        .id("close-settings")
                                        .px(px(10.))
                                        .py(px(6.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .text_color(theme.text_muted)
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.show_settings = false;
                                                this.show_default_model_dropdown = false;
                                                this.default_model_dropdown_bounds = None;
                                                cx.notify();
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                        .child(
                            // Tabs
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .px(px(20.))
                                .py(px(10.))
                                .border_b_1()
                                .border_color(theme.border)
                                .bg(theme.panel_background)
                                .children(
                                    [
                                        SettingsTab::PinnedAgents,
                                        SettingsTab::Models,
                                        SettingsTab::General,
                                    ]
                                    .into_iter()
                                    .map(|t| {
                                        let is_selected = t == tab;
                                        div()
                                            .id(SharedString::from(format!("settings-tab-{:?}", t)))
                                            .px(px(12.))
                                            .py(px(7.))
                                            .rounded(px(999.))
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
                                            .text_size(px(12.))
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.9))
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.settings_tab = t;
                                                    cx.notify();
                                                }),
                                            )
                                            .child(t.label())
                                    }),
                                ),
                        )
                        .child(
                            // Content
                            div()
                                .id("settings-content-wrap")
                                .flex()
                                .flex_1()
                                .min_h(px(0.))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .id("settings-content-scroll")
                                        .flex_1()
                                        .min_h(px(0.))
                                        .overflow_y_scroll()
                                        .track_scroll(&self.settings_scroll_handle)
                                        .on_scroll_wheel(cx.listener(|_, _, _, cx| {
                                            cx.notify();
                                        }))
                                        .px(px(20.))
                                        .py(px(18.))
                                        .child(
                                            div()
                                                .when(tab == SettingsTab::PinnedAgents, |d| {
                                                    d.child(self.render_settings_pinned_agents(cx))
                                                })
                                                .when(tab == SettingsTab::Models, |d| {
                                                    d.child(self.render_settings_models(cx))
                                                })
                                                .when(tab == SettingsTab::General, |d| {
                                                    d.child(self.render_settings_general(cx))
                                                }),
                                        ),
                                )
                                .child(scrollbar(
                                    self.settings_scroll_handle.clone(),
                                    self.settings_scrollbar_drag.clone(),
                                    theme.clone(),
                                )),
                        ),
                )
        })
    }

    fn render_settings_pinned_agents(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let agents = self.agents.list();
        let available_models = self.available_models.clone();
        let default_model = self.current_model.clone();
        let selected_agent = self.settings_selected_agent.clone();

        let view = cx.entity().clone();

        let settings = Settings::new(&self.db);
        let pins = settings.get_all_agent_pinned_models().unwrap_or_default();

        let bounds_tracker = gpui::canvas(
            move |bounds, _window, cx| {
                let should_update = view.read(cx).default_model_dropdown_bounds != Some(bounds);
                if should_update {
                    view.update(cx, |this, _| {
                        this.default_model_dropdown_bounds = Some(bounds);
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let default_model_section = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .mb(px(16.))
            .pb(px(16.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Default Model"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("Used when an agent does not have a pinned model."),
            )
            .child(
                div()
                    .child(
                        div()
                            .id("default-model-dropdown")
                            .mt(px(4.))
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(theme.tool_card)
                            .cursor_pointer()
                            .relative()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.show_default_model_dropdown =
                                        !this.show_default_model_dropdown;
                                    cx.notify();
                                }),
                            )
                            .child(bounds_tracker)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(theme.text)
                                            .child(default_model.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(theme.text_muted)
                                            .child(if self.show_default_model_dropdown {
                                                "▲"
                                            } else {
                                                "▼"
                                            }),
                                    ),
                            ),
                    )
                    .when(
                        self.show_default_model_dropdown
                            && self.default_model_dropdown_bounds.is_some(),
                        |d| {
                            let bounds = self.default_model_dropdown_bounds.unwrap();
                            let position = gpui::Point::new(
                                bounds.origin.x,
                                bounds.origin.y + bounds.size.height + px(4.),
                            );

                            d.child(deferred(
                                anchored().position(position).child(
                                    div()
                                        .id("default-model-dropdown-list")
                                        .w(bounds.size.width.max(px(280.)))
                                        .max_h(px(300.))
                                        .overflow_y_scroll()
                                        .scrollbar_width(px(8.))
                                        .rounded(px(8.))
                                        .bg(theme.panel_background)
                                        .border_1()
                                        .border_color(theme.border)
                                        .shadow_lg()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .children(available_models.iter().map(|model| {
                                            let is_selected = model == &default_model;
                                            let model_name = model.clone();

                                            div()
                                                .id(SharedString::from(format!(
                                                    "default-dropdown-{}",
                                                    model
                                                )))
                                                .px(px(12.))
                                                .py(px(8.))
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
                                                .text_size(px(12.))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(theme.tool_card))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.current_model = model_name.clone();
                                                        let settings = Settings::new(&this.db);
                                                        let _ = settings.set("model", &model_name);
                                                        this.show_default_model_dropdown = false;
                                                        cx.notify();
                                                    }),
                                                )
                                                .child(model.clone())
                                        })),
                                ),
                            ))
                        },
                    ),
            );

        let left_panel = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Agents"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("Select an agent, then pin a model."),
            )
            .child(
                div()
                    .id("settings-agents-scroll")
                    .mt(px(6.))
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .scrollbar_width(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(agents.into_iter().map(|info| {
                        let is_selected = info.name == selected_agent;
                        let pinned = pins.get(&info.name).cloned();
                        let subtitle = match pinned {
                            Some(p) => format!("Pinned: {}", Self::truncate_model_name(&p)),
                            None => {
                                format!("Default: {}", Self::truncate_model_name(&default_model))
                            }
                        };

                        let agent_name = info.name.clone();
                        div()
                            .id(SharedString::from(format!("pin-agent-{}", agent_name)))
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
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.settings_selected_agent = agent_name.clone();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(info.display_name)
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(if is_selected {
                                                rgb(0xffffff)
                                            } else {
                                                theme.text_muted
                                            })
                                            .child(subtitle),
                                    ),
                            )
                    })),
            );

        let pinned_for_selected = Settings::new(&self.db).get_agent_pinned_model(&selected_agent);

        let right_panel = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Models"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child(format!("Pin a model for: {}", selected_agent)),
            )
            .child(
                div()
                    .id("settings-pin-models-scroll")
                    .mt(px(6.))
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .scrollbar_width(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child({
                        let is_selected = pinned_for_selected.is_none();
                        let agent_name = selected_agent.clone();
                        let default_label = format!(
                            "Use Default ({})",
                            Self::truncate_model_name(&default_model)
                        );

                        div()
                            .id("pin-model-default")
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
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let settings = Settings::new(&this.db);
                                    if let Err(e) = settings.clear_agent_pinned_model(&agent_name) {
                                        tracing::warn!(
                                            "Failed to clear pinned model for {}: {}",
                                            agent_name,
                                            e
                                        );
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(default_label)
                    })
                    .children(available_models.iter().map(|model| {
                        let pinned = pinned_for_selected.as_deref() == Some(model.as_str());
                        let agent_name = selected_agent.clone();
                        let model_name = model.clone();

                        div()
                            .id(SharedString::from(format!("pin-model-{}", model)))
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(if pinned {
                                theme.accent
                            } else {
                                theme.tool_card
                            })
                            .text_color(if pinned { rgb(0xffffff) } else { theme.text })
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let settings = Settings::new(&this.db);
                                    if let Err(e) =
                                        settings.set_agent_pinned_model(&agent_name, &model_name)
                                    {
                                        tracing::warn!(
                                            "Failed to pin model for {}: {}",
                                            agent_name,
                                            e
                                        );
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(Self::truncate_model_name(model))
                    })),
            );

        div().flex().flex_col().child(default_model_section).child(
            div()
                .flex()
                .gap(px(18.))
                .child(div().w(px(360.)).child(left_panel))
                .child(div().flex_1().child(right_panel)),
        )
    }

    fn render_settings_models(&self, cx: &Context<Self>) -> impl IntoElement {
        use crate::models::ModelType;
        use std::collections::BTreeMap;

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
                            cx.listener(|this, _, _, cx| {
                                this.show_add_model_dialog = true;
                                this.add_model_selected_provider = None;
                                this.add_model_selected_model = None;
                                this.add_model_models.clear();
                                this.add_model_error = None;

                                if this.add_model_api_key_input_entity.is_none() {
                                    let theme = this.theme.clone();
                                    this.add_model_api_key_input_entity = Some(cx.new(|cx| {
                                        let mut input =
                                            crate::gui::components::TextInput::new(cx, theme);
                                        input.set_placeholder("Enter API key...");
                                        input
                                    }));
                                }

                                if let Some(input) = &this.add_model_api_key_input_entity {
                                    input.update(cx, |input, cx| input.clear(cx));
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
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Available Models"),
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

    fn render_settings_general(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let user_mode = self.user_mode;

        div().flex().flex_col().gap(px(18.)).child(
            // User Mode
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child("User Mode"),
                )
                .children(
                    [UserMode::Normal, UserMode::Expert, UserMode::Developer]
                        .iter()
                        .map(|mode| {
                            let is_selected = *mode == user_mode;
                            let mode_clone = *mode;
                            let mode_label = match mode {
                                UserMode::Normal => "Normal",
                                UserMode::Expert => "Expert",
                                UserMode::Developer => "Developer",
                            };

                            div()
                                .id(SharedString::from(format!("mode-{:?}", mode)))
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
                                .text_size(px(13.))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.9))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.user_mode = mode_clone;
                                        let settings = Settings::new(&this.db);
                                        if let Err(e) = settings.set_user_mode(mode_clone) {
                                            tracing::warn!("Failed to save user_mode: {}", e);
                                        }

                                        this.available_agents = this
                                            .agents
                                            .list_filtered(mode_clone)
                                            .into_iter()
                                            .map(|info| {
                                                (info.name.clone(), info.display_name.clone())
                                            })
                                            .collect();

                                        let should_switch = !this
                                            .available_agents
                                            .iter()
                                            .any(|(name, _)| name == &this.current_agent);
                                        if should_switch {
                                            if let Some((name, _)) = this.available_agents.first() {
                                                let name = name.clone();
                                                this.set_current_agent(&name);
                                            }
                                        }

                                        cx.notify();
                                    }),
                                )
                                .child(mode_label)
                        }),
                ),
        )
    }

    fn render_oauth_status(
        &self,
        provider: &'static str,
        display_name: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        use crate::auth::TokenStorage;

        let theme = self.theme.clone();
        let storage = TokenStorage::new(&self.db);
        let is_authenticated = storage.is_authenticated(provider).unwrap_or(false);

        div()
            .id(SharedString::from(format!("oauth-{}", provider)))
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(theme.tool_card)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.text)
                            .child(display_name),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(if is_authenticated {
                                rgb(0x4ade80)
                            } else {
                                theme.text_muted
                            })
                            .child(if is_authenticated {
                                "✓ Connected"
                            } else {
                                "Not connected"
                            }),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("oauth-btn-{}", provider)))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(if is_authenticated {
                        theme.background
                    } else {
                        theme.accent
                    })
                    .text_color(if is_authenticated {
                        theme.text
                    } else {
                        rgb(0xffffff)
                    })
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.start_oauth_flow(provider, cx);
                        }),
                    )
                    .child(if is_authenticated {
                        "Reconnect"
                    } else {
                        "Connect"
                    }),
            )
    }

    pub(super) fn render_api_keys_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_api_keys_dialog;

        div().when(show, |d| {
            d.child(
                div()
                    .id("api-keys-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x000000aa))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.show_api_keys_dialog = false;
                            this.api_key_new_name.clear();
                            this.api_key_new_value.clear();
                            cx.notify();
                        }),
                    )
                    .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(450.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .max_h(px(500.))
                        .bg(theme.panel_background)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(12.))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
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
                                        .child("🔑 API Keys"),
                                )
                                .child(
                                    div()
                                        .id("close-api-keys")
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
                                            cx.listener(|this, _, _, cx| {
                                                this.show_api_keys_dialog = false;
                                                this.api_key_new_name.clear();
                                                this.api_key_new_value.clear();
                                                cx.notify();
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                        .child(
                            div()
                                .id("api-keys-content")
                                .flex_1()
                                .overflow_y_scroll().scrollbar_width(px(8.))
                                .p(px(20.))
                                .flex()
                                .flex_col()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child(format!(
                                            "Stored Keys ({})",
                                            self.api_keys_list.len()
                                        )),
                                )
                                .when(self.api_keys_list.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .py(px(8.))
                                            .child("No API keys stored yet."),
                                    )
                                })
                                .children(self.api_keys_list.iter().map(|key_name| {
                                    let name = key_name.clone();
                                    let name_for_delete = key_name.clone();

                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .px(px(12.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(theme.tool_card)
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .text_color(theme.text)
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_muted)
                                                        .child("••••••••"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "delete-key-{}",
                                                    name_for_delete
                                                )))
                                                .px(px(8.))
                                                .py(px(4.))
                                                .rounded(px(4.))
                                                .text_size(px(11.))
                                                .text_color(rgb(0xff6b6b))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(theme.background))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        let _ = this
                                                            .db
                                                            .delete_api_key(&name_for_delete);
                                                        this.refresh_api_keys_list();
                                                        cx.notify();
                                                    }),
                                                )
                                                .child("Delete"),
                                        )
                                }))
                                .child(
                                    div()
                                        .mt(px(8.))
                                        .pt(px(12.))
                                        .border_t_1()
                                        .border_color(theme.border)
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.text_muted)
                                                .child("Add New Key"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.background)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .child(
                                                            div()
                                                                .text_size(px(12.))
                                                                .text_color(if self
                                                                    .api_key_new_name
                                                                    .is_empty()
                                                                {
                                                                    theme.text_muted
                                                                } else {
                                                                    theme.text
                                                                })
                                                                .child(if self
                                                                    .api_key_new_name
                                                                    .is_empty()
                                                                {
                                                                    SharedString::from(
                                                                        "Name (e.g., OPENAI_API_KEY)",
                                                                    )
                                                                } else {
                                                                    SharedString::from(
                                                                        self.api_key_new_name
                                                                            .clone(),
                                                                    )
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id("paste-key-name")
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.tool_card)
                                                        .text_size(px(11.))
                                                        .text_color(theme.text)
                                                        .cursor_pointer()
                                                        .hover(|s| s.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                if let Some(text) = cx
                                                                    .read_from_clipboard()
                                                                    .and_then(|i| i.text())
                                                                {
                                                                    this.api_key_new_name =
                                                                        text.to_string();
                                                                    cx.notify();
                                                                }
                                                            }),
                                                        )
                                                        .child("Paste"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.background)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .child(
                                                            div()
                                                                .text_size(px(12.))
                                                                .text_color(if self
                                                                    .api_key_new_value
                                                                    .is_empty()
                                                                {
                                                                    theme.text_muted
                                                                } else {
                                                                    theme.text
                                                                })
                                                                .child(if self
                                                                    .api_key_new_value
                                                                    .is_empty()
                                                                {
                                                                    SharedString::from(
                                                                        "API Key value",
                                                                    )
                                                                } else {
                                                                    SharedString::from(
                                                                        "•".repeat(
                                                                            self.api_key_new_value
                                                                                .len()
                                                                                .min(20),
                                                                        ),
                                                                    )
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id("paste-key-value")
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.tool_card)
                                                        .text_size(px(11.))
                                                        .text_color(theme.text)
                                                        .cursor_pointer()
                                                        .hover(|s| s.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                if let Some(text) = cx
                                                                    .read_from_clipboard()
                                                                    .and_then(|i| i.text())
                                                                {
                                                                    this.api_key_new_value =
                                                                        text.to_string();
                                                                    cx.notify();
                                                                }
                                                            }),
                                                        )
                                                        .child("Paste"),
                                                ),
                                        )
                                        .when(
                                            !self.api_key_new_name.is_empty()
                                                && !self.api_key_new_value.is_empty(),
                                            |d| {
                                                d.child(
                                                    div()
                                                        .id("save-new-key")
                                                        .px(px(12.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.accent)
                                                        .text_color(rgb(0xffffff))
                                                        .text_size(px(12.))
                                                        .cursor_pointer()
                                                        .hover(|s| s.opacity(0.9))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                let name = this
                                                                    .api_key_new_name
                                                                    .clone();
                                                                let value = this
                                                                    .api_key_new_value
                                                                    .clone();
                                                                let _ = this
                                                                    .db
                                                                    .save_api_key(&name, &value);
                                                                this.api_key_new_name.clear();
                                                                this.api_key_new_value.clear();
                                                                this.refresh_api_keys_list();
                                                                cx.notify();
                                                            }),
                                                        )
                                                        .child("Save Key"),
                                                )
                                            },
                                        ),
                        ),
                        ),
                )
            )
        })
    }

    pub(super) fn render_add_model_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
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
                                            cx.listener(|this, _, _, cx| {
                                                this.show_add_model_dialog = false;
                                                this.add_model_selected_provider = None;
                                                this.add_model_selected_model = None;
                                                this.add_model_models.clear();
                                                if let Some(input) =
                                                    &this.add_model_api_key_input_entity
                                                {
                                                    input.update(cx, |input, cx| input.clear(cx));
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
                                cx.listener(move |this, _, _, cx| {
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
                                        input.update(cx, |input, cx| input.clear(cx));
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
            .map(|e| !e.read(cx).content().is_empty())
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
                                self.add_model_api_key_input_entity.clone(),
                                |d, input| d.child(input),
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
                                        cx.listener(|this, _, _, cx| {
                                            if let Some(text) =
                                                cx.read_from_clipboard().and_then(|i| i.text())
                                            {
                                                if let Some(input) =
                                                    &this.add_model_api_key_input_entity
                                                {
                                                    input.update(cx, |input, cx| {
                                                        input.set_content(text.to_string(), cx);
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
