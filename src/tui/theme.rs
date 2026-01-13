//! TUI color theme matching the GUI dark theme

use ratatui::style::Color;

/// TUI color theme
#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub panel_background: Color,
    pub text: Color,
    pub text_muted: Color,
    pub border: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub user_bubble: Color,
    pub assistant_bubble: Color,
    pub tool_verb: Color,
    pub tool_bullet: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme matching GUI
    pub fn dark() -> Self {
        Self {
            background: Color::Rgb(30, 30, 30),
            panel_background: Color::Rgb(37, 37, 38),
            text: Color::Rgb(212, 212, 212),
            text_muted: Color::Rgb(128, 128, 128),
            border: Color::Rgb(60, 60, 60),
            accent: Color::Rgb(0, 122, 204),
            success: Color::Rgb(72, 185, 100),
            warning: Color::Rgb(255, 193, 7),
            error: Color::Rgb(244, 67, 54),
            user_bubble: Color::Rgb(45, 45, 48),
            assistant_bubble: Color::Rgb(37, 37, 38),
            tool_verb: Color::Rgb(156, 220, 254),
            tool_bullet: Color::Rgb(100, 100, 100),
        }
    }
}
