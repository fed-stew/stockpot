//! MCP Servers settings tab
//!
//! Two-panel layout with MCP server list and agent-MCP assignments.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::config::Settings;
use crate::mcp::McpConfig;
use crate::tui::app::TuiApp;
use crate::tui::hit_test::{ClickTarget, HitTestRegistry};
use crate::tui::theme::Theme;

use super::McpPanel;

/// Server info: (name, enabled, description, command_preview)
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub command_preview: String,
}

/// Load servers from MCP config
pub fn load_servers() -> Vec<ServerInfo> {
    let mcp_config = McpConfig::load_or_default();
    let mut servers: Vec<ServerInfo> = mcp_config
        .servers
        .iter()
        .map(|(name, entry)| {
            let cmd_preview = if entry.args.is_empty() {
                entry.command.clone()
            } else {
                format!("{} {}", entry.command, entry.args.join(" "))
            };
            ServerInfo {
                name: name.clone(),
                enabled: entry.enabled,
                description: entry.description.clone(),
                command_preview: truncate_str(&cmd_preview, 35),
            }
        })
        .collect();
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    servers
}

/// Render the MCP Servers settings tab content
pub fn render_mcp_servers_tab(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    hit_registry: &mut HitTestRegistry,
) {
    let servers = load_servers();
    let settings = Settings::new(&app.db);
    let agents = app.agents.list();

    // Get the selected agent (default to first agent if not set)
    let selected_agent_name = agents
        .get(app.settings_state.mcp_agent_index)
        .map(|a| a.name.clone())
        .unwrap_or_default();

    let agent_mcps = settings.get_agent_mcps(&selected_agent_name);
    let all_attachments = settings.get_all_agent_mcps().unwrap_or_default();

    // Layout: Left panel (servers) | Right panel (agent assignments)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Server list
            Constraint::Percentage(60), // Agent assignments
        ])
        .split(area);

    // ─────────────────────────────────────────────────────────────────────────
    // Left Panel: MCP Server List
    // ─────────────────────────────────────────────────────────────────────────
    render_server_list(
        frame,
        chunks[0],
        &servers,
        app.settings_state.mcp_server_index,
        app.settings_state.mcp_panel == McpPanel::Servers,
    );

    // Register hit targets for server list items
    // Each server item is 4 lines tall (name, status, command, empty)
    // There's a 2-line header before the list
    if !servers.is_empty() {
        let server_panel_inner = inner_rect(chunks[0]);
        let list_start_y = server_panel_inner.y + 2; // Skip 2-line header
        for (idx, _) in servers.iter().enumerate() {
            let item_y = list_start_y + (idx as u16 * 4); // 4 lines per item
            if item_y + 3 < server_panel_inner.y + server_panel_inner.height {
                hit_registry.register(
                    Rect::new(server_panel_inner.x, item_y, server_panel_inner.width, 4),
                    ClickTarget::McpServerItem(idx),
                );
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Right Panel: Agent MCP Assignments (two sub-columns)
    // ─────────────────────────────────────────────────────────────────────────
    render_agent_assignments(
        frame,
        chunks[1],
        app,
        &agents,
        &selected_agent_name,
        &servers,
        &agent_mcps,
        &all_attachments,
    );

    // Calculate the sub-column areas for hit target registration
    // The right panel has a border, then splits into two sub-columns
    let right_inner = inner_rect(chunks[1]);
    let sub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(right_inner);

    // Register hit targets for agent list (left sub-column)
    // Each agent item is 1 line tall
    let agent_panel_inner = inner_rect(sub_chunks[0]);
    for (idx, _) in agents.iter().enumerate() {
        let item_y = agent_panel_inner.y + idx as u16;
        if item_y < agent_panel_inner.y + agent_panel_inner.height {
            hit_registry.register(
                Rect::new(agent_panel_inner.x, item_y, agent_panel_inner.width, 1),
                ClickTarget::McpAgentItem(idx),
            );
        }
    }

    // Register hit targets for MCP checkboxes (right sub-column)
    // Only enabled servers are shown, each is 1 line tall
    let checkbox_panel_inner = inner_rect(sub_chunks[1]);
    let enabled_servers: Vec<_> = servers.iter().filter(|s| s.enabled).collect();
    if !selected_agent_name.is_empty() {
        for (idx, _) in enabled_servers.iter().enumerate() {
            let item_y = checkbox_panel_inner.y + idx as u16;
            if item_y < checkbox_panel_inner.y + checkbox_panel_inner.height {
                hit_registry.register(
                    Rect::new(
                        checkbox_panel_inner.x,
                        item_y,
                        checkbox_panel_inner.width,
                        1,
                    ),
                    ClickTarget::McpCheckbox(idx),
                );
            }
        }
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

/// Render the left panel with MCP servers list
fn render_server_list(
    frame: &mut Frame,
    area: Rect,
    servers: &[ServerInfo],
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
            format!(" 🔌 MCP Servers ({} defined) ", servers.len()),
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if servers.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No MCP servers defined.",
                Style::default().fg(Theme::MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Add servers via config file:",
                Style::default().fg(Theme::MUTED),
            )),
            Line::from(Span::styled(
                "  ~/.stockpot/mcp_servers.json",
                Style::default().fg(Theme::ACCENT),
            )),
        ]);
        frame.render_widget(msg, inner);
        return;
    }

    // Header with hints
    let header_area = Rect::new(inner.x, inner.y, inner.width, 2);
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );

    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("  Enter", Style::default().fg(Theme::ACCENT)),
        Span::styled(": toggle  ", Style::default().fg(Theme::MUTED)),
        Span::styled("Del", Style::default().fg(Theme::ACCENT)),
        Span::styled(": remove", Style::default().fg(Theme::MUTED)),
    ])]);
    frame.render_widget(header, header_area);

    // Server list
    let items: Vec<ListItem> = servers
        .iter()
        .enumerate()
        .map(|(idx, server)| {
            let is_selected = idx == selected_index && is_focused;
            render_server_item(server, is_selected)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, list_area);
}

