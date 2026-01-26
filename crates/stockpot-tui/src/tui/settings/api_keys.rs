//! API Key Pool management for TUI settings
//!
//! Allows managing multiple API keys per provider with keyboard navigation.
//! Features:
//! - Add/delete keys with labels
//! - Toggle key active status
//! - Reorder keys by priority (Shift+J/K)
//! - Visual feedback for key status

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::sync::Arc;

use crate::tui::theme::{dim_background, Theme};
use stockpot_core::db::{Database, PoolKey};

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

// ─────────────────────────────────────────────────────────────────────────────
// Input Mode
// ─────────────────────────────────────────────────────────────────────────────

/// Input mode for key pool management
#[derive(Debug, Clone, PartialEq, Default)]
pub enum KeyPoolInputMode {
    #[default]
    Navigation,
    EnteringKey,
    EnteringLabel,
}

// ─────────────────────────────────────────────────────────────────────────────
// Key Pool State
// ─────────────────────────────────────────────────────────────────────────────

/// State for key pool management
#[derive(Debug, Default)]
pub struct KeyPoolState {
    /// Whether the key pool view is active
    pub active: bool,
    /// Current provider being managed (e.g., "openai", "anthropic")
    pub provider: Option<String>,
    /// Provider display name (e.g., "OpenAI", "Anthropic")
    pub provider_display: Option<String>,
    /// List of keys for the provider
    pub keys: Vec<PoolKey>,
    /// Selected key index in the list
    pub selected_index: usize,
    /// Current input mode
    pub input_mode: KeyPoolInputMode,
    /// New key value being entered
    pub new_key: String,
    /// New key label being entered
    pub new_label: String,
}

impl KeyPoolState {
    /// Open the key pool view for a provider
    pub fn open(&mut self, provider: &str, display_name: &str, keys: Vec<PoolKey>) {
        self.active = true;
        self.provider = Some(provider.to_string());
        self.provider_display = Some(display_name.to_string());
        self.keys = keys;
        self.selected_index = 0;
        self.input_mode = KeyPoolInputMode::Navigation;
        self.new_key.clear();
        self.new_label.clear();
    }

    /// Close the key pool view
    pub fn close(&mut self) {
        self.active = false;
        self.provider = None;
        self.provider_display = None;
        self.keys.clear();
        self.selected_index = 0;
        self.input_mode = KeyPoolInputMode::Navigation;
        self.new_key.clear();
        self.new_label.clear();
    }

    /// Select the next key in the list
    pub fn select_next(&mut self) {
        if !self.keys.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.keys.len();
        }
    }

    /// Select the previous key in the list
    pub fn select_prev(&mut self) {
        if !self.keys.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.keys.len() - 1);
        }
    }

    /// Get the currently selected key
    pub fn selected_key(&self) -> Option<&PoolKey> {
        self.keys.get(self.selected_index)
    }

    /// Check if we can move the selected key up
    pub fn can_move_up(&self) -> bool {
        self.selected_index > 0 && !self.keys.is_empty()
    }

    /// Check if we can move the selected key down
    pub fn can_move_down(&self) -> bool {
        self.selected_index < self.keys.len().saturating_sub(1)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Provider Mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Get the provider name for key pool storage from a model type label
pub fn provider_from_type_label(type_label: &str) -> Option<(&'static str, &'static str)> {
    // Returns (provider_name, display_name)
    match type_label {
        "OpenAI" => Some(("openai", "OpenAI")),
        "Anthropic" => Some(("anthropic", "Anthropic")),
        "Google Gemini" => Some(("gemini", "Google Gemini")),
        "Azure OpenAI" => Some(("azure_openai", "Azure OpenAI")),
        "OpenRouter" => Some(("openrouter", "OpenRouter")),
        _ if type_label.starts_with("Custom:") => {
            // For custom providers, we'd need to extract the name
            // For now, skip custom providers
            None
        }
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rendering
// ─────────────────────────────────────────────────────────────────────────────

/// Render the key pool management overlay
pub fn render_key_pool_overlay(frame: &mut Frame, area: Rect, state: &KeyPoolState) {
    if !state.active {
        return;
    }

    // Dim background for modal effect
    dim_background(frame, area);

    // Create centered overlay (60% width, 70% height)
    let overlay_area = centered_rect(60, 70, area);

    // Clear the overlay area
    frame.render_widget(Clear, overlay_area);

    let title = state.provider_display.as_deref().unwrap_or("API Keys");

    // Main block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::ACCENT))
        .title(Span::styled(
            format!(" 🔑 {} API Keys ", title),
            Style::default()
                .fg(Theme::HEADER)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(Theme::PANEL_BG));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    // Split into sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Header info
            Constraint::Min(8),    // Key list
            Constraint::Length(7), // Add new key section
            Constraint::Length(2), // Help line
        ])
        .split(inner);

    // Header info
    render_header_info(frame, chunks[0], state);

    // Key list
    render_key_list(frame, chunks[1], state);

    // Add new key section
    render_add_section(frame, chunks[2], state);

    // Help line
    render_help(frame, chunks[3], state);
}

fn render_header_info(frame: &mut Frame, area: Rect, state: &KeyPoolState) {
    let key_count = state.keys.len();
    let active_count = state.keys.iter().filter(|k| k.is_active).count();

    let info = Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} keys configured", key_count),
            Style::default().fg(Theme::TEXT),
        ),
        Span::styled(" • ", Style::default().fg(Theme::MUTED)),
        Span::styled(
            format!("{} active", active_count),
            Style::default().fg(if active_count > 0 {
                Theme::GREEN
            } else {
                Theme::MUTED
            }),
        ),
    ]);

    frame.render_widget(Paragraph::new(info), area);
}

