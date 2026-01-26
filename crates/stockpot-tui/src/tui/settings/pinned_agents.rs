//! Pinned Agents settings tab
//!
//! Configure model pinning for specific agents with a two-panel layout.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::PinnedAgentsPanel;
use stockpot_core::config::Settings;
use crate::tui::app::TuiApp;
use crate::tui::hit_test::{ClickTarget, HitTestRegistry};
use crate::tui::theme::Theme;

/// Render the Pinned Agents settings tab content
pub fn render_pinned_agents_tab(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    hit_registry: &mut HitTestRegistry,
) {
    let settings = Settings::new(&app.db);
    let agents = app.agents.list();
    let available_models = app.model_registry.list_available(&app.db);
    let default_model = settings.model();
    let pins = settings.get_all_agent_pinned_models().unwrap_or_default();

    // Get current state
    let pinned_panel = app.settings_state.pinned_panel;
    let agent_list_index = app.settings_state.agent_list_index;
    let model_list_index = app.settings_state.model_list_index;
    let default_model_index = app.settings_state.default_model_index;

    // Determine selected agent
    let selected_agent = if !agents.is_empty() {
        agents.get(agent_list_index).map(|a| a.name.clone())
    } else {
        None
    };

    // Layout: Default Model section at top, then two-panel layout below
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Default Model section
            Constraint::Min(10),   // Two-panel layout
        ])
        .split(area);

    // ─────────────────────────────────────────────────────────────────────────
    // Default Model Section
    // ─────────────────────────────────────────────────────────────────────────
    render_default_model_section(
        frame,
        chunks[0],
        &default_model,
        &available_models,
        default_model_index,
        pinned_panel == PinnedAgentsPanel::DefaultModel,
    );

    // ─────────────────────────────────────────────────────────────────────────
    // Two-Panel Layout (Agents | Models)
    // ─────────────────────────────────────────────────────────────────────────
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    // Left Panel - Agent List
    let agents_data: Vec<_> = agents
        .iter()
        .map(|a| (a.name.clone(), a.display_name.clone()))
        .collect();
    render_agent_list(
        frame,
        panels[0],
        &agents_data,
        &pins,
        &default_model,
        agent_list_index,
        pinned_panel == PinnedAgentsPanel::Agents,
    );

    // Register hit targets for agent list items
    // Each agent item is 2 lines tall (name + status)
    let agent_panel_inner = inner_rect(panels[0]);
    for (idx, _) in agents_data.iter().enumerate() {
        let item_y = agent_panel_inner.y + (idx as u16 * 2); // 2 lines per item
        if item_y + 1 < agent_panel_inner.y + agent_panel_inner.height {
            hit_registry.register(
                Rect::new(agent_panel_inner.x, item_y, agent_panel_inner.width, 2),
                ClickTarget::PinnedAgentItem(idx),
            );
        }
    }

    // Right Panel - Model List for selected agent
    if let Some(ref agent_name) = selected_agent {
        let pinned_model = settings.get_agent_pinned_model(agent_name);
        render_model_list(
            frame,
            panels[1],
            agent_name,
            &available_models,
            &default_model,
            pinned_model.as_deref(),
            model_list_index,
            pinned_panel == PinnedAgentsPanel::Models,
        );

        // Register hit targets for model list items
        // "Use Default" is index 0, then models are index 1+
        let model_panel_inner = inner_rect(panels[1]);
        let total_model_items = available_models.len() + 1; // +1 for "Use Default"
        for idx in 0..total_model_items {
            let item_y = model_panel_inner.y + idx as u16; // 1 line per item
            if item_y < model_panel_inner.y + model_panel_inner.height {
                hit_registry.register(
                    Rect::new(model_panel_inner.x, item_y, model_panel_inner.width, 1),
                    ClickTarget::PinnedModelItem(idx),
                );
            }
        }
    } else {
        render_no_agent_selected(frame, panels[1]);
    }
}

