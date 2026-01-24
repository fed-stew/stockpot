//! Layout module - handles the main UI layout
//!
//! Provides a clean 5-area structure:
//! - Header bar (1 line)
//! - Spacer (1 line)
//! - Activity feed (flexible, takes remaining space)
//! - Input prompt (1-3 lines, dynamic based on content)
//! - Status bar (1 line)

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Layout areas for the TUI
pub struct AppLayout {
    pub header_area: Rect,
    pub activity_area: Rect,
    pub input_area: Rect,
    pub status_area: Rect,
}

impl AppLayout {
    /// Create the main layout from terminal area
    ///
    /// `input_lines` is the number of lines in the input textarea (1-3, clamped)
    pub fn new(area: Rect, input_lines: usize) -> Self {
        // Clamp input height to 1-3 lines (no border, so height = lines)
        let input_height = input_lines.clamp(1, 3) as u16;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),            // Header bar
                Constraint::Length(1),            // Spacer
                Constraint::Min(3),               // Activity feed (takes remaining space)
                Constraint::Length(input_height), // Input prompt (dynamic 1-3 lines)
                Constraint::Length(1),            // Status bar
            ])
            .split(area);

        Self {
            header_area: chunks[0],
            // chunks[1] is spacer - not stored
            activity_area: chunks[2],
            input_area: chunks[3],
            status_area: chunks[4],
        }
    }
}
