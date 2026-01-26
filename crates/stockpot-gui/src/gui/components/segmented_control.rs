//! Segmented control component for mutually exclusive options.
//!
//! A pill-shaped horizontal control with clickable segments, commonly used
//! for settings like mode selection (Normal | Expert | Developer).
//!
//! # Example
//! ```ignore
//! #[derive(Clone, PartialEq)]
//! enum UserMode {
//!     Normal,
//!     Expert,
//!     Developer,
//! }
//!
//! segmented_control(
//!     "user-mode-selector",
//!     vec![
//!         (UserMode::Normal, "Normal"),
//!         (UserMode::Expert, "Expert"),
//!         (UserMode::Developer, "Developer"),
//!     ],
//!     self.user_mode.clone(),
//!     &theme,
//!     |mode, _window, _cx| {
//!         // Handle selection - parent is responsible for state update and cx.notify()
//!     },
//! )
//! ```

use std::rc::Rc;

use gpui::{div, prelude::*, px, rgb, App, MouseButton, SharedString, Styled, Window};

use crate::gui::theme::Theme;

/// Container padding (space between container edge and segments).
const CONTAINER_PADDING: f32 = 4.0;

/// Segment horizontal padding.
const SEGMENT_PADDING_X: f32 = 12.0;

/// Segment vertical padding.
const SEGMENT_PADDING_Y: f32 = 8.0;

/// Font size for segment labels.
const FONT_SIZE: f32 = 13.0;

/// Selected segment text color (white).
const SELECTED_TEXT_COLOR: u32 = 0xffffff;

/// Render a segmented control component.
///
/// # Arguments
/// * `id` - Base identifier for this control (segments will have IDs like "id-0", "id-1", etc.)
/// * `options` - Vector of (value, label) pairs defining the available options
/// * `selected` - Currently selected value
/// * `theme` - App theme for colors
/// * `on_select` - Callback invoked when a segment is clicked, receives the selected value.
///   The parent component is responsible for updating state and calling `cx.notify()`.
///
/// # Type Parameters
/// * `T` - The value type. Must be `PartialEq` (for comparison) and `Clone` (for passing to callback).
///
/// # Example
/// ```ignore
/// segmented_control(
///     "theme-selector",
///     vec![
///         ("light", "Light"),
///         ("dark", "Dark"),
///         ("system", "System"),
///     ],
///     self.theme_mode,
///     &theme,
///     |mode, _window, _cx| {
///         // Handle selection
///     },
/// )
/// ```
pub fn segmented_control<T, F>(
    id: impl Into<SharedString>,
    options: Vec<(T, &'static str)>,
    selected: T,
    theme: &Theme,
    on_select: F,
) -> impl IntoElement
where
    T: PartialEq + Clone + 'static,
    F: Fn(T, &mut Window, &mut App) + 'static,
{
    // Wrap callback in Rc so it can be shared across segments
    let on_select = Rc::new(on_select);
    let id: SharedString = id.into();
    let container_bg = theme.tool_card;
    let text_color = theme.text;
    let accent_color = theme.accent;

    // Build segments
    let segments: Vec<_> = options
        .into_iter()
        .enumerate()
        .map(|(index, (value, label))| {
            let is_selected = value == selected;
            let segment_id = SharedString::from(format!("{}-{}", id, index));
            let on_select = Rc::clone(&on_select);
            let value_for_click = value.clone();

            div()
                .id(segment_id)
                .px(px(SEGMENT_PADDING_X))
                .py(px(SEGMENT_PADDING_Y))
                .rounded(px(999.0)) // Fully rounded (pill shape)
                .text_size(px(FONT_SIZE))
                .cursor_pointer()
                // Conditional styling based on selection state
                .when(is_selected, |segment| {
                    segment
                        .bg(accent_color)
                        .text_color(rgb(SELECTED_TEXT_COLOR))
                })
                .when(!is_selected, |segment| {
                    segment
                        .bg(gpui::transparent_black())
                        .text_color(text_color)
                        .hover(|s| s.opacity(0.7))
                })
                // Click handler
                .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                    on_select(value_for_click.clone(), window, cx);
                })
                .child(label)
        })
        .collect();

    // Container
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .p(px(CONTAINER_PADDING))
        .rounded(px(999.0)) // Fully rounded (pill shape)
        .bg(container_bg)
        .children(segments)
}

/// Render a segmented control with a label.
///
/// Convenience wrapper that places a label above the segmented control.
///
/// # Arguments
/// * `id` - Base identifier for this control
/// * `label` - Text label displayed above the control
/// * `options` - Vector of (value, label) pairs
/// * `selected` - Currently selected value
/// * `theme` - App theme for styling
/// * `on_select` - Callback invoked when a segment is clicked
///
/// # Example
/// ```ignore
/// labeled_segmented_control(
///     "user-mode",
///     "User Mode",
///     vec![
///         (UserMode::Normal, "Normal"),
///         (UserMode::Expert, "Expert"),
///     ],
///     self.mode.clone(),
///     &theme,
///     |mode, _window, _cx| { /* ... */ },
/// )
/// ```
pub fn labeled_segmented_control<T, F>(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    options: Vec<(T, &'static str)>,
    selected: T,
    theme: &Theme,
    on_select: F,
) -> impl IntoElement
where
    T: PartialEq + Clone + 'static,
    F: Fn(T, &mut Window, &mut App) + 'static,
{
    let label: SharedString = label.into();

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_color(theme.text)
                .text_size(px(FONT_SIZE))
                .child(label),
        )
        .child(segmented_control(id, options, selected, theme, on_select))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_padding() {
        assert_eq!(CONTAINER_PADDING, 4.0);
    }

    #[test]
    fn test_segment_padding() {
        assert_eq!(SEGMENT_PADDING_X, 12.0);
        assert_eq!(SEGMENT_PADDING_Y, 8.0);
    }

    #[test]
    fn test_font_size() {
        assert_eq!(FONT_SIZE, 13.0);
    }

    #[test]
    fn test_generic_with_enum() {
        // This test verifies the type constraints work with enums
        #[derive(Clone, PartialEq)]
        enum TestMode {
            A,
            B,
            C,
        }

        let options = vec![
            (TestMode::A, "Option A"),
            (TestMode::B, "Option B"),
            (TestMode::C, "Option C"),
        ];

        // Verify PartialEq works
        assert!(options[0].0 == TestMode::A);
        assert!(options[1].0 != TestMode::A);

        // Verify Clone works
        let cloned = options[0].0.clone();
        assert!(cloned == TestMode::A);
    }

    #[test]
    fn test_generic_with_strings() {
        // Verify it works with &'static str as well
        let options = vec![
            ("light", "Light Mode"),
            ("dark", "Dark Mode"),
            ("system", "System"),
        ];

        assert_eq!(options[0].0, "light");
        assert_eq!(options[0].1, "Light Mode");
    }
}
