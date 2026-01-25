//! Header bar widget
//!
//! Displays stockpot branding with combined agent/model selector and working folder.
//! Design: 🍲 stockpot │ Agent 🐶 • model ▾ │ 📁 folder ▾ │ F2 ⚙
//!
//! Matches GUI behavior: single dropdown for agent selection,
//! model pinning is done in Settings > Pinned Agents tab.

use std::path::Path;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::tui::theme::Theme;

/// Header bar widget with combined agent/model and folder display
pub struct Header {
    /// Agent display name (e.g., "Code-Puppy 🐶")
    agent_display: String,
    /// Model display (pinned model name or "default")
    model_display: String,
    /// Formatted folder path
    folder_path: String,
}

impl Header {
    /// Create a new header
    ///
    /// # Arguments
    /// * `agent_display` - The agent's display name (e.g., "Code-Puppy 🐶")
    /// * `model_display` - The effective model (pinned model or "default")
    /// * `folder` - Current working directory path
    pub fn new(agent_display: &str, model_display: &str, folder: &Path) -> Self {
        Self {
            agent_display: agent_display.to_string(),
            model_display: Self::truncate_model(model_display),
            folder_path: Self::format_folder(folder),
        }
    }

    /// Truncate model name for display (max 30 chars)
    fn truncate_model(model: &str) -> String {
        if model.len() > 30 {
            // Use safe truncation to respect UTF-8 character boundaries
            let end = Self::safe_truncate_index(model, 27);
            format!("{}...", &model[..end])
        } else {
            model.to_string()
        }
    }

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

    /// Format folder path for display (truncate if too long, use ~ for home)
    fn format_folder(path: &Path) -> String {
        let path_str = path.to_string_lossy();

        // Replace home directory with ~
        let display = if let Some(home) = dirs::home_dir() {
            if let Ok(relative) = path.strip_prefix(&home) {
                format!("~/{}", relative.display())
            } else {
                path_str.to_string()
            }
        } else {
            path_str.to_string()
        };

        // Truncate if too long (keep last 25 chars with ...)
        if display.len() > 28 {
            format!("...{}", &display[display.len() - 25..])
        } else {
            display
        }
    }

    /// Get the width of the combined agent/model section (for hit testing)
    pub fn agent_section_width(&self) -> u16 {
        // agent_display + " • " (3) + model_display + " ▾" (2)
        (self.agent_display.chars().count() + 3 + self.model_display.chars().count() + 2) as u16
    }

    /// Get the x offset where the folder section starts (for hit testing)
    pub fn folder_offset(&self) -> u16 {
        // " 🍲 " (4) + "stockpot" (8) + " │ " (3) + agent_section + " │ " (3)
        (4 + 8 + 3 + self.agent_section_width() as usize + 3) as u16
    }

    /// Get the width of the folder section (for hit testing)
    pub fn folder_width(&self) -> u16 {
        // "📁 " (2) + folder_path + " ▾" (2)
        (2 + self.folder_path.chars().count() + 2) as u16
    }

    /// Get the x offset where the settings section starts (for hit testing)
    pub fn settings_offset(&self) -> u16 {
        // folder_offset + folder_width + " │ " (3)
        self.folder_offset() + self.folder_width() + 3
    }

    /// Get the width of the settings section (for hit testing)
    pub fn settings_width(&self) -> u16 {
        // "F2" (2) + " ⚙" (2)
        4
    }
}

impl Widget for Header {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background with INPUT_BG (darker header bg)
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(Theme::INPUT_BG);
            }
        }

        let separator = Style::default().fg(Theme::MUTED);
        let dropdown_indicator = Style::default().fg(Theme::MUTED);

        let header_line = Line::from(vec![
            // Branding
            Span::styled(" 🍲 ", Style::default()),
            Span::styled(
                "stockpot",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            // Separator
            Span::styled(" │ ", separator),
            // Combined agent/model selector (single dropdown)
            // Format: "Agent 🐶 • model ▾"
            Span::styled(
                &self.agent_display,
                Style::default()
                    .fg(Theme::AGENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" • ", Style::default().fg(Theme::MUTED)),
            Span::styled(&self.model_display, Style::default().fg(Theme::GREEN)),
            Span::styled(" ▾", dropdown_indicator),
            // Separator
            Span::styled(" │ ", separator),
            // Folder with dropdown
            Span::styled("📁 ", Style::default()),
            Span::styled(&self.folder_path, Style::default().fg(Theme::YELLOW)),
            Span::styled(" ▾", dropdown_indicator),
            // Separator and settings hint
            Span::styled(" │ ", separator),
            Span::styled("F2", Style::default().fg(Theme::MUTED)),
            Span::styled(" ⚙", Style::default().fg(Theme::MUTED)),
        ]);

        let paragraph = Paragraph::new(header_line).style(Style::default().bg(Theme::INPUT_BG));
        paragraph.render(area, buf);
    }
}
