//! Main UI rendering
//!
//! Uses rustpuppy-style layout: Header → ActivityFeed → Input → StatusBar

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::app::TuiApp;
use super::layout::AppLayout;
use super::settings::render_settings;
use crate::tui::hit_test::ClickTarget;
use crate::tui::theme::{dim_background, Theme};
use crate::tui::widgets::{ActivityFeed, DropdownWidget, Header, StatusBar, TextSelection};

/// Render the entire UI
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    app.hit_registry.clear();

    // Use rustpuppy-style layout with dynamic input height
    let input_lines = app.input.lines().len();
    let layout = AppLayout::new(frame.area(), input_lines);

    // Cache activity area for mouse calculations
    app.cached_activity_area = layout.activity_area;

    // Update viewport height for scrolling
    app.activity_state.viewport_height = layout.activity_area.height as usize;

    // Fill entire frame with background
    for y in frame.area().y..frame.area().y + frame.area().height {
        for x in frame.area().x..frame.area().x + frame.area().width {
            frame.buffer_mut()[(x, y)].set_bg(Theme::BG);
        }
    }

    // Render components
    render_header(frame, app, layout.header_area);
    render_activity_feed(frame, app, layout.activity_area);
    render_input(frame, app, layout.input_area);
    render_status_bar(frame, app, layout.status_area);

    // Dropdowns overlay (if visible)
    if app.show_agent_dropdown {
        render_agent_dropdown(frame, app, layout.header_area);
    }
    // Note: Model dropdown removed from header - model pinning is done in Settings > Pinned Agents
    if app.show_folder_modal {
        render_folder_modal(frame, app, layout.header_area);
    }

    // Help overlay
    if app.show_help {
        render_help(frame, frame.area());
    }

    // Settings overlay (takes priority over help)
    if app.show_settings {
        render_settings(frame, frame.area(), app);
    }
}

fn render_header(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    use crate::config::Settings;

    // Get agent display name
    let agent_display = app
        .agents
        .list()
        .iter()
        .find(|a| a.name == app.current_agent)
        .map(|a| a.display_name.clone())
        .unwrap_or_else(|| app.current_agent.clone());

    // Get effective model for this agent (pinned or "default")
    let settings = Settings::new(&app.db);
    let model_display = settings
        .get_agent_pinned_model(&app.current_agent)
        .unwrap_or_else(|| "default".to_string());

    let header = Header::new(&agent_display, &model_display, &app.current_working_dir);

    // Calculate hit target positions (before render consumes header)
    let agent_section_width = header.agent_section_width();
    let folder_offset = header.folder_offset();
    let folder_width = header.folder_width();
    let settings_offset = header.settings_offset();
    let settings_width = header.settings_width();

    frame.render_widget(header, area);

    // Register hit targets for dropdowns
    // Combined agent/model dropdown trigger (after "stockpot │ ")
    // Clicking anywhere on "Agent • model ▾" opens the agent dropdown
    app.hit_registry.register(
        Rect::new(area.x + 15, area.y, agent_section_width, 1),
        ClickTarget::AgentDropdown,
    );
    // Folder dropdown trigger
    app.hit_registry.register(
        Rect::new(area.x + folder_offset, area.y, folder_width, 1),
        ClickTarget::FolderDropdown,
    );
    // Settings button trigger - make it more generous
    // Add extra width and allow for emoji width variations (emojis take 2 cells but count as 1 char)
    app.hit_registry.register(
        Rect::new(
            (area.x + settings_offset).saturating_sub(2), // Start a bit earlier
            area.y,
            settings_width + 6, // Extra clickable width
            area.height,        // Use full header height
        ),
        ClickTarget::SettingsButton,
    );
}

fn render_activity_feed(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    // Empty state: welcome screen
    if app.activities.is_empty() {
        render_welcome(frame, area);
        return;
    }

    // Build selection for the widget
    let selection = if app.selection.is_active() {
        let mut sel = TextSelection::default();
        if let (Some((sl, sc)), Some((el, ec))) = (app.selection.start(), app.selection.end()) {
            sel.start_line = sl;
            sel.start_col = sc;
            sel.end_line = el;
            sel.end_col = ec;
            sel.active = true;
        }
        Some(sel)
    } else {
        None
    };

    // Create the activity feed widget
    let mut activity_feed =
        ActivityFeed::new(&app.activities).rendered_lines(&mut app.rendered_lines);

    if let Some(ref sel) = selection {
        activity_feed = activity_feed.selection(sel);
    }

    frame.render_stateful_widget(activity_feed, area, &mut app.activity_state);
}

