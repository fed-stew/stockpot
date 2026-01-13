use gpui::{div, prelude::*, px, rgb, Context, MouseButton, Styled};
use rfd::AsyncFileDialog;

use super::super::components::{throughput_chart, ThroughputChartProps};
use super::{ChatApp, NewConversation};
use crate::tokens::format_tokens_with_separator;

impl ChatApp {
    pub(super) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let agent_display = self.current_agent_display();
        let (effective_model, _is_pinned) = self.current_effective_model();
        let model_display_short = if _is_pinned {
            Self::truncate_model_name(&effective_model)
        } else {
            "default".to_string()
        };
        let agent_chevron = if self.show_agent_dropdown {
            "▴"
        } else {
            "▾"
        };

        // Context usage calculation
        let usage_percent = if self.context_window_size > 0 {
            ((self.context_tokens_used as f64 / self.context_window_size as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        // Color based on usage threshold
        let (progress_color, label_color) = if usage_percent > 80.0 {
            (self.theme.error, self.theme.error)
        } else if usage_percent > 60.0 {
            (self.theme.warning, self.theme.warning)
        } else {
            (self.theme.accent, self.theme.text_muted)
        };

        // Capture for tooltip closure
        let context_tokens_used = self.context_tokens_used;
        let context_window_size = self.context_window_size;
        let current_throughput_cps = self.current_throughput_cps;
        let throughput_history: Vec<f64> = self.throughput_history.iter().cloned().collect();
        let is_streaming_active = self.is_streaming_active;
        let theme_accent = self.theme.accent;
        let theme_success = self.theme.success;
        let theme_border = self.theme.border;
        let theme_text_muted = self.theme.text_muted;

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
                                    .min_w(px(220.))
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(gpui::rgba(0x00000000))
                                    .text_color(self.theme.text)
                                    .text_size(px(14.))
                                    .cursor_pointer()
                                    .relative()
                                    .hover(|s| s.border_color(self.theme.border))
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
                                    .child(format!(
                                        "{} • {} {}",
                                        agent_display, model_display_short, agent_chevron
                                    )),
                            )
                            // Context usage indicator
                            .child(
                                div()
                                    .id("context-usage")
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded(px(6.))
                                    .tooltip(move |_window, cx| {
                                        cx.new(|_| {
                                            super::super::components::SimpleTooltip::new(format!(
                                                "~{} / {} tokens",
                                                context_tokens_used, context_window_size
                                            ))
                                        })
                                        .into()
                                    })
                                    // Label
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(self.theme.text_muted)
                                            .child("Context:"),
                                    )
                                    // Progress bar container
                                    .child(
                                        div()
                                            .w(px(60.))
                                            .h(px(4.))
                                            .bg(self.theme.border)
                                            .rounded(px(2.))
                                            .overflow_hidden()
                                            .child(
                                                // Progress bar fill
                                                div()
                                                    .h_full()
                                                    .w(gpui::relative(usage_percent as f32 / 100.0))
                                                    .bg(progress_color)
                                                    .rounded(px(2.)),
                                            ),
                                    )
                                    // Token count text - fixed width to prevent layout shift
                                    .child(
                                        div()
                                            .min_w(px(95.)) // Fits "999 999/999 999"
                                            .text_size(px(11.))
                                            .text_color(label_color)
                                            .child(format!(
                                                "{}/{}",
                                                format_tokens_with_separator(context_tokens_used),
                                                format_tokens_with_separator(context_window_size)
                                            )),
                                    ),
                            )
                            // Throughput chart
                            .child(
                                div()
                                    .id("throughput-chart")
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .tooltip(move |_window, cx| {
                                        let cps_display = if current_throughput_cps > 0.0 {
                                            format!("{:.0} chars/sec", current_throughput_cps)
                                        } else {
                                            "Idle".to_string()
                                        };
                                        cx.new(|_| {
                                            super::super::components::SimpleTooltip::new(
                                                cps_display,
                                            )
                                        })
                                        .into()
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(theme_text_muted)
                                            .child("Speed:"),
                                    )
                                    .child(throughput_chart(ThroughputChartProps {
                                        samples: throughput_history,
                                        current_cps: current_throughput_cps,
                                        is_active: is_streaming_active,
                                        max_cps: 500.0,
                                        bar_color: theme_accent,
                                        bar_color_fast: theme_success,
                                        background_color: theme_border,
                                    })),
                            ),
                    )
                    .child(
                        // Right side - actions
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
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
                        cx.listener(|_this, _, _, cx| {
                            // Spawn async task to open folder dialog without blocking UI
                            cx.spawn(async move |this, cx| {
                                let folder = AsyncFileDialog::new().pick_folder().await;

                                let Some(folder) = folder else {
                                    return;
                                };

                                let folder_path = folder.path().to_path_buf();

                                let _ = this.update(cx, |this, cx| {
                                    if let Err(e) = std::env::set_current_dir(&folder_path) {
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
                                });
                            })
                            .detach();
                        }),
                    )
                    .child(format!("📁 {}", cwd_display)),
            )
    }
}