/// Calculate inner rect (inside 1-cell border)
fn inner_rect(area: Rect) -> Rect {
    Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

/// Render the Default Model dropdown section
fn render_default_model_section(
    frame: &mut Frame,
    area: Rect,
    current_model: &str,
    _available_models: &[String],
    _selected_index: usize,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Theme::ACCENT
    } else {
        Theme::BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " Default Model ",
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selector = if is_focused { "▶ " } else { "  " };

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, Style::default().fg(Theme::ACCENT)),
            Span::styled("Current: ", Style::default().fg(Theme::MUTED)),
            Span::styled(
                current_model,
                Style::default()
                    .fg(Theme::GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "    Used when an agent does not have a pinned model",
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the left panel with agent list
fn render_agent_list(
    frame: &mut Frame,
    area: Rect,
    agents: &[(String, String)], // (name, display_name)
    pins: &std::collections::HashMap<String, String>,
    default_model: &str,
    selected_index: usize,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Theme::ACCENT
    } else {
        Theme::BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " Agents ",
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if agents.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "  No agents available",
            Style::default().fg(Theme::MUTED),
        ));
        frame.render_widget(msg, inner);
        return;
    }

    // Create list items
    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, (name, display_name))| {
            let is_selected = i == selected_index;
            let pinned = pins.get(name);

            let status = match pinned {
                Some(model) => format!("Pinned: {}", truncate_model_name(model)),
                None => format!("Default: {}", truncate_model_name(default_model)),
            };

            let status_color = if pinned.is_some() {
                Theme::GREEN
            } else {
                Theme::MUTED
            };

            let selector = if is_selected && is_focused {
                "▶ "
            } else {
                "  "
            };

            let name_style = if is_selected && is_focused {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Theme::TEXT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                    Span::styled(display_name.clone(), name_style),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(status, Style::default().fg(status_color)),
                ]),
            ])
        })
        .collect();

    let list = List::new(items);

    // Use ListState for scrolling
    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));

    frame.render_stateful_widget(list, inner, &mut list_state);
}

/// Render the right panel with model list for selected agent
#[allow(clippy::too_many_arguments)]
fn render_model_list(
    frame: &mut Frame,
    area: Rect,
    agent_name: &str,
    available_models: &[String],
    default_model: &str,
    pinned_model: Option<&str>,
    selected_index: usize,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Theme::ACCENT
    } else {
        Theme::BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            format!(" Models for {} ", truncate_name(agent_name, 20)),
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build list: "Use Default" first, then all models
    let mut items: Vec<ListItem> = Vec::with_capacity(available_models.len() + 1);

    // "Use Default" option (index 0)
    let use_default_selected = pinned_model.is_none();
    let is_item_selected = selected_index == 0;
    let selector = if is_item_selected && is_focused {
        "▶ "
    } else {
        "  "
    };

    let default_style = if use_default_selected {
        Style::default()
            .fg(Theme::GREEN)
            .add_modifier(Modifier::BOLD)
    } else if is_item_selected && is_focused {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::TEXT)
    };

    items.push(ListItem::new(Line::from(vec![
        Span::styled(selector, Style::default().fg(Theme::ACCENT)),
        Span::styled(
            format!("Use Default ({})", truncate_model_name(default_model)),
            default_style,
        ),
    ])));

    // Model options (index 1+)
    for (i, model) in available_models.iter().enumerate() {
        let item_index = i + 1;
        let is_pinned = pinned_model == Some(model.as_str());
        let is_item_selected = selected_index == item_index;

        let selector = if is_item_selected && is_focused {
            "▶ "
        } else {
            "  "
        };

        let model_style = if is_pinned {
            Style::default()
                .fg(Theme::GREEN)
                .add_modifier(Modifier::BOLD)
        } else if is_item_selected && is_focused {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::TEXT)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(selector, Style::default().fg(Theme::ACCENT)),
            Span::styled(truncate_model_name(model), model_style),
        ])));
    }

    let list = List::new(items);

    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));

    frame.render_stateful_widget(list, inner, &mut list_state);
}

/// Render placeholder when no agent is selected
fn render_no_agent_selected(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::BORDER))
        .title(Span::styled(" Models ", Style::default().fg(Theme::HEADER)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let msg = Paragraph::new(Span::styled(
        "  Select an agent to pin a model",
        Style::default().fg(Theme::MUTED),
    ));
    frame.render_widget(msg, inner);
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// Find the last valid UTF-8 char boundary at or before max_bytes
fn safe_truncate_index(s: &str, max_bytes: usize) -> usize {
    if s.len() <= max_bytes {
        return s.len();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Truncate model name for display
fn truncate_model_name(name: &str) -> String {
    if name.len() > 35 {
        let end = safe_truncate_index(name, 32);
        format!("{}...", &name[..end])
    } else {
        name.to_string()
    }
}

/// Truncate any name to a max length
fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() > max_len {
        let end = safe_truncate_index(name, max_len.saturating_sub(3));
        format!("{}...", &name[..end])
    } else {
        name.to_string()
    }
}
