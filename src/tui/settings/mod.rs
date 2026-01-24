//! Settings screen module for TUI
//!
//! Provides a full-screen settings overlay with multiple tabs:
//! - General: Theme, behavior, shortcuts
//! - Models: Model configuration and API keys
//! - Pinned Agents: Agent-specific model pinning
//! - MCP Servers: Model Context Protocol server configuration

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame,
};

use crate::config::Settings;
use crate::db::Database;
use crate::tui::app::TuiApp;
use crate::tui::theme::Theme;
use anyhow::Result;

// Submodules for each settings tab
mod general;
pub mod mcp_servers;
pub mod models;
mod pinned_agents;

pub use general::render_general_tab;
pub use mcp_servers::render_mcp_servers_tab;
pub use models::render_models_tab;
pub use pinned_agents::render_pinned_agents_tab;

// ─────────────────────────────────────────────────────────────────────────────
// Settings Item Types (for keyboard navigation and interaction)
// ─────────────────────────────────────────────────────────────────────────────

/// Type of setting control
#[derive(Debug, Clone)]
pub enum SettingItemType {
    /// Radio button group - mutually exclusive options
    Radio {
        options: Vec<String>,
        selected: usize,
    },
    /// Toggle switch - on/off
    Toggle { enabled: bool },
    /// Text input field
    Text { value: String },
    /// Section header (not selectable)
    Header,
}

/// A single setting item for tracking selectable items
#[derive(Debug, Clone)]
pub struct SettingItem {
    /// Unique identifier for the setting
    pub id: String,
    /// Display name
    pub name: String,
    /// Type and current value
    pub item_type: SettingItemType,
    /// Area on screen (for hit testing)
    pub area: Rect,
}

impl SettingItem {
    pub fn new(id: &str, name: &str, item_type: SettingItemType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            item_type,
            area: Rect::default(),
        }
    }

    pub fn with_area(mut self, area: Rect) -> Self {
        self.area = area;
        self
    }

    /// Check if this item is selectable (not a header)
    pub fn is_selectable(&self) -> bool {
        !matches!(self.item_type, SettingItemType::Header)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings Tab Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Available tabs in the settings screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Models,
    PinnedAgents,
    McpServers,
}

impl SettingsTab {
    /// Get all tabs in order
    pub fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::General,
            SettingsTab::Models,
            SettingsTab::PinnedAgents,
            SettingsTab::McpServers,
        ]
    }

    /// Get the display name for the tab
    pub fn display_name(&self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Models => "Models",
            SettingsTab::PinnedAgents => "Pinned Agents",
            SettingsTab::McpServers => "MCP Servers",
        }
    }

    /// Get the index of the tab (for Tabs widget)
    pub fn index(&self) -> usize {
        match self {
            SettingsTab::General => 0,
            SettingsTab::Models => 1,
            SettingsTab::PinnedAgents => 2,
            SettingsTab::McpServers => 3,
        }
    }

    /// Get tab from index
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => SettingsTab::General,
            1 => SettingsTab::Models,
            2 => SettingsTab::PinnedAgents,
            3 => SettingsTab::McpServers,
            _ => SettingsTab::General,
        }
    }

    /// Get the next tab (wraps around)
    pub fn next(&self) -> Self {
        let idx = (self.index() + 1) % Self::all().len();
        Self::from_index(idx)
    }

    /// Get the previous tab (wraps around)
    pub fn prev(&self) -> Self {
        let len = Self::all().len();
        let idx = (self.index() + len - 1) % len;
        Self::from_index(idx)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings State
// ─────────────────────────────────────────────────────────────────────────────

/// Which panel is focused in Pinned Agents tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinnedAgentsPanel {
    #[default]
    DefaultModel,
    Agents,
    Models,
}

impl PinnedAgentsPanel {
    pub fn next(&self) -> Self {
        match self {
            Self::DefaultModel => Self::Agents,
            Self::Agents => Self::Models,
            Self::Models => Self::DefaultModel,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::DefaultModel => Self::Models,
            Self::Agents => Self::DefaultModel,
            Self::Models => Self::Agents,
        }
    }
}

/// Which panel is focused in MCP Servers tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpPanel {
    #[default]
    Servers,
    Agents,
    McpCheckboxes,
}

impl McpPanel {
    pub fn next(&self) -> Self {
        match self {
            Self::Servers => Self::Agents,
            Self::Agents => Self::McpCheckboxes,
            Self::McpCheckboxes => Self::Servers,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Servers => Self::McpCheckboxes,
            Self::Agents => Self::Servers,
            Self::McpCheckboxes => Self::Agents,
        }
    }
}

