//! Theme and color definitions for the GUI

use gpui::{rgb, Rgba};

/// Color theme for the application
#[derive(Clone)]
pub struct Theme {
    /// Background color for the main window
    pub background: Rgba,
    /// Background color for the sidebar/panels
    pub panel_background: Rgba,
    /// Border color
    pub border: Rgba,
    /// Primary text color
    pub text: Rgba,
    /// Secondary/muted text color
    pub text_muted: Rgba,
    /// Accent color for interactive elements
    pub accent: Rgba,
    /// User message bubble background
    pub user_bubble: Rgba,
    /// Assistant message bubble background
    pub assistant_bubble: Rgba,
    /// Tool call card background
    pub tool_card: Rgba,
    /// Tool call bullet/dot color (muted)
    pub tool_bullet: Rgba,
    /// Tool call verb color (bold verb like "Edited", "Read")
    pub tool_verb: Rgba,
    /// Success color (green)
    pub success: Rgba,
    /// Error color (red)
    pub error: Rgba,
    /// Warning color (yellow)
    pub warning: Rgba,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            background: rgb(0x1e1e1e),
            panel_background: rgb(0x252526),
            border: rgb(0x3c3c3c),
            text: rgb(0xcccccc),
            text_muted: rgb(0x808080),
            accent: rgb(0x0078d4),
            user_bubble: rgb(0x2d2d30),
            assistant_bubble: rgb(0x1e1e1e),
            tool_card: rgb(0x2d2d30),
            tool_bullet: rgb(0x6e6e6e),
            tool_verb: rgb(0x0078d4),
            success: rgb(0x4ec9b0),
            error: rgb(0xf14c4c),
            warning: rgb(0xdcdcaa),
        }
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            background: rgb(0xffffff),
            panel_background: rgb(0xf3f3f3),
            border: rgb(0xe5e5e5),
            text: rgb(0x1e1e1e),
            text_muted: rgb(0x6e6e6e),
            accent: rgb(0x0078d4),
            user_bubble: rgb(0xe8e8e8),
            assistant_bubble: rgb(0xf5f5f5),
            tool_card: rgb(0xf0f0f0),
            tool_bullet: rgb(0x9e9e9e),
            tool_verb: rgb(0x0078d4),
            success: rgb(0x16825d),
            error: rgb(0xd32f2f),
            warning: rgb(0xf9a825),
        }
    }
}
