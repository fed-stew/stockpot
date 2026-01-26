//! Dropdown menu widget with mouse support

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

use crate::tui::hit_test::{ClickTarget, HitTestRegistry};

pub struct DropdownWidget<'a> {
    items: Vec<(String, String)>, // (Label, ID)
    selected_id: Option<&'a str>,
    mouse_pos: Option<(u16, u16)>,
    title: &'a str,
    target_wrapper: fn(String) -> ClickTarget,
}

impl<'a> DropdownWidget<'a> {
    pub fn new(
        items: Vec<(String, String)>,
        selected_id: Option<&'a str>,
        title: &'a str,
        target_wrapper: fn(String) -> ClickTarget,
    ) -> Self {
        Self {
            items,
            selected_id,
            mouse_pos: None,
            title,
            target_wrapper,
        }
    }

    pub fn mouse_pos(mut self, pos: Option<(u16, u16)>) -> Self {
        self.mouse_pos = pos;
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer, registry: &mut HitTestRegistry) {
        // Render block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!(" {} ", self.title));
        block.render(area, buf);

        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        for (i, (label, id)) in self.items.iter().enumerate() {
            if i >= inner_area.height as usize {
                break;
            }

            let item_rect = Rect {
                x: inner_area.x,
                y: inner_area.y + i as u16,
                width: inner_area.width,
                height: 1,
            };

            // Check hover
            let is_hovered = if let Some((mx, my)) = self.mouse_pos {
                mx >= item_rect.x
                    && mx < item_rect.x + item_rect.width
                    && my >= item_rect.y
                    && my < item_rect.y + item_rect.height
            } else {
                false
            };

            let is_selected = Some(id.as_str()) == self.selected_id;

            // Selection/hover indicator
            let prefix = if is_hovered {
                "▶ "
            } else if is_selected {
                "✓ "
            } else {
                "  "
            };

            let style = if is_hovered {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Render prefix and label
            let display = format!("{}{}", prefix, label);
            buf.set_string(item_rect.x, item_rect.y, &display, style);

            // Register hit
            registry.register(item_rect, (self.target_wrapper)(id.clone()));
        }
    }
}
