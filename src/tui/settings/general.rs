//! General settings tab
//!
//! Contains PDF processing mode, user mode, reasoning display, YOLO mode,
//! and context compression settings.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::agents::UserMode;
use crate::config::{PdfMode, Settings};
use crate::tokens::format_tokens_with_separator;
use crate::tui::app::TuiApp;
use crate::tui::hit_test::{ClickTarget, HitTestRegistry};
use crate::tui::theme::Theme;

/// Render the General settings tab content
pub fn render_general_tab(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    hit_registry: &mut HitTestRegistry,
) {
    let settings = Settings::new(&app.db);
    let selected_index = app.settings_state.selected_index;

    // Load current values
    let pdf_mode = settings.pdf_mode();
    let user_mode = settings.user_mode();
    let show_reasoning = settings.get_bool("show_reasoning").unwrap_or(true);
    let yolo_mode = settings.yolo_mode();
    let compression_enabled = settings.get_compression_enabled();
    let compression_strategy = settings.get_compression_strategy();
    let compression_threshold = settings.get_compression_threshold();
    let compression_target = settings.get_compression_target_tokens();

    // Track which selectable item we're on
    let mut selectable_index = 0;

    // Calculate layout - we need room for all sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // PDF Processing Mode
            Constraint::Length(1), // Spacer
            Constraint::Length(5), // User Mode
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Show Reasoning
            Constraint::Length(1), // Spacer
            Constraint::Length(4), // YOLO Mode
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Compression Toggle
            Constraint::Length(3), // Compression Strategy (conditional)
            Constraint::Length(3), // Compression Threshold (conditional)
            Constraint::Length(3), // Compression Target (conditional)
            Constraint::Min(0),    // Remaining space
        ])
        .split(area);

    // ─────────────────────────────────────────────────────────────────────────
    // PDF Processing Mode (Radio buttons)
    // ─────────────────────────────────────────────────────────────────────────
    let pdf_area = chunks[0];
    let pdf_selected = selected_index == selectable_index;
    render_pdf_mode_section(frame, pdf_area, pdf_mode, pdf_selected);
    selectable_index += 1;

    // Register hit targets for PDF mode radio buttons
    // Line 2 (y+2): "    ◉ 📷 Image Mode" - Image option
    // Line 4 (y+4): "    ○ 📝 Text Mode" - Text option
    hit_registry.register(
        Rect::new(pdf_area.x + 4, pdf_area.y + 2, 20, 1),
        ClickTarget::SettingsRadio("pdf_mode".to_string(), 0), // Image
    );
    hit_registry.register(
        Rect::new(pdf_area.x + 4, pdf_area.y + 4, 18, 1),
        ClickTarget::SettingsRadio("pdf_mode".to_string(), 1), // Text
    );

    // ─────────────────────────────────────────────────────────────────────────
    // User Mode (Radio buttons - horizontal layout)
    // ─────────────────────────────────────────────────────────────────────────
    let user_mode_area = chunks[2];
    let user_mode_selected = selected_index == selectable_index;
    render_user_mode_section(frame, user_mode_area, user_mode, user_mode_selected);
    selectable_index += 1;

    // Register hit targets for User Mode radio buttons (all on line 3, y+3)
    // Layout: "    ◉ Normal   ○ Expert   ○ Developer"
    //          ^4   ^5      ^15        ^26
    hit_registry.register(
        Rect::new(user_mode_area.x + 4, user_mode_area.y + 3, 9, 1),
        ClickTarget::SettingsRadio("user_mode".to_string(), 0), // Normal
    );
    hit_registry.register(
        Rect::new(user_mode_area.x + 15, user_mode_area.y + 3, 9, 1),
        ClickTarget::SettingsRadio("user_mode".to_string(), 1), // Expert
    );
    hit_registry.register(
        Rect::new(user_mode_area.x + 26, user_mode_area.y + 3, 12, 1),
        ClickTarget::SettingsRadio("user_mode".to_string(), 2), // Developer
    );

    // ─────────────────────────────────────────────────────────────────────────
    // Show Agent Reasoning (Toggle)
    // ─────────────────────────────────────────────────────────────────────────
    let reasoning_area = chunks[4];
    let reasoning_selected = selected_index == selectable_index;
    render_reasoning_toggle(frame, reasoning_area, show_reasoning, reasoning_selected);
    selectable_index += 1;

    // Register hit target for the entire reasoning toggle row
    hit_registry.register(
        Rect::new(reasoning_area.x, reasoning_area.y, reasoning_area.width, 1),
        ClickTarget::SettingsToggle("show_reasoning".to_string()),
    );

    // ─────────────────────────────────────────────────────────────────────────
    // YOLO Mode (Toggle)
    // ─────────────────────────────────────────────────────────────────────────
    let yolo_area = chunks[6];
    let yolo_selected = selected_index == selectable_index;
    render_yolo_mode_toggle(frame, yolo_area, yolo_mode, yolo_selected);
    selectable_index += 1;

    // Register hit target for the entire YOLO toggle row
    hit_registry.register(
        Rect::new(yolo_area.x, yolo_area.y, yolo_area.width, 1),
        ClickTarget::SettingsToggle("yolo_mode".to_string()),
    );

    // ─────────────────────────────────────────────────────────────────────────
    // Context Compression Toggle
    // ─────────────────────────────────────────────────────────────────────────
    let compression_toggle_area = chunks[8];
    let compression_toggle_selected = selected_index == selectable_index;
    render_compression_toggle(
        frame,
        compression_toggle_area,
        compression_enabled,
        compression_toggle_selected,
    );
    selectable_index += 1;

    // Register hit target for compression toggle
    hit_registry.register(
        Rect::new(
            compression_toggle_area.x,
            compression_toggle_area.y,
            compression_toggle_area.width,
            1,
        ),
        ClickTarget::SettingsToggle("compression.enabled".to_string()),
    );

    // Only show sub-options if compression is enabled
    if compression_enabled {
        // ─────────────────────────────────────────────────────────────────────
        // Compression Strategy (Radio buttons)
        // ─────────────────────────────────────────────────────────────────────
        let strategy_area = chunks[9];
        let strategy_selected = selected_index == selectable_index;
        render_compression_strategy(
            frame,
            strategy_area,
            &compression_strategy,
            strategy_selected,
        );
        selectable_index += 1;

        // Register hit targets for strategy radio buttons
        // Layout: "    ◉ ✂️ Truncate   ○ 📝 Summarize"
        hit_registry.register(
            Rect::new(strategy_area.x + 4, strategy_area.y + 1, 14, 1),
            ClickTarget::SettingsRadio("compression.strategy".to_string(), 0), // Truncate
        );
        hit_registry.register(
            Rect::new(strategy_area.x + 20, strategy_area.y + 1, 14, 1),
            ClickTarget::SettingsRadio("compression.strategy".to_string(), 1), // Summarize
        );

        // ─────────────────────────────────────────────────────────────────────
        // Compression Threshold (Radio buttons)
        // ─────────────────────────────────────────────────────────────────────
        let threshold_area = chunks[10];
        let threshold_selected = selected_index == selectable_index;
        render_compression_threshold(
            frame,
            threshold_area,
            compression_threshold,
            threshold_selected,
        );
        selectable_index += 1;

        // Register hit targets for threshold radio buttons
        // Layout: "    ◉ 50%  ○ 65%  ○ 75%  ○ 85%  ○ 95%"
        let thresholds = [0.50, 0.65, 0.75, 0.85, 0.95];
        for (i, _) in thresholds.iter().enumerate() {
            let x_offset = 4 + (i as u16 * 8); // Each option takes ~8 chars
            hit_registry.register(
                Rect::new(threshold_area.x + x_offset, threshold_area.y + 1, 6, 1),
                ClickTarget::SettingsRadio("compression.threshold".to_string(), i),
            );
        }

        // ─────────────────────────────────────────────────────────────────────
        // Compression Target Tokens (Radio buttons)
        // ─────────────────────────────────────────────────────────────────────
        let target_area = chunks[11];
        let target_selected = selected_index == selectable_index;
        render_compression_target(frame, target_area, compression_target, target_selected);
        // selectable_index += 1; // Uncomment when adding more items

        // Register hit targets for target token radio buttons
        let targets = [10_000usize, 20_000, 30_000, 50_000, 75_000];
        for (i, _) in targets.iter().enumerate() {
            let x_offset = 4 + (i as u16 * 10); // Each option takes ~10 chars
            hit_registry.register(
                Rect::new(target_area.x + x_offset, target_area.y + 1, 8, 1),
                ClickTarget::SettingsRadio("compression.target_tokens".to_string(), i),
            );
        }
    }
}

