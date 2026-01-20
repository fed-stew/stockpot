//! General settings tab
//!
//! Contains PDF processing mode, user mode, reasoning display, and YOLO mode settings.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::agents::UserMode;
use crate::config::{PdfMode, Settings};
use crate::tui::app::TuiApp;
use crate::tui::theme::Theme;

/// Render the General settings tab content
pub fn render_general_tab(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let settings = Settings::new(&app.db);
    let selected_index = app.settings_state.selected_index;

    // Load current values
    let pdf_mode = settings.pdf_mode();
    let user_mode = settings.user_mode();
    let show_reasoning = settings.get_bool("show_reasoning").unwrap_or(true);
    let yolo_mode = settings.yolo_mode();

    // Track which selectable item we're on
    let mut selectable_index = 0;

    // Calculate layout - we need room for all sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // PDF Processing Mode
            Constraint::Length(1),  // Spacer
            Constraint::Length(5),  // User Mode
            Constraint::Length(1),  // Spacer
            Constraint::Length(3),  // Show Reasoning
            Constraint::Length(1),  // Spacer
            Constraint::Length(4),  // YOLO Mode
            Constraint::Min(0),     // Remaining space
        ])
        .split(area);

    // ─────────────────────────────────────────────────────────────────────────
    // PDF Processing Mode
    // ─────────────────────────────────────────────────────────────────────────
    let pdf_area = chunks[0];
    let pdf_selected = selected_index == selectable_index;
    render_pdf_mode_section(frame, pdf_area, pdf_mode, pdf_selected);
    selectable_index += 1;

    // ─────────────────────────────────────────────────────────────────────────
    // User Mode
    // ─────────────────────────────────────────────────────────────────────────
    let user_mode_area = chunks[2];
    let user_mode_selected = selected_index == selectable_index;
    render_user_mode_section(frame, user_mode_area, user_mode, user_mode_selected);
    selectable_index += 1;

    // ─────────────────────────────────────────────────────────────────────────
    // Show Agent Reasoning
    // ─────────────────────────────────────────────────────────────────────────
    let reasoning_area = chunks[4];
    let reasoning_selected = selected_index == selectable_index;
    render_reasoning_toggle(frame, reasoning_area, show_reasoning, reasoning_selected);
    selectable_index += 1;

    // ─────────────────────────────────────────────────────────────────────────
    // YOLO Mode
    // ─────────────────────────────────────────────────────────────────────────
    let yolo_area = chunks[6];
    let yolo_selected = selected_index == selectable_index;
    render_yolo_mode_toggle(frame, yolo_area, yolo_mode, yolo_selected);
    // selectable_index += 1; // Uncomment when adding more items
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
