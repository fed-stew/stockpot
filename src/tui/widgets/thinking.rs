//! Thinking section widget for TUI
//!
//! Collapsible block showing model's reasoning process

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::state::ThinkingSection;
use crate::tui::theme::Theme;

const PILL_LEFT: &str = "◖";
const PILL_RIGHT: &str = "◗";

pub struct ThinkingWidget<'a> {
    section: &'a ThinkingSection,
    #[allow(dead_code)]
    theme: &'a Theme,
}

impl<'a> ThinkingWidget<'a> {
    pub fn new(section: &'a ThinkingSection, theme: &'a Theme) -> Self {
        Self { section, theme }
    }

    pub fn height(&self) -> u16 {
        if self.section.is_collapsed {
            1
        } else {
            1 + self.section.content.lines().count() as u16
        }
    }
}

impl Widget for ThinkingWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Header with pill-style rendering
        let arrow = if self.section.is_collapsed {
            "▶"
        } else {
            "▼"
        };
        
        let pill_style = Style::default()
            .fg(Theme::THINKING)
            .add_modifier(Modifier::BOLD);
        let muted_style = Style::default().fg(Theme::MUTED);

        let mut header_spans = vec![
            Span::styled(format!("{} ", arrow), muted_style),
            Span::styled(format!("{} Thinking {}", PILL_LEFT, PILL_RIGHT), pill_style),
        ];

        if self.section.is_collapsed {
            header_spans.push(Span::styled(
                format!(" {}", self.section.preview()),
                muted_style,
            ));
        } else if !self.section.is_complete {
            header_spans.push(Span::styled("...", muted_style));
        }

        buf.set_line(area.x, area.y, &Line::from(header_spans), area.width);

        // Content if expanded
        if !self.section.is_collapsed {
            let content_style = Style::default().fg(Theme::MUTED);
            let mut y_offset = 1;

            for line in self.section.content.lines() {
                if y_offset >= area.height {
                    break;
                }
                buf.set_line(
                    area.x + 2, // Indent
                    area.y + y_offset,
                    &Line::from(Span::styled(line, content_style)),
                    area.width - 2,
                );
                y_offset += 1;
            }
        }
    }
}
