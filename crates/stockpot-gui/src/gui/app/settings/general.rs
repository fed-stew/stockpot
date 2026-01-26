//! General settings tab
//!
//! Contains PDF processing mode, user mode, reasoning display, YOLO mode,
//! and context compression settings.
//!
//! Refactored to use the new settings components for a clean, consistent UI.

use gpui::{div, prelude::*, px, Context, Styled};

use crate::gui::app::ChatApp;
use crate::gui::components::{segmented_control, settings_row, settings_row_no_border, toggle};
use stockpot_core::agents::UserMode;
use stockpot_core::config::{PdfMode, Settings};

impl ChatApp {
    pub(crate) fn render_settings_general(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let settings = Settings::new(&self.db);
        let entity = cx.entity().clone();

        // Read current settings values
        let pdf_mode = self.pdf_mode;
        let user_mode = self.user_mode;
        let show_reasoning = self.show_reasoning;
        let yolo_enabled = settings.yolo_mode();
        let compression_enabled = settings.get_compression_enabled();
        let compression_strategy = settings.get_compression_strategy();
        let compression_threshold = settings.get_compression_threshold();
        let compression_target = settings.get_compression_target_tokens();

        div()
            .flex()
            .flex_col()
            .gap(px(0.)) // No gap - borders handle separation
            // =========================================================================
            // PDF Processing Mode (Segmented Control - same pattern as User Mode)
            // =========================================================================
            .child({
                let entity = entity.clone();
                settings_row(
                    "PDF Processing Mode",
                    Some("📷 Image: best for diagrams, charts, scans\n📝 Text: faster, uses fewer tokens"),
                    None,
                    false,
                    &theme,
                    segmented_control(
                        "pdf-mode",
                        vec![
                            (PdfMode::Image, "Image"),
                            (PdfMode::TextExtract, "Text"),
                        ],
                        pdf_mode,
                        &theme,
                        move |mode: PdfMode, _window, cx| {
                            entity.update(cx, |this, cx| {
                                this.pdf_mode = mode;
                                let settings = Settings::new(&this.db);
                                if let Err(e) = settings.set_pdf_mode(mode) {
                                    tracing::warn!("Failed to save pdf_mode: {}", e);
                                }
                                cx.notify();
                            });
                        },
                    ),
                )
            })
            // =========================================================================
            // User Mode (Segmented Control in Settings Row)
            // =========================================================================
            .child({
                let entity = entity.clone();
                settings_row(
                    "User Mode",
                    Some("Controls which features and agents are visible"),
                    None,
                    false,
                    &theme,
                    segmented_control(
                        "user-mode",
                        vec![
                            (UserMode::Normal, "Normal"),
                            (UserMode::Expert, "Expert"),
                            (UserMode::Developer, "Developer"),
                        ],
                        user_mode,
                        &theme,
                        move |mode: UserMode, _window, cx| {
                            entity.update(cx, |this, cx| {
                                this.user_mode = mode;
                                let settings = Settings::new(&this.db);
                                if let Err(e) = settings.set_user_mode(mode) {
                                    tracing::warn!("Failed to save user_mode: {}", e);
                                }

                                // Update available agents based on new mode
                                this.available_agents = this
                                    .agents
                                    .list_filtered(mode)
                                    .into_iter()
                                    .map(|info| (info.name.clone(), info.display_name.clone()))
                                    .collect();

                                // Switch to a valid agent if current one is no longer available
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
                            });
                        },
                    ),
                )
            })
            // =========================================================================
            // Show Agent Reasoning (Toggle)
            // =========================================================================
            .child({
                let entity = entity.clone();
                settings_row(
                    "Show Agent Reasoning",
                    Some("Display the AI's thought process and planned steps"),
                    Some("🧠"),
                    false,
                    &theme,
                    toggle(
                        "show-reasoning",
                        show_reasoning,
                        &theme,
                        move |_window, cx| {
                            entity.update(cx, |this, cx| {
                                this.show_reasoning = !this.show_reasoning;
                                let settings = Settings::new(&this.db);
                                let value = if this.show_reasoning { "true" } else { "false" };
                                if let Err(e) = settings.set("show_reasoning", value) {
                                    tracing::warn!("Failed to save show_reasoning: {}", e);
                                }
                                cx.notify();
                            });
                        },
                    ),
                )
            })
            // =========================================================================
            // YOLO Mode (Toggle with Warning)
            // =========================================================================
            .child({
                let entity = entity.clone();
                settings_row(
                    "YOLO Mode",
                    Some("Auto-accept shell commands without confirmation. High-risk commands (sudo, rm -rf, etc.) still require approval."),
                    None, // No icon - the warning indicator is enough
                    true, // Warning indicator
                    &theme,
                    toggle(
                        "yolo-mode",
                        yolo_enabled,
                        &theme,
                        move |_window, cx| {
                            entity.update(cx, |this, cx| {
                                let settings = Settings::new(&this.db);
                                let current = settings.yolo_mode();
                                if let Err(e) = settings.set_yolo_mode(!current) {
                                    tracing::warn!("Failed to save yolo_mode: {}", e);
                                }
                                cx.notify();
                            });
                        },
                    ),
                )
            })
            // =========================================================================
            // Context Compression (Toggle + Conditional Sub-settings)
            // =========================================================================
            .child({
                let entity_compression = entity.clone();
                let entity_strategy = entity.clone();
                let entity_threshold = entity.clone();
                let entity_target = entity.clone();

                div()
                    .flex()
                    .flex_col()
                    // Main compression toggle row
                    .child(settings_row_no_border(
                        "Context Compression",
                        Some("Automatically compress conversation history when context window fills up"),
                        Some("📦"),
                        false,
                        &theme,
                        toggle(
                            "compression-enabled",
                            compression_enabled,
                            &theme,
                            move |_window, cx| {
                                entity_compression.update(cx, |this, cx| {
                                    let settings = Settings::new(&this.db);
                                    let current = settings.get_compression_enabled();
                                    settings.set_compression_enabled(!current);
                                    cx.notify();
                                });
                            },
                        ),
                    ))
                    // Sub-settings (only when compression is enabled)
                    .when(compression_enabled, |container| {
                        container.child(
                            div()
                                .pl(px(32.)) // Indent sub-settings
                                .flex()
                                .flex_col()
                                .border_b_1()
                                .border_color(theme.border)
                                // Strategy row
                                .child({
                                    let entity_strategy = entity_strategy.clone();
                                    settings_row(
                                        "Compression Strategy",
                                        Some("✂️ Truncate: fast, may lose context\n📝 Summarize: slower, preserves context"),
                                        None,
                                        false,
                                        &theme,
                                        segmented_control(
                                            "compression-strategy",
                                            vec![
                                                ("truncate", "Truncate"),
                                                ("summarize", "Summarize"),
                                            ],
                                            if compression_strategy == "summarize" {
                                                "summarize"
                                            } else {
                                                "truncate"
                                            },
                                            &theme,
                                            move |strategy: &'static str, _window, cx| {
                                                entity_strategy.update(cx, |this, cx| {
                                                    let settings = Settings::new(&this.db);
                                                    settings.set_compression_strategy(strategy);
                                                    cx.notify();
                                                });
                                            },
                                        ),
                                    )
                                })
                                // Threshold row
                                .child({
                                    let entity_threshold = entity_threshold.clone();
                                    settings_row(
                                        "Trigger Threshold",
                                        Some("Compress when context usage exceeds this level"),
                                        None,
                                        false,
                                        &theme,
                                        segmented_control(
                                            "compression-threshold",
                                            vec![
                                                (50, "50%"),
                                                (65, "65%"),
                                                (75, "75%"),
                                                (85, "85%"),
                                                (95, "95%"),
                                            ],
                                            ((compression_threshold * 100.0).round() as i32)
                                                .clamp(50, 95),
                                            &theme,
                                            move |threshold_pct: i32, _window, cx| {
                                                entity_threshold.update(cx, |this, cx| {
                                                    let settings = Settings::new(&this.db);
                                                    settings.set_compression_threshold(
                                                        threshold_pct as f64 / 100.0,
                                                    );
                                                    cx.notify();
                                                });
                                            },
                                        ),
                                    )
                                })
                                // Target Tokens row (last item, no border)
                                .child({
                                    let entity_target = entity_target.clone();
                                    settings_row_no_border(
                                        "Target Tokens",
                                        Some("Token count to compress down to"),
                                        None,
                                        false,
                                        &theme,
                                        segmented_control(
                                            "compression-target",
                                            vec![
                                                (10_000_usize, "10k"),
                                                (20_000_usize, "20k"),
                                                (30_000_usize, "30k"),
                                                (50_000_usize, "50k"),
                                                (75_000_usize, "75k"),
                                            ],
                                            compression_target,
                                            &theme,
                                            move |tokens: usize, _window, cx| {
                                                entity_target.update(cx, |this, cx| {
                                                    let settings = Settings::new(&this.db);
                                                    settings.set_compression_target_tokens(tokens);
                                                    cx.notify();
                                                });
                                            },
                                        ),
                                    )
                                }),
                        )
                    })
            })
    }
}
