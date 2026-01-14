//! Main UI rendering

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::app::TuiApp;
use crate::tui::hit_test::ClickTarget;
use crate::tui::widgets::{DropdownWidget, MessageList, MetricsWidget};

/// Render the entire UI
pub fn render(frame: &mut Frame, app: &mut TuiApp) {
    app.hit_registry.clear();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Toolbar
            Constraint::Min(0),    // Messages
            Constraint::Length(1), // Metrics
            Constraint::Length(3), // Input
        ])
        .split(frame.area());

    render_toolbar(frame, app, chunks[0]);
    render_messages(frame, app, chunks[1]);
    render_metrics(frame, app, chunks[2]);
    render_input(frame, app, chunks[3]);

    // Render dropdowns on top if visible
    if app.show_agent_dropdown {
        render_agent_dropdown(frame, app, chunks[0]);
    }
    if app.show_model_dropdown {
        render_model_dropdown(frame, app, chunks[0]);
    }

    if app.show_help {
        render_help(frame, frame.area());
    }
}

fn render_toolbar(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    let toolbar = Paragraph::new(Line::from(vec![
        Span::styled(
            "🍲 Stockpot",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(&app.current_agent, Style::default().fg(Color::Cyan)),
        Span::raw(" ▾ │ "),
        Span::styled(&app.current_model, Style::default().fg(Color::Green)),
        Span::raw(" ▾"),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(toolbar, area);

    // Register hit targets for dropdowns
    app.hit_registry.register(
        Rect::new(area.x + 12, area.y, 20, 1),
        ClickTarget::AgentDropdown,
    );
    app.hit_registry.register(
        Rect::new(area.x + 40, area.y, 20, 1),
        ClickTarget::ModelDropdown,
    );
}

fn render_messages(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    if app.conversation.messages.is_empty() {
        // Welcome screen
        let welcome = Paragraph::new(vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled("🍲", Style::default())),
            Line::from(""),
            Line::from(Span::styled(
                "Welcome to Stockpot",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Your AI-powered coding assistant",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Type a message below to get started",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "Press F1 for keyboard shortcuts",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ])
        .alignment(Alignment::Center);

        frame.render_widget(welcome, area);
        return;
    }

    let message_list = MessageList::new(&app.conversation.messages, &app.theme)
        .generating(app.is_generating)
        .registry(&mut app.hit_registry)
        .selection(&app.selection);

    frame.render_stateful_widget(message_list, area, &mut app.message_list_state);
}

fn render_metrics(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let throughput = if app.is_generating {
        Some(app.current_throughput_cps)
    } else {
        None
    };

    let usage = if app.context_tokens_used > 0 {
        Some(format!(
            "{} / {}",
            app.context_tokens_used, app.context_window_size
        ))
    } else {
        None
    };

    let widget = MetricsWidget::new(app.current_model.clone(), throughput, usage);
    frame.render_widget(widget, area);
}

fn render_input(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    // Update textarea block based on state
    let mut title = if app.is_generating {
        " Generating... ".to_string()
    } else {
        " Message ".to_string()
    };

    if !app.attachments.is_empty() {
        title = format!("{} [{} files] ", title, app.attachments.pending.len());
    }

    let border_color = if app.is_generating {
        Color::DarkGray
    } else {
        Color::Cyan
    };

    app.input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title),
    );

    frame.render_widget(&app.input, area);
}

fn render_agent_dropdown(frame: &mut Frame, app: &mut TuiApp, toolbar_area: Rect) {
    let available_agents = app.agents.list();
    let items: Vec<(String, String)> = available_agents
        .iter()
        .map(|info| (info.display_name.clone(), info.name.clone()))
        .collect();

    let dropdown_height = (items.len() as u16 + 2).min(10);
    let dropdown_area = Rect::new(toolbar_area.x + 12, toolbar_area.y + 1, 30, dropdown_height);

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

fn render_model_dropdown(frame: &mut Frame, app: &mut TuiApp, toolbar_area: Rect) {
    let available_models = app.model_registry.list_available(&app.db);
    let items: Vec<(String, String)> = available_models
        .iter()
        .map(|name| (name.clone(), name.clone()))
        .collect();

    let dropdown_height = (items.len() as u16 + 2).min(10);
    let dropdown_area = Rect::new(toolbar_area.x + 40, toolbar_area.y + 1, 35, dropdown_height);

    let widget = DropdownWidget::new(
        items,
        Some(&app.current_model),
        "Select Model",
        ClickTarget::ModelItem,
    )
    .mouse_pos(app.last_mouse_pos);

    frame.render_widget(Clear, dropdown_area);
    widget.render(dropdown_area, frame.buffer_mut(), &mut app.hit_registry);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_lines = vec![
        Line::from(vec![Span::styled(
            " Keyboard Shortcuts ",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Ctrl+Q      ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+N      ", Style::default().fg(Color::Cyan)),
            Span::raw("New conversation"),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+C      ", Style::default().fg(Color::Cyan)),
            Span::raw("Copy selected / Cancel"),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+V      ", Style::default().fg(Color::Cyan)),
            Span::raw("Paste"),
        ]),
        Line::from(vec![
            Span::styled(" Enter       ", Style::default().fg(Color::Cyan)),
            Span::raw("Send message"),
        ]),
        Line::from(vec![
            Span::styled(" Shift+Enter ", Style::default().fg(Color::Cyan)),
            Span::raw("New line"),
        ]),
        Line::from(vec![
            Span::styled(" Esc         ", Style::default().fg(Color::Cyan)),
            Span::raw("Close dropdown/help"),
        ]),
        Line::from(vec![
            Span::styled(" F1          ", Style::default().fg(Color::Cyan)),
            Span::raw("Show this help"),
        ]),
        Line::from(vec![
            Span::styled(" ↑/↓         ", Style::default().fg(Color::Cyan)),
            Span::raw("Scroll messages"),
        ]),
        Line::from(vec![
            Span::styled(" Mouse       ", Style::default().fg(Color::Cyan)),
            Span::raw("Select text, click UI"),
        ]),
        Line::from(vec![
            Span::styled(" /attach     ", Style::default().fg(Color::Cyan)),
            Span::raw("Attach file command"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Help ");

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