fn render_welcome(frame: &mut Frame, area: Rect) {
    let welcome = Paragraph::new(vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled("🍲", Style::default())),
        Line::from(""),
        Line::from(Span::styled(
            "Welcome to Stockpot",
            Style::default()
                .fg(Theme::HEADER)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Your AI-powered coding assistant",
            Style::default().fg(Theme::MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Type a message below to get started",
            Style::default().fg(Theme::MUTED),
        )),
        Line::from(Span::styled(
            "Press F1 for help",
            Style::default()
                .fg(Theme::MUTED)
                .add_modifier(Modifier::ITALIC),
        )),
    ])
    .alignment(Alignment::Center)
    .style(Style::default().bg(Theme::BG));

    frame.render_widget(welcome, area);
}

fn render_input(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Fill background for the entire input area
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            frame.buffer_mut()[(x, y)].set_bg(Theme::INPUT_BG);
        }
    }

    // Render prompt character "› " on the left
    let prompt = Span::styled("› ", Style::default().fg(Theme::ACCENT));
    let prompt_width = 2u16;

    // Render prompt on first line of input area
    frame
        .buffer_mut()
        .set_span(area.x, area.y, &prompt, prompt_width);

    // Textarea gets the remaining space after the prompt
    let textarea_area = Rect {
        x: area.x + prompt_width,
        y: area.y,
        width: area.width.saturating_sub(prompt_width),
        height: area.height,
    };

    frame.render_widget(&app.input, textarea_area);
}

fn render_status_bar(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Check for recent copy feedback (expires after 2 seconds)
    let copy_feedback = app
        .copy_feedback
        .as_ref()
        .filter(|(instant, _)| instant.elapsed().as_secs() < 2)
        .map(|(_, text)| text.clone());

    let status = StatusBar::new(
        app.is_generating,
        app.selection.is_active(),
        app.context_percentage(),
    )
    .with_error_message(app.error_message.clone())
    .with_copy_feedback(copy_feedback);

    frame.render_widget(status, area);
}

fn render_agent_dropdown(frame: &mut Frame, app: &mut TuiApp, header_area: Rect) {
    use crate::config::Settings;

    // Dim background for modal effect
    dim_background(frame, frame.area());

    let settings = Settings::new(&app.db);
    let available_agents = app.agents.list_filtered(app.user_mode);

    // Build items with pinned model info
    // Format: "Agent Name (model)" or "Agent Name (default)"
    let items: Vec<(String, String)> = available_agents
        .iter()
        .map(|info| {
            let pinned = settings.get_agent_pinned_model(&info.name);
            let model_hint = match pinned {
                Some(m) => format!(" ({})", m), // Show full model name, no truncation
                None => " (default)".to_string(),
            };
            (
                format!("{}{}", info.display_name, model_hint),
                info.name.clone(),
            )
        })
        .collect();

    // Calculate width dynamically based on longest item
    let max_item_len = items
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(30);

    // Add padding for borders (2), selection indicator (2), and some margin (4)
    // Use generous max width to show full model names
    let dropdown_width = ((max_item_len + 8) as u16).clamp(45, 100);

    // Height: show all items up to max 20, plus 2 for borders
    let dropdown_height = (items.len() as u16 + 2).min(20);

    // Position below the agent section in header (after "stockpot │ ")
    let dropdown_area = Rect::new(
        header_area.x + 15,
        header_area.y + 1,
        dropdown_width,
        dropdown_height,
    );

    let widget = DropdownWidget::new(
        items,
        Some(&app.current_agent),
        "Select Agent",
        ClickTarget::AgentItem,
    )
    .mouse_pos(app.last_mouse_pos);

    frame.render_widget(Clear, dropdown_area);
    widget.render(dropdown_area, frame.buffer_mut(), &mut app.hit_registry);
}

