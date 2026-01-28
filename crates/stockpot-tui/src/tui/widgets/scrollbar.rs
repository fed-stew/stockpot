//! Visual scrollbar widget for TUI
//!
//! Renders a track with thumb indicator showing scroll position.

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::tui::theme::Theme;

/// Scrollbar widget that shows scroll position
pub struct Scrollbar {
    /// Current scroll offset (in content lines)
    scroll_offset: usize,
    /// Total content height (in lines)
    content_height: usize,
    /// Viewport height (in lines)
    viewport_height: usize,
}

impl Scrollbar {
    /// Create a new scrollbar
    pub fn new(scroll_offset: usize, content_height: usize, viewport_height: usize) -> Self {
        Self {
            scroll_offset,
            content_height,
            viewport_height,
        }
    }

    /// Check if scrollbar should be visible (content exceeds viewport)
    pub fn is_visible(&self) -> bool {
        self.content_height > self.viewport_height
    }

    /// Calculate thumb size as a proportion of track
    fn thumb_height(&self, track_height: u16) -> u16 {
        if self.content_height == 0 || self.viewport_height == 0 {
            return track_height;
        }

        let ratio = self.viewport_height as f64 / self.content_height as f64;
        let thumb = (track_height as f64 * ratio).round() as u16;

        // Minimum thumb size of 1, maximum of track height
        thumb.clamp(1, track_height)
    }

    /// Calculate thumb position (top of thumb)
    fn thumb_position(&self, track_height: u16, thumb_height: u16) -> u16 {
        if self.content_height <= self.viewport_height {
            return 0;
        }

        let max_scroll = self.content_height.saturating_sub(self.viewport_height);
        if max_scroll == 0 {
            return 0;
        }

        let scroll_ratio = self.scroll_offset as f64 / max_scroll as f64;
        let max_thumb_pos = track_height.saturating_sub(thumb_height);

        (max_thumb_pos as f64 * scroll_ratio).round() as u16
    }
}

impl Widget for Scrollbar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Don't render if no scrolling needed
        if !self.is_visible() || area.height == 0 || area.width == 0 {
            return;
        }

        let track_height = area.height;
        let thumb_height = self.thumb_height(track_height);
        let thumb_pos = self.thumb_position(track_height, thumb_height);

        // Styles
        let track_style = Style::default().fg(Theme::BORDER);
        let thumb_style = Style::default().fg(Theme::ACCENT);

        // Render track and thumb
        for y in 0..track_height {
            let cell = &mut buf[(area.x, area.y + y)];

            if y >= thumb_pos && y < thumb_pos + thumb_height {
                // Thumb - solid block
                cell.set_char('█').set_style(thumb_style);
            } else {
                // Track - dim vertical line
                cell.set_char('│').set_style(track_style);
            }
        }
    }
}