fn render_key_list(frame: &mut Frame, area: Rect, state: &KeyPoolState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::BORDER))
        .title(Span::styled(
            " Keys (priority order) ",
            Style::default().fg(Theme::HEADER),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.keys.is_empty() {
        let msg = Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "No keys configured. Press 'a' to add one.",
                Style::default().fg(Theme::MUTED),
            ),
        ]));
        frame.render_widget(msg, inner);
        return;
    }

    let items: Vec<ListItem> = state
        .keys
        .iter()
        .enumerate()
        .map(|(idx, key)| {
            let is_selected = idx == state.selected_index;
            let is_active = key.is_active;

            // Mask the key value (use safe truncation for UTF-8 safety)
            let masked = if key.api_key.len() > 16 {
                let start_end = safe_truncate_index(&key.api_key, 8);
                // Find a safe start point for the suffix
                let suffix_start = key.api_key.len().saturating_sub(4);
                let safe_suffix_start = {
                    let mut start = suffix_start;
                    while start < key.api_key.len() && !key.api_key.is_char_boundary(start) {
                        start += 1;
                    }
                    start
                };
                format!(
                    "{}...{}",
                    &key.api_key[..start_end],
                    &key.api_key[safe_suffix_start..]
                )
            } else if key.api_key.len() > 8 {
                let end = safe_truncate_index(&key.api_key, 8);
                format!("{}...", &key.api_key[..end])
            } else {
                "••••••••".to_string()
            };

            let default_label = format!("Key #{}", idx + 1);
            let label = key.label.as_deref().unwrap_or(&default_label);
            let selector = if is_selected { "▶ " } else { "  " };

            // Status indicator
            let status = if !is_active {
                Span::styled(" [DISABLED]", Style::default().fg(Color::Red))
            } else if key.error_count > 0 {
                Span::styled(
                    format!(" [{}⚠]", key.error_count),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::styled(" ✓", Style::default().fg(Theme::GREEN))
            };

            // Style based on selection and active status
            let style = if is_selected {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if !is_active {
                Style::default().fg(Theme::MUTED)
            } else {
                Style::default().fg(Theme::TEXT)
            };

            ListItem::new(Line::from(vec![
                Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                Span::styled(format!("{}. ", idx + 1), Style::default().fg(Theme::MUTED)),
                Span::styled(format!("{:<20}", truncate_str(label, 20)), style),
                Span::styled(masked, Style::default().fg(Theme::MUTED)),
                status,
            ]))
        })
        .collect();

    let list = List::new(items);
    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_index));
    frame.render_stateful_widget(list, inner, &mut list_state);
}

