//! Render style configuration for terminal output.

use crossterm::style::Color;

/// Render style configuration.
#[derive(Debug, Clone)]
pub struct RenderStyle {
    pub info_color: Color,
    pub success_color: Color,
    pub warning_color: Color,
    pub error_color: Color,
    pub code_color: Color,
    pub diff_add_color: Color,
    pub diff_remove_color: Color,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            info_color: Color::White,
            success_color: Color::Green,
            warning_color: Color::Yellow,
            error_color: Color::Red,
            code_color: Color::Cyan,
            diff_add_color: Color::Green,
            diff_remove_color: Color::Red,
        }
    }
}