/// State for the settings screen
#[derive(Debug, Default)]
pub struct SettingsState {
    /// Currently active tab
    pub active_tab: SettingsTab,
    /// Selected item index within the current tab (General tab)
    pub selected_index: usize,
    /// Whether we're in edit mode for a field
    pub editing: bool,
    /// Cached items for the current tab (for navigation)
    pub current_items: Vec<SettingItem>,

    // ─────────────────────────────────────────────────────────────────────────
    // Pinned Agents tab specific state
    // ─────────────────────────────────────────────────────────────────────────
    /// Currently focused panel in Pinned Agents tab
    pub pinned_panel: PinnedAgentsPanel,
    /// Selected agent name
    pub selected_agent: Option<String>,
    /// Index in agent list
    pub agent_list_index: usize,
    /// Index in model list (0 = "Use Default")
    pub model_list_index: usize,
    /// Index in default model dropdown
    pub default_model_index: usize,

    // ─────────────────────────────────────────────────────────────────────────
    // Models tab specific state
    // ─────────────────────────────────────────────────────────────────────────
    /// Selected index in models tab (0 = OAuth section, 1+ = model groups/items)
    pub models_selected_index: usize,
    /// Which provider groups are expanded (by provider label)
    pub models_expanded_providers: std::collections::HashSet<String>,
    /// Whether we're in the OAuth section (vs model list)
    pub models_in_oauth_section: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // MCP Servers tab specific state
    // ─────────────────────────────────────────────────────────────────────────
    /// Currently focused panel in MCP tab
    pub mcp_panel: McpPanel,
    /// Selected index in MCP server list
    pub mcp_server_index: usize,
    /// Selected index in agent list (for MCP assignments)
    pub mcp_agent_index: usize,
    /// Selected index in MCP checkboxes
    pub mcp_checkbox_index: usize,
}

impl SettingsState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch to the next tab
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.reset_tab_state();
    }

    /// Switch to the previous tab
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.reset_tab_state();
    }

    /// Reset state when switching tabs
    fn reset_tab_state(&mut self) {
        self.selected_index = 0;
        self.editing = false;
        self.current_items.clear();
        // Reset pinned agents state
        self.pinned_panel = PinnedAgentsPanel::default();
        self.agent_list_index = 0;
        self.model_list_index = 0;
        self.default_model_index = 0;
        // Reset models state
        self.models_selected_index = 0;
        self.models_in_oauth_section = true;
        // Don't clear expanded providers - keep user's preference
        // Reset MCP state
        self.mcp_panel = McpPanel::default();
        self.mcp_server_index = 0;
        self.mcp_agent_index = 0;
        self.mcp_checkbox_index = 0;
    }

    /// Get the number of selectable items
    pub fn selectable_count(&self) -> usize {
        self.current_items
            .iter()
            .filter(|i| i.is_selectable())
            .count()
    }

    /// Clamp selected_index to valid range
    pub fn clamp_selection(&mut self) {
        let count = self.selectable_count();
        if count > 0 && self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Render Function
// ─────────────────────────────────────────────────────────────────────────────

/// Render the settings overlay
pub fn render_settings(frame: &mut Frame, area: Rect, app: &TuiApp) {
    // Create a centered overlay (80% width, 80% height)
    let overlay_area = centered_rect(80, 80, area);

    // Clear the area first
    frame.render_widget(Clear, overlay_area);

    // Main settings block with border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::BORDER))
        .title(Span::styled(
            " ⚙ Settings ",
            Style::default()
                .fg(Theme::HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Theme::PANEL_BG));

    frame.render_widget(block.clone(), overlay_area);

    // Inner area (inside the border)
    let inner_area = block.inner(overlay_area);

    // Split into tabs area and content area (leave room for footer)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Tab bar
            Constraint::Min(1),    // Content
            Constraint::Length(2), // Footer
        ])
        .split(inner_area);

    let tabs_area = chunks[0];
    let content_area = chunks[1];
    let footer_area = chunks[2];

    // Render tab bar
    render_tab_bar(frame, tabs_area, &app.settings_state);

    // Render active tab content
    match app.settings_state.active_tab {
        SettingsTab::General => render_general_tab(frame, content_area, app),
        SettingsTab::Models => render_models_tab(frame, content_area, app),
        SettingsTab::PinnedAgents => render_pinned_agents_tab(frame, content_area, app),
        SettingsTab::McpServers => render_mcp_servers_tab(frame, content_area, app),
    }

    // Render footer with navigation hints
    render_footer(frame, footer_area, &app.settings_state);
}