fn render_add_section(frame: &mut Frame, area: Rect, state: &KeyPoolState) {
    let is_input_active = matches!(
        state.input_mode,
        KeyPoolInputMode::EnteringKey | KeyPoolInputMode::EnteringLabel
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if is_input_active {
            Theme::ACCENT
        } else {
            Theme::BORDER
        }))
        .title(Span::styled(
            " Add New Key ",
            Style::default().fg(if is_input_active {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // Label input
            Constraint::Length(1), // Key input
            Constraint::Length(1), // Action help
        ])
        .split(inner);

    // Label input
    let label_active = state.input_mode == KeyPoolInputMode::EnteringLabel;
    let label_style = if label_active {
        Style::default().fg(Theme::ACCENT)
    } else {
        Style::default().fg(Theme::TEXT)
    };
    let label_cursor = if label_active { "│" } else { "" };
    let label_text = if state.new_label.is_empty() && !label_active {
        "Label: (optional, press 'l' to enter)".to_string()
    } else {
        format!("Label: {}{}", state.new_label, label_cursor)
    };
    frame.render_widget(Paragraph::new(label_text).style(label_style), chunks[0]);

    // Key input
    let key_active = state.input_mode == KeyPoolInputMode::EnteringKey;
    let key_style = if key_active {
        Style::default().fg(Theme::ACCENT)
    } else {
        Style::default().fg(Theme::TEXT)
    };
    let key_cursor = if key_active { "│" } else { "" };
    let key_text = if state.new_key.is_empty() && !key_active {
        "Key:   (press 'a' to enter API key)".to_string()
    } else {
        format!(
            "Key:   {}{}",
            "•".repeat(state.new_key.len().min(40)),
            key_cursor
        )
    };
    frame.render_widget(Paragraph::new(key_text).style(key_style), chunks[1]);

    // Action help
    let action_help = if !state.new_key.is_empty() {
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Theme::ACCENT)),
            Span::styled(" to save • ", Style::default().fg(Theme::MUTED)),
            Span::styled("Esc", Style::default().fg(Theme::ACCENT)),
            Span::styled(" to cancel", Style::default().fg(Theme::MUTED)),
        ])
    } else {
        Line::from(Span::styled(
            "Enter an API key to enable saving",
            Style::default().fg(Theme::MUTED),
        ))
    };
    frame.render_widget(Paragraph::new(action_help), chunks[2]);
}

