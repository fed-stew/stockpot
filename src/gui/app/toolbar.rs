use gpui::{div, prelude::*, px, rgb, Context, MouseButton, Styled};
use rfd::FileDialog;

use super::{ChatApp, NewConversation};

impl ChatApp {
    pub(super) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let agent_display = self.current_agent_display();
        let (effective_model, _is_pinned) = self.current_effective_model();
        let model_display = Self::truncate_model_name(&effective_model);
        let agent_chevron = if self.show_agent_dropdown {
            "▴"
        } else {
            "▾"
        };
        let model_chevron = if self.show_model_dropdown {
            "▴"
        } else {
            "▾"
        };

        let view = cx.entity().clone();

        let agent_bounds_tracker = {
            let view = view.clone();
            gpui::canvas(
                move |bounds, _window, cx| {
                    let should_update = view.read(cx).agent_dropdown_bounds != Some(bounds);
                    if should_update {
                        view.update(cx, |this, _| {
                            this.agent_dropdown_bounds = Some(bounds);
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        let model_bounds_tracker = {
            let view = view.clone();
            gpui::canvas(
                move |bounds, _window, cx| {
                    let should_update = view.read(cx).model_dropdown_bounds != Some(bounds);
                    if should_update {
                        view.update(cx, |this, _| {
                            this.model_dropdown_bounds = Some(bounds);
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        let cwd_display = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".to_string());

        div()
            .flex()
            .flex_col()
            .px(px(16.))
            .pt(px(10.))
            .pb(px(10.))
            .border_b_1()
            .border_color(self.theme.border)
            .bg(self.theme.panel_background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        // Left side - branding and selectors
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            // Logo
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(div().text_size(px(18.)).child("🍲"))
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(self.theme.text)
                                            .child("Stockpot"),
                                    ),
                            )
                            // Agent selector
                            .child(
                                div()
                                    .id("agent-selector")
                                    .min_w(px(180.))
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded(px(6.))
                                    .bg(self.theme.tool_card)
                                    .text_color(self.theme.text)
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .relative()
                                    .hover(|s| s.opacity(0.8))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.show_agent_dropdown = !this.show_agent_dropdown;
                                            if this.show_agent_dropdown {
                                                this.show_model_dropdown = false;
                                                this.show_settings = false;
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(agent_bounds_tracker)
                                    .child(format!("{} {}", agent_display, agent_chevron)),
                            )
                            // Model selector
                            .child(
                                div()
                                    .id("model-selector")
                                    .min_w(px(220.))
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded(px(6.))
                                    .bg(self.theme.tool_card)
                                    .text_color(self.theme.text)
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .relative()
                                    .hover(|s| s.opacity(0.8))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.show_model_dropdown = !this.show_model_dropdown;
                                            if this.show_model_dropdown {
                                                this.show_agent_dropdown = false;
                                                this.show_settings = false;
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(model_bounds_tracker)
                                    .child(format!("📦 {} {}", model_display, model_chevron)),
                            ),
                    )
                    .child(
                        // Right side - actions
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            // MCP status
                            .child(
                                div()
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded(px(6.))
                                    .bg(self.theme.tool_card)
                                    .text_color(self.theme.text_muted)
                                    .text_size(px(12.))
                                    .child("🔌 MCP"),
                            )
                            // New conversation
                            .child(
                                div()
                                    .id("new-btn")
                                    .px(px(12.))
                                    .py(px(6.))
                                    .rounded(px(6.))
                                    .bg(self.theme.accent)
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.9))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.new_conversation(&NewConversation, window, cx);
                                        }),
                                    )
                                    .child("+ New"),
                            )
                            // Settings
                            .child(
                                div()
                                    .id("settings-btn")
                                    .px(px(12.))
                                    .py(px(6.))
                                    .rounded(px(6.))
                                    .bg(self.theme.tool_card)
                                    .text_color(self.theme.text)
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.8))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.show_agent_dropdown = false;
                                            this.show_model_dropdown = false;
                                            this.show_settings = !this.show_settings;
                                            if this.show_settings {
                                                this.settings_tab =
                                                    super::settings::SettingsTab::PinnedAgents;
                                                this.settings_selected_agent =
                                                    this.current_agent.clone();
                                                // Refresh models to pick up any OAuth or API key changes
                                                this.refresh_models();
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child("⚙"),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(11.))
                    .text_color(self.theme.text_muted)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            let Some(folder) = FileDialog::new().pick_folder() else {
                                return;
                            };

                            if let Err(e) = std::env::set_current_dir(&folder) {
                                this.error_message =
                                    Some(format!("Failed to change directory: {}", e));
                                cx.notify();
                                return;
                            }

                            this.error_message = None;
                            this.show_agent_dropdown = false;
                            this.show_model_dropdown = false;
                            this.show_settings = false;
                            cx.notify();
                        }),
                    )
                    .child(format!("📁 {}", cwd_display)),
            )
    }
}
