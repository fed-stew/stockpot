//! Status bar widget
//!
//! Displays mode indicator, keybind hints, and context usage.
//! Format: ` MODE  │ hints... │ XX% context`

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::theme::Theme;

/// Status bar widget with mode and context info
pub struct StatusBar {
    is_generating: bool,
    is_selecting: bool,
    context_percentage: u8,
    copy_feedback: Option<String>,
    error_message: Option<String>,
}

impl StatusBar {
    /// Create a new status bar
    pub fn new(is_generating: bool, is_selecting: bool, context_percentage: u8) -> Self {
        Self {
            is_generating,
            is_selecting,
            context_percentage,
            copy_feedback: None,
            error_message: None,
        }
    }

    /// Add copy feedback message (shown instead of hints)
    pub fn with_copy_feedback(mut self, feedback: Option<String>) -> Self {
        self.copy_feedback = feedback;
        self
    }

    /// Add error message (shown in red, takes priority over other feedback)
    pub fn with_error_message(mut self, error: Option<String>) -> Self {
        self.error_message = error;
        self
    }

    /// Get mode text and color
    fn mode_info(&self) -> (&'static str, Color) {
        if self.is_selecting {
            ("SELECT", Theme::YELLOW)
        } else if self.is_generating {
            ("GENERATING", Theme::YELLOW)
        } else {
            ("READY", Theme::GREEN)
        }
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background with Theme::BG
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(Theme::BG);
            }
        }

        let (mode_text, mode_color) = self.mode_info();
        let separator_style = Style::default().fg(Theme::MUTED);
        let hint_style = Style::default().fg(Theme::MUTED);

        // Mode pill with colored background
        let mode_span = Span::styled(
            format!(" {} ", mode_text),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        );

        // Build left side: mode + hints or feedback
        // Priority: error_message > copy_feedback > hints
        let mut spans = vec![mode_span, Span::styled(" │ ", separator_style)];

        if let Some(error) = &self.error_message {
            // Show error message in red with warning icon
            spans.push(Span::styled(
                format!("⚠ {}", error),
                Style::default()
                    .fg(Theme::ERROR)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if let Some(feedback) = &self.copy_feedback {
            // Show copy feedback in green
            spans.push(Span::styled(
                feedback.clone(),
                Style::default().fg(Theme::GREEN),
            ));
        } else {
            // Show keybind hints
            let hints = [
                ("Enter", "send"),
                ("Shift+Enter", "newline"),
                ("F1", "help"),
                ("Ctrl+Q", "quit"),
            ];

            for (i, (key, action)) in hints.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" │ ", separator_style));
                }
                spans.push(Span::styled(format!("{}: {}", key, action), hint_style));
            }
        }

        // Calculate left side width for right-alignment
        let left_content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let left_width = left_content.chars().count();

        // Context percentage (right side)
        let context_text = format!("{}% context ", self.context_percentage);
        let context_width = context_text.len();

        // Calculate padding to right-align context
        let available_width = area.width as usize;
        let padding_needed = available_width.saturating_sub(left_width + context_width);

        if padding_needed > 0 {
            spans.push(Span::raw(" ".repeat(padding_needed)));
        }

        spans.push(Span::styled(context_text, hint_style));

        // Render the line
        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}