fn render_folder_modal(frame: &mut Frame, app: &mut TuiApp, header_area: Rect) {
    // Dim background for modal effect
    dim_background(frame, frame.area());

    // Modal dimensions
    let modal_width: u16 = 50;
    let modal_height: u16 = 15; // Fixed height for consistency

    // Visible area for entries (modal height - borders - header - separator)
    // 15 - 2 (borders) - 1 (path) - 1 (separator) = 11, but reserve 1 for scroll indicators
    let visible_entries: usize = 8;

    // Position modal below the folder indicator in header
    let modal_area = Rect::new(
        header_area.x + 45, // Approximate folder position
        header_area.y + 1,
        modal_width,
        modal_height,
    );

    // Clear the area
    frame.render_widget(Clear, modal_area);

    // Build the modal content
    let mut lines: Vec<Line> = Vec::new();

    // Current path display
    let path_display = app.current_working_dir.to_string_lossy();
    let truncated_path = if path_display.len() > (modal_width as usize - 6) {
        format!(
            "...{}",
            &path_display[path_display.len() - (modal_width as usize - 9)..]
        )
    } else {
        path_display.to_string()
    };

    lines.push(Line::from(vec![
        Span::styled("📁 ", Style::default()),
        Span::styled(truncated_path, Style::default().fg(Theme::YELLOW)),
    ]));
    lines.push(Line::from(Span::styled(
        "─".repeat(modal_width as usize - 2),
        Style::default().fg(Theme::BORDER),
    )));

    // Calculate scroll state
    let total_items = app.folder_modal_item_count();
    let scroll = app.folder_modal_scroll;
    let has_more_above = scroll > 0;
    let has_more_below = scroll + visible_entries < total_items;

    // Show "more above" indicator
    if has_more_above {
        lines.push(Line::from(Span::styled(
            "  ▲ more above",
            Style::default().fg(Theme::MUTED),
        )));
    }

    // Build list of all items (parent + entries)
    let mut all_items: Vec<(usize, String, bool)> = Vec::new(); // (index, display_name, is_dir)
    all_items.push((0, "..  (parent directory)".to_string(), true));

    for (i, path) in app.folder_modal_entries.iter().enumerate() {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let display_name = if name.len() > (modal_width as usize - 8) {
            format!("{}...", &name[..(modal_width as usize - 11)])
        } else {
            format!("{}/", name)
        };
        all_items.push((i + 1, display_name, true));
    }

    // Render visible items based on scroll
    let visible_range_start = scroll;
    let visible_range_end = (scroll + visible_entries).min(total_items);
    let mut render_y_offset = 0u16;

    for item_index in visible_range_start..visible_range_end {
        if let Some((idx, name, _)) = all_items.get(item_index) {
            let is_selected = app.folder_modal_selected == *idx;
            let selector = if is_selected { "▶ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            };

            lines.push(Line::from(vec![
                Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                Span::styled(name.clone(), style),
            ]));

            // Register hit target
            let entry_y = modal_area.y + 3 + (if has_more_above { 1 } else { 0 }) + render_y_offset;
            if entry_y < modal_area.y + modal_area.height - 1 {
                let entry_rect = Rect::new(modal_area.x + 1, entry_y, modal_width - 2, 1);
                app.hit_registry
                    .register(entry_rect, ClickTarget::FolderItem(*idx));
            }
            render_y_offset += 1;
        }
    }

    // Show "more below" indicator
    if has_more_below {
        lines.push(Line::from(Span::styled(
            "  ▼ more below",
            Style::default().fg(Theme::MUTED),
        )));
    }

    // Render the modal
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::ACCENT))
        .title(Span::styled(
            " Change Working Folder (Ctrl+Enter to confirm) ",
            Style::default().fg(Theme::ACCENT),
        ))
        .style(Style::default().bg(Theme::INPUT_BG));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, modal_area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_lines = vec![
        Line::from(vec![Span::styled(
            " Keyboard Shortcuts ",
            Style::default()
                .fg(Theme::HEADER)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Ctrl+Q      ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Quit", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+N      ", Style::default().fg(Theme::ACCENT)),
            Span::styled("New conversation", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+C      ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Copy selected / Cancel", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+V      ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Paste", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Enter       ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Send message", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Shift+Enter ", Style::default().fg(Theme::ACCENT)),
            Span::styled("New line", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Esc         ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Close dropdown/help", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" F1          ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Show this help", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" F2 / Ctrl+, ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Open settings", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" ↑/↓         ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Scroll activity feed", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" Mouse       ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Select text, click UI", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled(" /attach     ", Style::default().fg(Theme::ACCENT)),
            Span::styled("Attach file command", Style::default().fg(Theme::TEXT)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::YELLOW))
        .title(Span::styled(" Help ", Style::default().fg(Theme::YELLOW)))
        .style(Style::default().bg(Theme::INPUT_BG));

    let paragraph = Paragraph::new(help_lines)
        .block(block)
        .alignment(Alignment::Left);

    let area = centered_rect(60, 50, area);
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
