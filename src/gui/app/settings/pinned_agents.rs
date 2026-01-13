//! Pinned agents settings tab
//!
//! Allows users to pin specific models to individual agents.

use gpui::{
    anchored, deferred, div, prelude::*, px, rgb, Context, MouseButton, SharedString, Styled,
};

use crate::config::Settings;
use crate::gui::app::ChatApp;

impl ChatApp {
    pub(crate) fn render_settings_pinned_agents(&self, cx: &Context<Self>) -> impl IntoElement {
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
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
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
                                                .text_size(px(13.))
                                                .overflow_hidden()
                                                .cursor_pointer()
                                                .hover(|s| s.bg(theme.tool_card))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.current_model = model_name.clone();
                                                        let settings = Settings::new(&this.db);
                                                        let _ = settings.set("model", &model_name);
                                                        this.update_context_usage();
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
            .flex_1()
            .min_h(px(0.))
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
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
                    .flex_1()
                    .min_h(px(0.))
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
            .flex_1()
            .min_h(px(0.))
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
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
                    .flex_1()
                    .min_h(px(0.))
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
                            .text_size(px(13.))
                            .overflow_hidden()
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
                            .text_size(px(13.))
                            .overflow_hidden()
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

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .child(default_model_section)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(18.))
                    .child(div().w(px(260.)).flex().flex_col().child(left_panel))
                    .child(div().flex_1().flex().flex_col().child(right_panel)),
            )
    }
}
