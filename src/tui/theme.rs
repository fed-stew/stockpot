//! TUI color theme matching rustpuppy's dark theme with good contrast
//!
//! Uses associated constants for easy access: `Theme::BG`, `Theme::TEXT`, etc.

use ratatui::style::Color;

/// Colors matching the dark theme with good contrast
///
/// Ported from rustpuppy's activity.rs theme for visual consistency.
pub struct Theme;

impl Theme {
    // ─────────────────────────────────────────────────────────────────────────────
    // Core colors from rustpuppy
    // ─────────────────────────────────────────────────────────────────────────────

    /// Main background color
    pub const BG: Color = Color::Rgb(13, 17, 23);

    /// Light gray for general text
    pub const TEXT: Color = Color::Rgb(201, 209, 217);

    /// Muted for timestamps, output
    pub const MUTED: Color = Color::Rgb(125, 133, 144);

    /// White for emphasis/headers
    pub const HEADER: Color = Color::Rgb(255, 255, 255);

    /// Blue for pills/accents
    pub const ACCENT: Color = Color::Rgb(88, 166, 255);

    /// Green for additions, success states
    pub const GREEN: Color = Color::Rgb(126, 231, 135);

    /// Red for deletions, errors
    pub const RED: Color = Color::Rgb(248, 81, 73);

    /// Yellow for tasks, notes, warnings
    pub const YELLOW: Color = Color::Rgb(210, 168, 75);

    /// Line numbers in code blocks
    pub const LINE_NUM: Color = Color::Rgb(110, 118, 129);

    /// Selection/highlight background
    pub const SELECTION: Color = Color::Rgb(56, 89, 138);

    /// Pure white for file paths (high contrast)
    pub const PATH: Color = Color::Rgb(255, 255, 255);

    /// Bright white for commands (high contrast)
    pub const COMMAND: Color = Color::Rgb(230, 237, 243);

    // ─────────────────────────────────────────────────────────────────────────────
    // Stockpot-specific colors
    // ─────────────────────────────────────────────────────────────────────────────

    /// Purple-ish tone for thinking/reasoning blocks
    pub const THINKING: Color = Color::Rgb(178, 132, 190);

    /// Cyan for nested agent indicators (distinct from accent blue)
    pub const AGENT: Color = Color::Rgb(121, 192, 202);

    /// Background for input area
    pub const INPUT_BG: Color = Color::Rgb(22, 27, 34);

    /// Background for input area when focused
    pub const INPUT_BG_FOCUSED: Color = Color::Rgb(30, 35, 42);

    // ─────────────────────────────────────────────────────────────────────────────
    // Legacy aliases for backwards compatibility
    // (can be removed once all usages are migrated)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Alias for BG
    pub const BACKGROUND: Color = Self::BG;

    /// Alias for TEXT
    pub const TEXT_PRIMARY: Color = Self::TEXT;

    /// Alias for MUTED
    pub const TEXT_MUTED: Color = Self::MUTED;

    /// Alias for RED
    pub const ERROR: Color = Self::RED;

    /// Alias for GREEN
    pub const SUCCESS: Color = Self::GREEN;

    /// Alias for YELLOW
    pub const WARNING: Color = Self::YELLOW;

    /// Border color (slightly brighter than BG for visibility)
    pub const BORDER: Color = Color::Rgb(48, 54, 61);

    /// Tool verb color (alias for ACCENT - used for tool action words)
    pub const TOOL_VERB: Color = Self::ACCENT;

    /// Panel background (slightly lighter than BG)
    pub const PANEL_BG: Color = Color::Rgb(22, 27, 34);

    /// User bubble background
    pub const USER_BUBBLE: Color = Color::Rgb(30, 35, 42);

    /// Assistant bubble background  
    pub const ASSISTANT_BUBBLE: Color = Color::Rgb(22, 27, 34);

    /// Tool bullet color
    pub const TOOL_BULLET: Color = Self::MUTED;
}

impl Default for Theme {
    fn default() -> Self {
        Theme
    }
}

impl Theme {
    /// Create a dark theme (returns unit struct - colors are all constants)
    pub fn dark() -> Self {
        Theme
    }
}
