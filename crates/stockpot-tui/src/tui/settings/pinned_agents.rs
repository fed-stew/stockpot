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
use crate::tui::app::TuiApp;
use crate::tui::hit_test::{ClickTarget, HitTestRegistry};
use crate::tui::theme::Theme;
use stockpot_core::config::Settings;

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
            Constraint::Length(4), // Default Model section (collapsed)
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
        pinned_panel == PinnedAgentsPanel::DefaultModel,
        app.settings_state.default_model_dropdown_open,
    );

    // Register hit target for the collapsed dropdown trigger
    let default_model_inner = inner_rect(chunks[0]);
    hit_registry.register(
        Rect::new(
            default_model_inner.x,
            default_model_inner.y,
            default_model_inner.width,
            1,
        ),
        ClickTarget::DefaultModelItem(default_model_index), // Click triggers dropdown
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

    // ─────────────────────────────────────────────────────────────────────────
    // Default Model Dropdown Overlay (rendered last so it's on top)
    // ─────────────────────────────────────────────────────────────────────────
    if app.settings_state.default_model_dropdown_open {
        render_default_model_dropdown(
            frame,
            chunks[0],
            &default_model,
            &available_models,
            default_model_index,
            hit_registry,
        );
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

/// Render the Default Model section (collapsed dropdown style)
fn render_default_model_section(
    frame: &mut Frame,
    area: Rect,
    current_model: &str,
    is_focused: bool,
    dropdown_open: bool,
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

    let chevron = if dropdown_open { "▲" } else { "▼" };
    let selector = if is_focused { "▶ " } else { "  " };

    let lines = vec![
        Line::from(vec![
            Span::styled(selector, Style::default().fg(Theme::ACCENT)),
            Span::styled(
                truncate_model_name(current_model),
                Style::default()
                    .fg(Theme::GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {}", chevron), Style::default().fg(Theme::MUTED)),
        ]),
        Line::from(Span::styled(
            "    Press Enter to change default model",
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the Default Model dropdown overlay (when expanded)
fn render_default_model_dropdown(
    frame: &mut Frame,
    anchor_area: Rect,
    current_model: &str,
    available_models: &[String],
    selected_index: usize,
    hit_registry: &mut HitTestRegistry,
) {
    use crate::tui::theme::dim_background;
    use ratatui::widgets::Clear;

    // Dim background for modal effect
    dim_background(frame, frame.area());

    // Calculate dropdown dimensions
    let max_model_len = available_models.iter().map(|m| m.len()).max().unwrap_or(20);
    let dropdown_width = (max_model_len + 15).min(60) as u16;
    let dropdown_height = (available_models.len() + 2).min(15) as u16;

    // Position below the anchor
    let dropdown_area = Rect::new(
        anchor_area.x,
        anchor_area.y + anchor_area.height,
        dropdown_width.min(anchor_area.width),
        dropdown_height,
    );

    // Clear and draw dropdown
    frame.render_widget(Clear, dropdown_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::ACCENT))
        .title(Span::styled(
            " Select Default Model ",
            Style::default().fg(Theme::ACCENT),
        ))
        .style(Style::default().bg(Theme::INPUT_BG));

    let inner = block.inner(dropdown_area);
    frame.render_widget(block, dropdown_area);

    // Build list of models
    let items: Vec<ListItem> = available_models
        .iter()
        .enumerate()
        .map(|(i, model)| {
            let is_current = model == current_model;
            let is_selected = i == selected_index;

            let selector = if is_selected { "▶ " } else { "  " };

            let style = if is_current {
                Style::default()
                    .fg(Theme::GREEN)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            };

            let mut line_spans = vec![
                Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                Span::styled(truncate_model_name(model), style),
            ];

            if is_current {
                line_spans.push(Span::styled(" ✓", Style::default().fg(Theme::GREEN)));
            }

            ListItem::new(Line::from(line_spans))
        })
        .collect();

    let list = List::new(items);
    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));
    frame.render_stateful_widget(list, inner, &mut list_state);

    // Register hit targets for dropdown items
    for (idx, _) in available_models.iter().enumerate() {
        let item_y = inner.y + idx as u16;
        if item_y < inner.y + inner.height {
            hit_registry.register(
                Rect::new(inner.x, item_y, inner.width, 1),
                ClickTarget::DefaultModelItem(idx),
            );
        }
    }
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
