//! Layout module - handles the main UI layout
//!
//! Provides a clean 5-area structure:
//! - Header bar (1 line)
//! - Spacer (1 line)
//! - Activity feed (flexible, takes remaining space)
//! - Input prompt (3-5 lines, minimum 3 for comfortable typing)
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
    /// `input_lines` is the number of lines in the input textarea (3-5, clamped)
    pub fn new(area: Rect, input_lines: usize) -> Self {
        // Input height: minimum 3 lines (for comfortable typing), max 5 lines
        let input_height = input_lines.max(3).clamp(3, 5) as u16;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),            // Header bar
                Constraint::Length(1),            // Spacer
                Constraint::Min(3),               // Activity feed (takes remaining space)
                Constraint::Length(input_height), // Input prompt (3-5 lines)
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
