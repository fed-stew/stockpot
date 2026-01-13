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
    }
}
