//! Tool call widget for TUI
//!
//! Displays tool execution with:
//! - Verb + Subject format (e.g., "Read src/main.rs")
//! - Status indicator: ● (running), ✓ (success), ✗ (failed)

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::state::ToolCallSection;
use crate::tui::theme::Theme;

pub struct ToolCallWidget<'a> {
    section: &'a ToolCallSection,
    theme: &'a Theme,
}

impl<'a> ToolCallWidget<'a> {
    pub fn new(section: &'a ToolCallSection, theme: &'a Theme) -> Self {
        Self { section, theme }
    }
}

impl Widget for ToolCallWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let info = &self.section.info;

        let (symbol, color) = if self.section.is_running {
            ("●", Color::Yellow)
        } else {
            match self.section.succeeded {
                Some(true) => ("✓", Color::Green),
                Some(false) => ("✗", Color::Red),
                None => ("?", Color::Gray),
            }
        };

        let verb_style = Style::default()
            .fg(self.theme.tool_verb)
            .add_modifier(Modifier::BOLD);
        let subject_style = Style::default().fg(self.theme.text);

        let mut spans = vec![
            Span::styled(format!("  {} ", symbol), Style::default().fg(color)),
            Span::styled(format!("{} ", info.verb), verb_style),
        ];

        if !info.subject.is_empty() {
            spans.push(Span::styled(&info.subject, subject_style));
        }

        if self.section.is_running {
            spans.push(Span::styled("...", Style::default().fg(Color::DarkGray)));
        }

        buf.set_line(area.x, area.y, &Line::from(spans), area.width);
    }
}
