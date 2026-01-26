//! General settings tab
//!
//! Contains PDF processing mode and user mode settings.

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::agents::UserMode;
use crate::config::{PdfMode, Settings};
use crate::gui::app::ChatApp;

impl ChatApp {
    pub(crate) fn render_settings_general(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let user_mode = self.user_mode;
        let pdf_mode = self.pdf_mode;

        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            // PDF Processing Mode
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("PDF Processing Mode"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .mb(px(4.))
                            .child("Choose how PDFs are sent to the AI model"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child({
                                let is_selected = pdf_mode == PdfMode::Image;
                                div()
                                    .id("pdf-mode-image")
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
                                        cx.listener(|this, _, _, cx| {
                                            this.pdf_mode = PdfMode::Image;
                                            let settings = Settings::new(&this.db);
                                            if let Err(e) = settings.set_pdf_mode(PdfMode::Image) {
                                                tracing::warn!("Failed to save pdf_mode: {}", e);
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .child("📷 Image Mode"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(if is_selected {
                                                        rgba(0xffffffcc)
                                                    } else {
                                                        theme.text_muted
                                                    })
                                                    .child("Convert pages to images (best for diagrams, charts, scans)"),
                                            ),
                                    )
                            })
                            .child({
                                let is_selected = pdf_mode == PdfMode::TextExtract;
                                div()
                                    .id("pdf-mode-text")
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
                                        cx.listener(|this, _, _, cx| {
                                            this.pdf_mode = PdfMode::TextExtract;
                                            let settings = Settings::new(&this.db);
                                            if let Err(e) = settings.set_pdf_mode(PdfMode::TextExtract) {
                                                tracing::warn!("Failed to save pdf_mode: {}", e);
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .child("📝 Text Mode"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(if is_selected {
                                                        rgba(0xffffffcc)
                                                    } else {
                                                        theme.text_muted
                                                    })
                                                    .child("Extract text content (faster, uses fewer tokens)"),
                                            ),
                                    )
                            }),
                    ),
            )
            // User Mode
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
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
            // Show Reasoning Toggle
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Show Agent Reasoning"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .mb(px(4.))
                            .child("Display the AI's thought process and planned steps"),
                    )
                    .child({
                        let is_enabled = self.show_reasoning;
                        div()
                            .id("show-reasoning-toggle")
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(if is_enabled {
                                theme.accent
                            } else {
                                theme.tool_card
                            })
                            .text_color(if is_enabled {
                                rgb(0xffffff)
                            } else {
                                theme.text
                            })
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.show_reasoning = !this.show_reasoning;
                                    let settings = Settings::new(&this.db);
                                    let value = if this.show_reasoning { "true" } else { "false" };
                                    if let Err(e) = settings.set("show_reasoning", value) {
                                        tracing::warn!("Failed to save show_reasoning: {}", e);
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(if is_enabled { "✓ Enabled" } else { "Disabled" })
                    }),
            )
            // YOLO Mode Toggle
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("⚡ YOLO Mode"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .mb(px(4.))
                            .child("Auto-accept shell commands without confirmation. High-risk commands (sudo, rm -rf, etc.) still require approval."),
                    )
                    .child({
                        let is_enabled = Settings::new(&self.db).yolo_mode();
                        div()
                            .id("yolo-mode-toggle")
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(if is_enabled {
                                rgba(0xf59e0bff)  // Warning orange color for YOLO mode
                            } else {
                                theme.tool_card
                            })
                            .text_color(if is_enabled {
                                rgb(0xffffff)
                            } else {
                                theme.text
                            })
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let settings = Settings::new(&this.db);
                                    let current = settings.yolo_mode();
                                    if let Err(e) = settings.set_yolo_mode(!current) {
                                        tracing::warn!("Failed to save yolo_mode: {}", e);
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(if is_enabled { "⚡ YOLO Enabled" } else { "Disabled" })
                    }),
            )
            // Context Compression Settings
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("📦 Context Compression"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .mb(px(4.))
                            .child("Automatically compress conversation history when context window fills up"),
                    )
                    // Enable/Disable Toggle
                    .child({
                        let is_enabled = Settings::new(&self.db).get_compression_enabled();
                        div()
                            .id("compression-toggle")
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(if is_enabled {
                                theme.accent
                            } else {
                                theme.tool_card
                            })
                            .text_color(if is_enabled {
                                rgb(0xffffff)
                            } else {
                                theme.text
                            })
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let settings = Settings::new(&this.db);
                                    let current = settings.get_compression_enabled();
                                    settings.set_compression_enabled(!current);
                                    cx.notify();
                                }),
                            )
                            .child(if is_enabled { "✓ Compression Enabled" } else { "Compression Disabled" })
                    })
                    // Strategy Selection (only show if enabled)
                    .when(Settings::new(&self.db).get_compression_enabled(), |el| {
                        let current_strategy = Settings::new(&self.db).get_compression_strategy();
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .mt(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child("Compression Strategy"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.))
                                        .child({
                                            let is_selected = current_strategy == "truncate";
                                            div()
                                                .id("strategy-truncate")
                                                .px(px(12.))
                                                .py(px(8.))
                                                .rounded(px(6.))
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
                                                    cx.listener(|this, _, _, cx| {
                                                        let settings = Settings::new(&this.db);
                                                        settings.set_compression_strategy("truncate");
                                                        cx.notify();
                                                    }),
                                                )
                                                .child("✂️ Truncate")
                                        })
                                        .child({
                                            let is_selected = current_strategy == "summarize";
                                            div()
                                                .id("strategy-summarize")
                                                .px(px(12.))
                                                .py(px(8.))
                                                .rounded(px(6.))
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
                                                    cx.listener(|this, _, _, cx| {
                                                        let settings = Settings::new(&this.db);
                                                        settings.set_compression_strategy("summarize");
                                                        cx.notify();
                                                    }),
                                                )
                                                .child("📝 Summarize")
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.text_muted)
                                        .child(if current_strategy == "summarize" {
                                            "Uses AI to summarize old messages (slower, preserves context)"
                                        } else {
                                            "Removes oldest messages to free space (fast, may lose context)"
                                        }),
                                ),
                        )
                    })
                    // Threshold Selection (only show if enabled)
                    .when(Settings::new(&self.db).get_compression_enabled(), |el| {
                        let current_threshold = Settings::new(&self.db).get_compression_threshold();
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .mt(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child(format!("Trigger Threshold: {:.0}%", current_threshold * 100.0)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(4.))
                                        .children([0.50, 0.65, 0.75, 0.85, 0.95].iter().map(|threshold| {
                                            let is_selected = (current_threshold - threshold).abs() < 0.01;
                                            let threshold_value = *threshold;
                                            div()
                                                .id(SharedString::from(format!("threshold-{}", (threshold * 100.0) as i32)))
                                                .px(px(8.))
                                                .py(px(6.))
                                                .rounded(px(4.))
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
                                                .text_size(px(11.))
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.9))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        let settings = Settings::new(&this.db);
                                                        settings.set_compression_threshold(threshold_value);
                                                        cx.notify();
                                                    }),
                                                )
                                                .child(format!("{:.0}%", threshold * 100.0))
                                        })),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.text_muted)
                                        .child("Compress when context usage exceeds this threshold"),
                                ),
                        )
                    })
                    // Target Tokens (only show if enabled)
                    .when(Settings::new(&self.db).get_compression_enabled(), |el| {
                        let current_target = Settings::new(&self.db).get_compression_target_tokens();
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .mt(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child(format!("Target Tokens: {}", crate::tokens::format_tokens_with_separator(current_target))),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(4.))
                                        .children([10_000, 20_000, 30_000, 50_000, 75_000].iter().map(|tokens| {
                                            let is_selected = current_target == *tokens;
                                            let token_value = *tokens;
                                            div()
                                                .id(SharedString::from(format!("tokens-{}", tokens)))
                                                .px(px(8.))
                                                .py(px(6.))
                                                .rounded(px(4.))
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
                                                .text_size(px(11.))
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.9))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        let settings = Settings::new(&this.db);
                                                        settings.set_compression_target_tokens(token_value);
                                                        cx.notify();
                                                    }),
                                                )
                                                .child(crate::tokens::format_tokens_with_separator(*tokens))
                                        })),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.text_muted)
                                        .child("Target token count after compression"),
                                ),
                        )
                    }),
            )
    }
}