fn render_help(frame: &mut Frame, area: Rect, state: &KeyPoolState) {
    let help_spans = match state.input_mode {
        KeyPoolInputMode::Navigation => {
            vec![
                Span::styled("j/↓", Style::default().fg(Theme::ACCENT)),
                Span::styled(" ", Style::default().fg(Theme::MUTED)),
                Span::styled("k/↑", Style::default().fg(Theme::ACCENT)),
                Span::styled(":nav  ", Style::default().fg(Theme::MUTED)),
                Span::styled("a", Style::default().fg(Theme::ACCENT)),
                Span::styled(":add  ", Style::default().fg(Theme::MUTED)),
                Span::styled("l", Style::default().fg(Theme::ACCENT)),
                Span::styled(":label  ", Style::default().fg(Theme::MUTED)),
                Span::styled("d", Style::default().fg(Theme::ACCENT)),
                Span::styled(":delete  ", Style::default().fg(Theme::MUTED)),
                Span::styled("Space", Style::default().fg(Theme::ACCENT)),
                Span::styled(":toggle  ", Style::default().fg(Theme::MUTED)),
                Span::styled("J/K", Style::default().fg(Theme::ACCENT)),
                Span::styled(":reorder  ", Style::default().fg(Theme::MUTED)),
                Span::styled("q/Esc", Style::default().fg(Theme::ACCENT)),
                Span::styled(":back", Style::default().fg(Theme::MUTED)),
            ]
        }
        KeyPoolInputMode::EnteringKey | KeyPoolInputMode::EnteringLabel => {
            vec![
                Span::styled("Enter", Style::default().fg(Theme::ACCENT)),
                Span::styled(":confirm  ", Style::default().fg(Theme::MUTED)),
                Span::styled("Esc", Style::default().fg(Theme::ACCENT)),
                Span::styled(":cancel  ", Style::default().fg(Theme::MUTED)),
                Span::styled("Ctrl+V", Style::default().fg(Theme::ACCENT)),
                Span::styled(":paste", Style::default().fg(Theme::MUTED)),
            ]
        }
    };

    let help = Paragraph::new(Line::from(help_spans));
    frame.render_widget(help, area);
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a centered rectangle
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

/// Truncate a string to max length
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}…", &s[..max_len - 1])
    } else {
        s.to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Event Handling
// ─────────────────────────────────────────────────────────────────────────────

/// Result of handling a key pool event
#[derive(Debug, Clone, PartialEq)]
pub enum KeyPoolEventResult {
    /// Event was handled, no further action needed
    Handled,
    /// Close the key pool overlay
    Close,
    /// Ignored (event not handled by key pool)
    Ignored,
}

/// Handle a key event in the key pool overlay.
/// Returns true if the event was consumed.
pub fn handle_key_pool_event(
    state: &mut KeyPoolState,
    db: &Arc<Database>,
    key: KeyEvent,
    clipboard_text: Option<&str>,
) -> KeyPoolEventResult {
    if !state.active {
        return KeyPoolEventResult::Ignored;
    }

    match state.input_mode {
        KeyPoolInputMode::Navigation => handle_navigation_mode(state, db, key),
        KeyPoolInputMode::EnteringKey => handle_key_input_mode(state, db, key, clipboard_text),
        KeyPoolInputMode::EnteringLabel => handle_label_input_mode(state, key, clipboard_text),
    }
}

/// Handle events in navigation mode
fn handle_navigation_mode(
    state: &mut KeyPoolState,
    db: &Arc<Database>,
    key: KeyEvent,
) -> KeyPoolEventResult {
    match (key.modifiers, key.code) {
        // Close overlay
        (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Char('q')) => {
            state.close();
            KeyPoolEventResult::Close
        }

        // Navigate down: j or Down arrow
        (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
            state.select_next();
            KeyPoolEventResult::Handled
        }

        // Navigate up: k or Up arrow
        (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
            state.select_prev();
            KeyPoolEventResult::Handled
        }

        // Start entering API key
        (KeyModifiers::NONE, KeyCode::Char('a')) => {
            state.input_mode = KeyPoolInputMode::EnteringKey;
            state.new_key.clear();
            KeyPoolEventResult::Handled
        }

        // Start entering label
        (KeyModifiers::NONE, KeyCode::Char('l')) => {
            state.input_mode = KeyPoolInputMode::EnteringLabel;
            state.new_label.clear();
            KeyPoolEventResult::Handled
        }

        // Delete selected key
        (KeyModifiers::NONE, KeyCode::Char('d')) | (KeyModifiers::NONE, KeyCode::Delete) => {
            if let Some(key) = state.selected_key() {
                let key_id = key.id;
                if let Err(e) = db.delete_pool_key(key_id) {
                    tracing::error!("Failed to delete pool key: {}", e);
                } else {
                    // Refresh keys from database
                    refresh_key_pool(state, db);
                }
            }
            KeyPoolEventResult::Handled
        }

        // Toggle active/inactive with Space
        (KeyModifiers::NONE, KeyCode::Char(' ')) => {
            if let Some(key) = state.selected_key() {
                let key_id = key.id;
                let new_active = !key.is_active;
                if let Err(e) = db.set_key_active(key_id, new_active) {
                    tracing::error!("Failed to toggle key active status: {}", e);
                } else {
                    refresh_key_pool(state, db);
                }
            }
            KeyPoolEventResult::Handled
        }

        // Move key up (Shift+K)
        (KeyModifiers::SHIFT, KeyCode::Char('K')) => {
            if state.can_move_up() {
                move_key_up(state, db);
            }
            KeyPoolEventResult::Handled
        }

        // Move key down (Shift+J)
        (KeyModifiers::SHIFT, KeyCode::Char('J')) => {
            if state.can_move_down() {
                move_key_down(state, db);
            }
            KeyPoolEventResult::Handled
        }

        _ => KeyPoolEventResult::Handled, // Absorb all other keys in navigation
    }
}

/// Handle events in key input mode
fn handle_key_input_mode(
    state: &mut KeyPoolState,
    db: &Arc<Database>,
    key: KeyEvent,
    clipboard_text: Option<&str>,
) -> KeyPoolEventResult {
    match (key.modifiers, key.code) {
        // Cancel input
        (KeyModifiers::NONE, KeyCode::Esc) => {
            state.input_mode = KeyPoolInputMode::Navigation;
            state.new_key.clear();
            KeyPoolEventResult::Handled
        }

        // Confirm and save key
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if !state.new_key.is_empty() {
                if let Some(provider) = state.provider.as_ref() {
                    let label = if state.new_label.is_empty() {
                        None
                    } else {
                        Some(state.new_label.as_str())
                    };

                    // Calculate next priority (after existing keys)
                    let next_priority = state.keys.len() as i32;

                    if let Err(e) =
                        db.save_pool_key(provider, &state.new_key, label, Some(next_priority))
                    {
                        tracing::error!("Failed to save pool key: {}", e);
                    } else {
                        state.new_key.clear();
                        state.new_label.clear();
                        refresh_key_pool(state, db);
                    }
                }
            }
            state.input_mode = KeyPoolInputMode::Navigation;
            KeyPoolEventResult::Handled
        }

        // Paste from clipboard
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
            if let Some(text) = clipboard_text {
                // Only paste first line (no newlines in keys)
                let first_line = text.lines().next().unwrap_or("");
                state.new_key.push_str(first_line);
            }
            KeyPoolEventResult::Handled
        }

        // Backspace to delete character
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            state.new_key.pop();
            KeyPoolEventResult::Handled
        }

        // Type character
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            state.new_key.push(c);
            KeyPoolEventResult::Handled
        }

        _ => KeyPoolEventResult::Handled,
    }
}