/// Render PDF Processing Mode section
fn render_pdf_mode_section(frame: &mut Frame, area: Rect, current: PdfMode, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::HEADER)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let image_selected = current == PdfMode::Image;
    let text_selected = current == PdfMode::TextExtract;

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, header_style),
            Span::styled("PDF Processing Mode", header_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("    "),
            render_radio(image_selected, is_selected),
            Span::styled(" 📷 Image Mode", option_style(image_selected, is_selected)),
        ]),
        Line::from(Span::styled(
            "         Convert pages to images (best for diagrams, charts, scans)",
            Style::default().fg(Theme::MUTED),
        )),
        Line::from(vec![
            Span::raw("    "),
            render_radio(text_selected, is_selected),
            Span::styled(" 📝 Text Mode", option_style(text_selected, is_selected)),
        ]),
        Line::from(Span::styled(
            "         Extract text content (faster, uses fewer tokens)",
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render User Mode section
fn render_user_mode_section(frame: &mut Frame, area: Rect, current: UserMode, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::HEADER)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let normal = current == UserMode::Normal;
    let expert = current == UserMode::Expert;
    let developer = current == UserMode::Developer;

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, header_style),
            Span::styled("User Mode", header_style),
        ]),
        Line::from(Span::styled(
            "    Controls which agents are visible in the agent selector",
            Style::default().fg(Theme::MUTED),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("    "),
            render_radio(normal, is_selected),
            Span::styled(" Normal", option_style(normal, is_selected)),
            Span::styled("   ", Style::default()),
            render_radio(expert, is_selected),
            Span::styled(" Expert", option_style(expert, is_selected)),
            Span::styled("   ", Style::default()),
            render_radio(developer, is_selected),
            Span::styled(" Developer", option_style(developer, is_selected)),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render Show Reasoning toggle
fn render_reasoning_toggle(frame: &mut Frame, area: Rect, enabled: bool, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::HEADER)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, header_style),
            Span::styled("Show Agent Reasoning", header_style),
            Span::raw("  "),
            render_toggle(enabled, is_selected),
        ]),
        Line::from(Span::styled(
            "    Display the AI's thought process and planned steps",
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render YOLO Mode toggle (with special warning styling)
fn render_yolo_mode_toggle(frame: &mut Frame, area: Rect, enabled: bool, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else if enabled {
        Style::default()
            .fg(Theme::YELLOW)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::HEADER)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let status_style = if enabled {
        Style::default().fg(Theme::YELLOW)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, header_style),
            Span::styled("⚡ YOLO Mode", header_style),
            Span::raw("  "),
            render_toggle_yolo(enabled, is_selected),
        ]),
        Line::from(Span::styled(
            "    Auto-accept shell commands without confirmation",
            status_style,
        )),
        Line::from(Span::styled(
            "    ⚠ High-risk commands still require approval",
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions for rendering controls
// ─────────────────────────────────────────────────────────────────────────────

/// Render a radio button
fn render_radio(selected: bool, parent_focused: bool) -> Span<'static> {
    let style = if parent_focused {
        Style::default().fg(Theme::ACCENT)
    } else if selected {
        Style::default().fg(Theme::GREEN)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    if selected {
        Span::styled("◉", style)
    } else {
        Span::styled("○", style)
    }
}

/// Get style for an option based on selection state
fn option_style(is_current: bool, parent_focused: bool) -> Style {
    if parent_focused && is_current {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else if is_current {
        Style::default().fg(Theme::GREEN)
    } else {
        Style::default().fg(Theme::TEXT)
    }
}

/// Render a toggle switch
fn render_toggle(enabled: bool, is_focused: bool) -> Span<'static> {
    let style = if is_focused {
        Style::default().fg(Theme::ACCENT)
    } else if enabled {
        Style::default().fg(Theme::GREEN)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    if enabled {
        Span::styled("[✓] Enabled", style)
    } else {
        Span::styled("[ ] Disabled", style)
    }
}

/// Render a YOLO toggle switch (with warning colors)
fn render_toggle_yolo(enabled: bool, is_focused: bool) -> Span<'static> {
    let style = if is_focused {
        Style::default().fg(Theme::ACCENT)
    } else if enabled {
        Style::default()
            .fg(Theme::YELLOW)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    if enabled {
        Span::styled("[⚡] ON", style)
    } else {
        Span::styled("[ ] OFF", style)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Context Compression Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Render Context Compression toggle
fn render_compression_toggle(frame: &mut Frame, area: Rect, enabled: bool, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::HEADER)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, header_style),
            Span::styled("📦 Context Compression", header_style),
            Span::raw("  "),
            render_toggle(enabled, is_selected),
        ]),
        Line::from(Span::styled(
            "    Automatically compress conversation history when context fills up",
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render Compression Strategy section
fn render_compression_strategy(frame: &mut Frame, area: Rect, current: &str, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let truncate_selected = current == "truncate";
    let summarize_selected = current == "summarize";

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, header_style),
            Span::styled("Strategy", header_style),
        ]),
        Line::from(vec![
            Span::raw("    "),
            render_radio(truncate_selected, is_selected),
            Span::styled(" ✂️ Truncate", option_style(truncate_selected, is_selected)),
            Span::raw("   "),
            render_radio(summarize_selected, is_selected),
            Span::styled(
                " 📝 Summarize",
                option_style(summarize_selected, is_selected),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render Compression Threshold section
fn render_compression_threshold(frame: &mut Frame, area: Rect, current: f64, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let thresholds = [0.50, 0.65, 0.75, 0.85, 0.95];

    let spans = vec![
        Span::styled(selector, header_style),
        Span::styled(format!("Threshold: {:.0}%", current * 100.0), header_style),
    ];

    let threshold_line: Vec<Span> = std::iter::once(Span::raw("    "))
        .chain(thresholds.iter().enumerate().flat_map(|(i, &t)| {
            let is_current = (current - t).abs() < 0.01;
            let mut parts = vec![
                render_radio(is_current, is_selected),
                Span::styled(
                    format!(" {:.0}%", t * 100.0),
                    option_style(is_current, is_selected),
                ),
            ];
            if i < thresholds.len() - 1 {
                parts.push(Span::raw("  "));
            }
            parts
        }))
        .collect();

    let lines = vec![Line::from(spans), Line::from(threshold_line)];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render Compression Target Tokens section
fn render_compression_target(frame: &mut Frame, area: Rect, current: usize, is_selected: bool) {
    let header_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    let selector = if is_selected { "▶ " } else { "  " };

    let targets: [usize; 5] = [10_000, 20_000, 30_000, 50_000, 75_000];

    let spans = vec![
        Span::styled(selector, header_style),
        Span::styled(
            format!("Target: {} tokens", format_tokens_with_separator(current)),
            header_style,
        ),
    ];

    let target_line: Vec<Span> = std::iter::once(Span::raw("    "))
        .chain(targets.iter().enumerate().flat_map(|(i, &t)| {
            let is_current = current == t;
            let mut parts = vec![
                render_radio(is_current, is_selected),
                Span::styled(
                    format!(" {}", format_tokens_with_separator(t)),
                    option_style(is_current, is_selected),
                ),
            ];
            if i < targets.len() - 1 {
                parts.push(Span::raw(" "));
            }
            parts
        }))
        .collect();

    let lines = vec![Line::from(spans), Line::from(target_line)];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