/// Render a single server item (multiple lines)
fn render_server_item(server: &ServerInfo, is_selected: bool) -> ListItem<'static> {
    let selector = if is_selected { "▶ " } else { "  " };
    let (status_icon, status_text, status_color) = if server.enabled {
        ("✓", "enabled", Theme::GREEN)
    } else {
        ("✗", "disabled", Theme::MUTED)
    };

    let name_style = if is_selected {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::TEXT)
    };

    ListItem::new(vec![
        // Server name
        Line::from(vec![
            Span::styled(selector, Style::default().fg(Theme::ACCENT)),
            Span::styled(server.name.clone(), name_style),
        ]),
        // Status
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("{} ", status_icon),
                Style::default().fg(status_color),
            ),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
        // Command preview
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                server.command_preview.clone(),
                Style::default().fg(Theme::MUTED),
            ),
        ]),
        // Empty line for spacing
        Line::from(""),
    ])
}

/// Render the right panel with agent-MCP assignments
#[allow(clippy::too_many_arguments)]
fn render_agent_assignments(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    agents: &[crate::agents::AgentInfo],
    selected_agent: &str,
    servers: &[ServerInfo],
    agent_mcps: &[String],
    all_attachments: &std::collections::HashMap<String, Vec<String>>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::BORDER))
        .title(Span::styled(
            " 🤖 Agent → MCP Attachments ",
            Style::default().fg(Theme::HEADER),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Two sub-columns: Agents | MCP Checkboxes
    let sub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // Left sub-column: Agent list
    render_agent_list(
        frame,
        sub_chunks[0],
        agents,
        selected_agent,
        all_attachments,
        app.settings_state.mcp_agent_index,
        app.settings_state.mcp_panel == McpPanel::Agents,
    );

    // Right sub-column: MCP checkboxes for selected agent
    render_mcp_checkboxes(
        frame,
        sub_chunks[1],
        servers,
        selected_agent,
        agent_mcps,
        app.settings_state.mcp_checkbox_index,
        app.settings_state.mcp_panel == McpPanel::McpCheckboxes,
    );
}

/// Render the agent list sub-column
fn render_agent_list(
    frame: &mut Frame,
    area: Rect,
    agents: &[crate::agents::AgentInfo],
    selected_agent: &str,
    all_attachments: &std::collections::HashMap<String, Vec<String>>,
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
                Theme::MUTED
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if agents.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "  No agents found",
            Style::default().fg(Theme::MUTED),
        ));
        frame.render_widget(msg, inner);
        return;
    }

    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(idx, info)| {
            let is_selected = idx == selected_index && is_focused;
            let is_current_agent = info.name == selected_agent;
            let mcp_count = all_attachments
                .get(&info.name)
                .map(|m| m.len())
                .unwrap_or(0);

            let selector = if is_selected { "▶ " } else { "  " };

            let name_style = if is_selected {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if is_current_agent {
                Style::default().fg(Theme::GREEN)
            } else {
                Style::default().fg(Theme::TEXT)
            };

            let mut spans = vec![
                Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                Span::styled(info.display_name.clone(), name_style),
            ];

            if mcp_count > 0 {
                spans.push(Span::styled(
                    format!(" ({})", mcp_count),
                    Style::default().fg(Theme::MUTED),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

/// Render the MCP checkboxes sub-column
fn render_mcp_checkboxes(
    frame: &mut Frame,
    area: Rect,
    servers: &[ServerInfo],
    selected_agent: &str,
    agent_mcps: &[String],
    selected_index: usize,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Theme::ACCENT
    } else {
        Theme::BORDER
    };

    let title = if selected_agent.is_empty() {
        " MCPs ".to_string()
    } else {
        format!(" MCPs for {} ", truncate_str(selected_agent, 15))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::MUTED
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Filter to enabled servers only
    let enabled_servers: Vec<&ServerInfo> = servers.iter().filter(|s| s.enabled).collect();

    if enabled_servers.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No enabled MCP servers",
                Style::default().fg(Theme::MUTED),
            )),
        ]);
        frame.render_widget(msg, inner);
        return;
    }

    if selected_agent.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "  Select an agent first",
            Style::default().fg(Theme::MUTED),
        ));
        frame.render_widget(msg, inner);
        return;
    }

    let items: Vec<ListItem> = enabled_servers
        .iter()
        .enumerate()
        .map(|(idx, server)| {
            let is_selected = idx == selected_index && is_focused;
            let is_attached = agent_mcps.contains(&server.name);

            let selector = if is_selected { "▶ " } else { "  " };
            let checkbox = if is_attached { "[✓]" } else { "[ ]" };
            let checkbox_color = if is_attached {
                Theme::GREEN
            } else {
                Theme::MUTED
            };

            let name_style = if is_selected {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            };

            ListItem::new(Line::from(vec![
                Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                Span::styled(
                    format!("{} ", checkbox),
                    Style::default().fg(checkbox_color),
                ),
                Span::styled(server.name.clone(), name_style),
            ]))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

/// Truncate a string for display
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions for keyboard navigation
// ─────────────────────────────────────────────────────────────────────────────

/// Get count of MCP servers
pub fn server_count() -> usize {
    load_servers().len()
}

/// Get count of enabled MCP servers (for checkbox navigation)
pub fn enabled_server_count() -> usize {
    load_servers().iter().filter(|s| s.enabled).count()
}

/// Toggle enabled state of a server
pub fn toggle_server_enabled(server_index: usize) {
    let mut mcp_config = McpConfig::load_or_default();
    let servers = load_servers();

    if let Some(server) = servers.get(server_index) {
        if let Some(entry) = mcp_config.servers.get_mut(&server.name) {
            entry.enabled = !entry.enabled;
            let _ = mcp_config.save_default();
        }
    }
}

/// Remove a server from config
pub fn remove_server(server_index: usize) {
    let mut mcp_config = McpConfig::load_or_default();
    let servers = load_servers();

    if let Some(server) = servers.get(server_index) {
        mcp_config.servers.remove(&server.name);
        let _ = mcp_config.save_default();
    }
}

/// Get the name of an enabled server at a given checkbox index
pub fn get_enabled_server_name(checkbox_index: usize) -> Option<String> {
    load_servers()
        .iter()
        .filter(|s| s.enabled)
        .nth(checkbox_index)
        .map(|s| s.name.clone())
}