/// Handle events in label input mode
fn handle_label_input_mode(
    state: &mut KeyPoolState,
    key: KeyEvent,
    clipboard_text: Option<&str>,
) -> KeyPoolEventResult {
    match (key.modifiers, key.code) {
        // Cancel input
        (KeyModifiers::NONE, KeyCode::Esc) => {
            state.input_mode = KeyPoolInputMode::Navigation;
            state.new_label.clear();
            KeyPoolEventResult::Handled
        }

        // Confirm label and switch to key input
        (KeyModifiers::NONE, KeyCode::Enter) => {
            // Label is saved, now prompt for key if not already entered
            if state.new_key.is_empty() {
                state.input_mode = KeyPoolInputMode::EnteringKey;
            } else {
                state.input_mode = KeyPoolInputMode::Navigation;
            }
            KeyPoolEventResult::Handled
        }

        // Paste from clipboard
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
            if let Some(text) = clipboard_text {
                let first_line = text.lines().next().unwrap_or("");
                state.new_label.push_str(first_line);
            }
            KeyPoolEventResult::Handled
        }

        // Backspace to delete character
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            state.new_label.pop();
            KeyPoolEventResult::Handled
        }

        // Type character
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            state.new_label.push(c);
            KeyPoolEventResult::Handled
        }

        _ => KeyPoolEventResult::Handled,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Database Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Refresh the key pool state from the database
pub fn refresh_key_pool(state: &mut KeyPoolState, db: &Arc<Database>) {
    if let Some(provider) = state.provider.as_ref() {
        match db.get_pool_keys(provider) {
            Ok(keys) => {
                let old_len = state.keys.len();
                state.keys = keys;
                // Adjust selected index if needed
                if state.selected_index >= state.keys.len() && !state.keys.is_empty() {
                    state.selected_index = state.keys.len() - 1;
                } else if state.keys.is_empty() {
                    state.selected_index = 0;
                } else if state.keys.len() > old_len {
                    // New key added - select it
                    state.selected_index = state.keys.len() - 1;
                }
            }
            Err(e) => {
                tracing::error!("Failed to refresh key pool: {}", e);
            }
        }
    }
}

/// Move the selected key up in priority (swap with previous key)
pub fn move_key_up(state: &mut KeyPoolState, db: &Arc<Database>) {
    if !state.can_move_up() {
        return;
    }

    let current_idx = state.selected_index;
    let prev_idx = current_idx - 1;

    // Get the IDs and priorities of both keys
    let current_key = &state.keys[current_idx];
    let prev_key = &state.keys[prev_idx];

    let current_id = current_key.id;
    let prev_id = prev_key.id;
    let current_priority = current_key.priority;
    let prev_priority = prev_key.priority;

    // Swap priorities
    if let Err(e) = db.update_key_priority(current_id, prev_priority) {
        tracing::error!("Failed to update key priority: {}", e);
        return;
    }
    if let Err(e) = db.update_key_priority(prev_id, current_priority) {
        tracing::error!("Failed to update key priority: {}", e);
        return;
    }

    // Refresh and update selection
    state.selected_index = prev_idx;
    refresh_key_pool(state, db);
    // Keep selection on the moved key
    state.selected_index = prev_idx;
}

/// Move the selected key down in priority (swap with next key)
pub fn move_key_down(state: &mut KeyPoolState, db: &Arc<Database>) {
    if !state.can_move_down() {
        return;
    }

    let current_idx = state.selected_index;
    let next_idx = current_idx + 1;

    // Get the IDs and priorities of both keys
    let current_key = &state.keys[current_idx];
    let next_key = &state.keys[next_idx];

    let current_id = current_key.id;
    let next_id = next_key.id;
    let current_priority = current_key.priority;
    let next_priority = next_key.priority;

    // Swap priorities
    if let Err(e) = db.update_key_priority(current_id, next_priority) {
        tracing::error!("Failed to update key priority: {}", e);
        return;
    }
    if let Err(e) = db.update_key_priority(next_id, current_priority) {
        tracing::error!("Failed to update key priority: {}", e);
        return;
    }

    // Refresh and update selection
    state.selected_index = next_idx;
    refresh_key_pool(state, db);
    // Keep selection on the moved key
    state.selected_index = next_idx;
}
