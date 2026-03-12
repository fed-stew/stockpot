//! Toggle/Switch component for boolean settings.
//!
//! A clean, animated-style toggle switch for dark-mode UI settings.
//! Follows common toggle switch UX patterns with visual feedback on hover.
//!
//! # Example
//! ```ignore
//! toggle(
//!     "auto-save-toggle",
//!     self.auto_save_enabled,
//!     &theme,
//!     |_window, _cx| {
//!         // Handle toggle - parent is responsible for state update and cx.notify()
//!     },
//! )
//! ```

use gpui::{div, prelude::*, px, rgb, App, MouseButton, Rgba, SharedString, Styled, Window};

use crate::gui::theme::Theme;

/// Track dimensions.
const TRACK_WIDTH: f32 = 44.0;
const TRACK_HEIGHT: f32 = 24.0;

/// Knob dimensions and positioning.
const KNOB_SIZE: f32 = 20.0;
const KNOB_PADDING: f32 = 2.0;

/// Track color when toggle is OFF (dark gray).
const TRACK_OFF_COLOR: u32 = 0x4a4a4a;

/// Knob color (white).
const KNOB_COLOR: u32 = 0xffffff;

/// Render a toggle switch component.
///
/// # Arguments
/// * `id` - Unique identifier for this toggle (used for GPUI element identification)
/// * `enabled` - Current toggle state (true = ON, false = OFF)
/// * `theme` - App theme for accent color
/// * `on_toggle` - Callback invoked when the toggle is clicked.
///   The parent component is responsible for updating state and calling `cx.notify()`.
///
/// # Example
/// ```ignore
/// toggle(
///     "feature-flag",
///     self.feature_enabled,
///     &theme,
///     |_window, _cx| {
///         // Toggle logic here
///     },
/// )
/// ```
pub fn toggle(
    id: impl Into<SharedString>,
    enabled: bool,
    theme: &Theme,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id: SharedString = id.into();

    // Track background color based on state
    let track_color: Rgba = if enabled {
        theme.accent
    } else {
        rgb(TRACK_OFF_COLOR)
    };

    // Knob position: left edge (OFF) or right edge (ON)
    // OFF: margin-left = KNOB_PADDING (2px)
    // ON: margin-left = TRACK_WIDTH - KNOB_SIZE - KNOB_PADDING (22px)
    let knob_margin_left = if enabled {
        TRACK_WIDTH - KNOB_SIZE - KNOB_PADDING
    } else {
        KNOB_PADDING
    };

    // Track container
    div()
        .id(id)
        .w(px(TRACK_WIDTH))
        .h(px(TRACK_HEIGHT))
        .rounded(px(TRACK_HEIGHT / 2.0)) // Fully rounded (pill shape)
        .bg(track_color)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .flex()
        .items_center()
        // Click handler
        .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
            on_toggle(window, cx);
        })
        // Knob
        .child(
            div()
                .w(px(KNOB_SIZE))
                .h(px(KNOB_SIZE))
                .rounded(px(KNOB_SIZE / 2.0)) // Fully rounded (circle)
                .bg(rgb(KNOB_COLOR))
                .ml(px(knob_margin_left)),
        )
}

/// Render a toggle switch with a label.
///
/// Convenience wrapper that places a label to the left of the toggle.
///
/// # Arguments
/// * `id` - Unique identifier for this toggle
/// * `label` - Text label displayed to the left of the toggle
/// * `enabled` - Current toggle state
/// * `theme` - App theme for styling
/// * `on_toggle` - Callback invoked when the toggle is clicked
///
/// # Example
/// ```ignore
/// labeled_toggle(
///     "dark-mode",
///     "Enable Dark Mode",
///     self.dark_mode,
///     &theme,
///     |_window, _cx| { /* toggle logic */ },
/// )
/// ```
pub fn labeled_toggle(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    theme: &Theme,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label: SharedString = label.into();

    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(div().text_color(theme.text).child(label))
        .child(toggle(id, enabled, theme, on_toggle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_dimensions() {
        assert_eq!(TRACK_WIDTH, 44.0);
        assert_eq!(TRACK_HEIGHT, 24.0);
    }

    #[test]
    fn test_knob_dimensions() {
        assert_eq!(KNOB_SIZE, 20.0);
        assert_eq!(KNOB_PADDING, 2.0);
        // Knob should fit inside track with padding on both sides
        assert!(KNOB_SIZE + (KNOB_PADDING * 2.0) <= TRACK_WIDTH);
        assert!(KNOB_SIZE + (KNOB_PADDING * 2.0) <= TRACK_HEIGHT);
    }

    #[test]
    fn test_knob_positions() {
        // OFF position: should be at left edge with padding
        let off_position = KNOB_PADDING;
        assert_eq!(off_position, 2.0);

        // ON position: should be at right edge with padding
        let on_position = TRACK_WIDTH - KNOB_SIZE - KNOB_PADDING;
        assert_eq!(on_position, 22.0);
    }
}