/// Render the tab bar at the top of the settings panel
fn render_tab_bar(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let tab_titles: Vec<Line> = SettingsTab::all()
        .iter()
        .map(|tab| {
            let style = if *tab == state.active_tab {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::MUTED)
            };
            Line::from(Span::styled(tab.display_name(), style))
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(state.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled(" │ ", Style::default().fg(Theme::BORDER)));

    frame.render_widget(tabs, area);
}

/// Render footer with keyboard hints (context-sensitive based on active tab)
fn render_footer(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let footer_text = match state.active_tab {
        SettingsTab::PinnedAgents => Line::from(vec![
            Span::styled("Tab", Style::default().fg(Theme::ACCENT)),
            Span::styled("/", Style::default().fg(Theme::MUTED)),
            Span::styled("←→", Style::default().fg(Theme::ACCENT)),
            Span::styled(" switch panels  ", Style::default().fg(Theme::MUTED)),
            Span::styled("↑↓", Style::default().fg(Theme::ACCENT)),
            Span::styled(" navigate  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Enter", Style::default().fg(Theme::ACCENT)),
            Span::styled(" select/pin  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Esc", Style::default().fg(Theme::ACCENT)),
            Span::styled(" close", Style::default().fg(Theme::MUTED)),
        ]),
        SettingsTab::Models => Line::from(vec![
            Span::styled("Tab", Style::default().fg(Theme::ACCENT)),
            Span::styled("/", Style::default().fg(Theme::MUTED)),
            Span::styled("Shift+Tab", Style::default().fg(Theme::ACCENT)),
            Span::styled(" switch tabs  ", Style::default().fg(Theme::MUTED)),
            Span::styled("↑↓", Style::default().fg(Theme::ACCENT)),
            Span::styled(" navigate  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Enter", Style::default().fg(Theme::ACCENT)),
            Span::styled(" expand/set default  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Esc", Style::default().fg(Theme::ACCENT)),
            Span::styled(" close", Style::default().fg(Theme::MUTED)),
        ]),
        SettingsTab::McpServers => Line::from(vec![
            Span::styled("←→", Style::default().fg(Theme::ACCENT)),
            Span::styled(" switch panels  ", Style::default().fg(Theme::MUTED)),
            Span::styled("↑↓", Style::default().fg(Theme::ACCENT)),
            Span::styled(" navigate  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Enter", Style::default().fg(Theme::ACCENT)),
            Span::styled(" toggle  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Del", Style::default().fg(Theme::ACCENT)),
            Span::styled(" remove  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Esc", Style::default().fg(Theme::ACCENT)),
            Span::styled(" close", Style::default().fg(Theme::MUTED)),
        ]),
        _ => Line::from(vec![
            Span::styled("Tab", Style::default().fg(Theme::ACCENT)),
            Span::styled("/", Style::default().fg(Theme::MUTED)),
            Span::styled("Shift+Tab", Style::default().fg(Theme::ACCENT)),
            Span::styled(" switch tabs  ", Style::default().fg(Theme::MUTED)),
            Span::styled("↑↓", Style::default().fg(Theme::ACCENT)),
            Span::styled(" navigate  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Enter/Space", Style::default().fg(Theme::ACCENT)),
            Span::styled(" toggle  ", Style::default().fg(Theme::MUTED)),
            Span::styled("Esc", Style::default().fg(Theme::ACCENT)),
            Span::styled(" close", Style::default().fg(Theme::MUTED)),
        ]),
    };

    let footer = Paragraph::new(footer_text).alignment(Alignment::Center);
    frame.render_widget(footer, area);
}

/// Create a centered rectangle with the given percentage of width and height
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

// ─────────────────────────────────────────────────────────────────────────────
// Database Helper Functions (preserved from original settings.rs)
// ─────────────────────────────────────────────────────────────────────────────

pub fn save_current_model(db: &Database, model: &str) -> Result<()> {
    let settings = Settings::new(db);
    settings.set("model", model)?;
    Ok(())
}

pub fn save_current_agent(db: &Database, agent: &str) -> Result<()> {
    let settings = Settings::new(db);
    settings.set("last_agent", agent)?;
    Ok(())
}

pub fn get_agent_pinned_model(db: &Database, agent: &str) -> Option<String> {
    let settings = Settings::new(db);
    settings.get_agent_pinned_model(agent)
}
