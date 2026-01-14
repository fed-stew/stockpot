//! Nested agent section widget for TUI
//!
//! Collapsible container for sub-agent output

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::state::AgentSection;
use crate::tui::theme::Theme;

pub struct NestedAgentWidget<'a> {
    section: &'a AgentSection,
    #[allow(dead_code)]
    theme: &'a Theme,
}

impl<'a> NestedAgentWidget<'a> {
    pub fn new(section: &'a AgentSection, theme: &'a Theme) -> Self {
        Self { section, theme }
    }

    pub fn height(&self) -> u16 {
        if self.section.is_collapsed {
            1
        } else {
            1 + self.section.content().lines().count() as u16
        }
    }
}

impl Widget for NestedAgentWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Header
        let arrow = if self.section.is_collapsed {
            "▶"
        } else {
            "▼"
        };
        let status = if self.section.is_complete { "" } else { "..." };

        let header_style = Style::default().fg(Color::Cyan);

        let header = Line::from(vec![
            Span::styled(format!("{} ", arrow), header_style),
            Span::styled(
                &self.section.display_name,
                header_style.add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {}", status), header_style),
        ]);

        buf.set_line(area.x, area.y, &header, area.width);

        // Content if expanded
        if !self.section.is_collapsed {
            let content = self.section.content();
            let style = Style::default().fg(Color::DarkGray);
            let mut y_offset = 1;
            for line in content.lines() {
                if y_offset >= area.height {
                    break;
                }
                buf.set_line(
                    area.x + 2,
                    area.y + y_offset,
                    &Line::from(Span::styled(line, style)),
                    area.width - 2,
                );
                y_offset += 1;
            }
        }
    }
}
