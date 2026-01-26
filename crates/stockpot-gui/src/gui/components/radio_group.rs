//! Radio group component for mutually exclusive options with descriptions.
//!
//! A vertical list of radio options, each with optional description and icon.
//! Uses subtle styling: thin border for selection rather than solid fill.
//!
//! # Example
//! ```ignore
//! #[derive(Clone, PartialEq)]
//! enum PdfMode {
//!     Fast,
//!     Accurate,
//!     Hybrid,
//! }
//!
//! radio_group(
//!     "pdf-mode",
//!     vec![
//!         RadioOption {
//!             value: PdfMode::Fast,
//!             label: "Fast Mode",
//!             description: Some("Quick processing, may miss some details"),
//!             icon: Some("⚡"),
//!         },
//!         RadioOption {
//!             value: PdfMode::Accurate,
//!             label: "Accurate Mode",
//!             description: Some("Thorough processing, slower but complete"),
//!             icon: Some("🎯"),
//!         },
//!     ],
//!     self.pdf_mode.clone(),
//!     &theme,
//!     |mode, _window, _cx| {
//!         // Handle selection
//!     },
//! )
//! ```

use std::rc::Rc;

use gpui::{div, prelude::*, px, App, Hsla, MouseButton, Rgba, SharedString, Styled, Window};

use crate::gui::theme::Theme;

// =============================================================================
// Constants
// =============================================================================

/// Gap between option rows.
const OPTION_GAP: f32 = 8.0;

/// Option row padding.
const OPTION_PADDING_X: f32 = 12.0;
const OPTION_PADDING_Y: f32 = 10.0;

/// Option row border radius.
const OPTION_BORDER_RADIUS: f32 = 8.0;

/// Gap between radio circle and content.
const RADIO_CONTENT_GAP: f32 = 12.0;

/// Radio circle dimensions.
const RADIO_OUTER_SIZE: f32 = 18.0;
const RADIO_INNER_SIZE: f32 = 8.0;
#[allow(dead_code)] // Used in tests for documentation
const RADIO_BORDER_WIDTH: f32 = 1.0;

/// Font sizes.
const LABEL_FONT_SIZE: f32 = 13.0;
const DESCRIPTION_FONT_SIZE: f32 = 12.0;

/// Selected background opacity (8% tint).
const SELECTED_BG_OPACITY: f32 = 0.08;

// =============================================================================
// Types
// =============================================================================

/// A single radio option with optional description and icon.
#[derive(Clone)]
pub struct RadioOption<T> {
    /// The value this option represents.
    pub value: T,
    /// Display label for the option.
    pub label: &'static str,
    /// Optional longer description text.
    pub description: Option<&'static str>,
    /// Optional emoji or icon character.
    pub icon: Option<&'static str>,
}

impl<T> RadioOption<T> {
    /// Create a simple option with just a label.
    pub fn new(value: T, label: &'static str) -> Self {
        Self {
            value,
            label,
            description: None,
            icon: None,
        }
    }

    /// Builder: add a description.
    pub fn with_description(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    /// Builder: add an icon.
    pub fn with_icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert Rgba to Hsla with a specific opacity.
fn with_opacity(color: Rgba, opacity: f32) -> Hsla {
    let hsla: Hsla = color.into();
    hsla.opacity(opacity)
}

/// Render the radio circle indicator.
///
/// - Unselected: empty circle with border
/// - Selected: circle with border and filled inner dot
fn render_radio_circle(is_selected: bool, theme: &Theme) -> impl IntoElement {
    let border_color = if is_selected {
        theme.accent
    } else {
        theme.border
    };

    div()
        .flex_shrink_0()
        .w(px(RADIO_OUTER_SIZE))
        .h(px(RADIO_OUTER_SIZE))
        .rounded(px(RADIO_OUTER_SIZE / 2.0)) // Fully rounded (circle)
        .border_1()
        .border_color(border_color)
        .flex()
        .items_center()
        .justify_center()
        // Inner filled circle (only when selected)
        .when(is_selected, |circle| {
            circle.child(
                div()
                    .w(px(RADIO_INNER_SIZE))
                    .h(px(RADIO_INNER_SIZE))
                    .rounded(px(RADIO_INNER_SIZE / 2.0))
                    .bg(theme.accent),
            )
        })
}

/// Render a single option row.
fn render_option_row<T, F>(
    option: RadioOption<T>,
    index: usize,
    base_id: &SharedString,
    is_selected: bool,
    theme: &Theme,
    on_select: F,
) -> impl IntoElement
where
    T: Clone + 'static,
    F: Fn(T, &mut Window, &mut App) + 'static,
{
    let row_id = SharedString::from(format!("{}-{}", base_id, index));
    let value_for_click = option.value.clone();
    let has_description = option.description.is_some();

    // Colors based on selection state
    let (bg_color, border_color, label_color) = if is_selected {
        (
            with_opacity(theme.accent, SELECTED_BG_OPACITY),
            theme.accent,
            theme.accent,
        )
    } else {
        (Hsla::transparent_black(), theme.border, theme.text)
    };

    div()
        .id(row_id)
        .w_full()
        .px(px(OPTION_PADDING_X))
        .py(px(OPTION_PADDING_Y))
        .rounded(px(OPTION_BORDER_RADIUS))
        .border_1()
        .border_color(border_color)
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(RADIO_CONTENT_GAP))
        // Click handler
        .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
            on_select(value_for_click.clone(), window, cx);
        })
        // Radio circle
        .child(render_radio_circle(is_selected, theme))
        // Content (icon + label + description)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                // Label row (with optional icon)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        // Icon (if present)
                        .when_some(option.icon, |row, icon| {
                            row.child(div().text_size(px(LABEL_FONT_SIZE)).child(icon))
                        })
                        // Label
                        .child(
                            div()
                                .text_size(px(LABEL_FONT_SIZE))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(label_color)
                                .child(option.label),
                        ),
                )
                // Description (if present)
                .when(has_description, |content| {
                    content.child(
                        div()
                            .text_size(px(DESCRIPTION_FONT_SIZE))
                            .text_color(theme.text_muted)
                            .child(option.description.unwrap_or_default()),
                    )
                }),
        )
}

// =============================================================================
// Public API
// =============================================================================

/// Render a radio group component.
///
/// # Arguments
/// * `id` - Base identifier for this group (options will have IDs like "id-0", "id-1", etc.)
/// * `options` - Vector of RadioOption defining the available choices
/// * `selected` - Currently selected value
/// * `theme` - App theme for colors
/// * `on_select` - Callback invoked when an option is clicked, receives the selected value.
///   The parent component is responsible for updating state and calling `cx.notify()`.
///
/// # Type Parameters
/// * `T` - The value type. Must be `PartialEq` (for comparison) and `Clone` (for callback).
///
/// # Example
/// ```ignore
/// radio_group(
///     "processing-mode",
///     vec![
///         RadioOption::new(Mode::Fast, "Fast")
///             .with_description("Quick but less accurate")
///             .with_icon("⚡"),
///         RadioOption::new(Mode::Slow, "Thorough")
///             .with_description("Slower but more accurate")
///             .with_icon("🔍"),
///     ],
///     self.mode.clone(),
///     &theme,
///     |mode, _window, _cx| {
///         // Handle selection
///     },
/// )
/// ```
pub fn radio_group<T, F>(
    id: impl Into<SharedString>,
    options: Vec<RadioOption<T>>,
    selected: T,
    theme: &Theme,
    on_select: F,
) -> impl IntoElement
where
    T: PartialEq + Clone + 'static,
    F: Fn(T, &mut Window, &mut App) + 'static,
{
    let id: SharedString = id.into();
    // Wrap callback in Rc so it can be shared across options
    let on_select = Rc::new(on_select);

    // Build option rows
    let rows: Vec<_> = options
        .into_iter()
        .enumerate()
        .map(|(index, option)| {
            let is_selected = option.value == selected;
            let on_select = Rc::clone(&on_select);
            render_option_row(option, index, &id, is_selected, theme, move |v, w, cx| {
                on_select(v, w, cx)
            })
        })
        .collect();

    // Container
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(OPTION_GAP))
        .children(rows)
}

/// Render a radio group with a label.
///
/// Convenience wrapper that places a label above the radio group.
///
/// # Arguments
/// * `id` - Base identifier for this group
/// * `label` - Text label displayed above the group
/// * `options` - Vector of RadioOption
/// * `selected` - Currently selected value
/// * `theme` - App theme for styling
/// * `on_select` - Callback invoked when an option is clicked
///
/// # Example
/// ```ignore
/// labeled_radio_group(
///     "pdf-mode",
///     "PDF Processing Mode",
///     vec![
///         RadioOption::new(PdfMode::Fast, "Fast").with_icon("⚡"),
///         RadioOption::new(PdfMode::Accurate, "Accurate").with_icon("🎯"),
///     ],
///     self.pdf_mode.clone(),
///     &theme,
///     |mode, _window, _cx| { /* ... */ },
/// )
/// ```
pub fn labeled_radio_group<T, F>(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    options: Vec<RadioOption<T>>,
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
                .text_size(px(LABEL_FONT_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(label),
        )
        .child(radio_group(id, options, selected, theme, on_select))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_gap() {
        assert_eq!(OPTION_GAP, 8.0);
    }

    #[test]
    fn test_radio_circle_dimensions() {
        assert_eq!(RADIO_OUTER_SIZE, 18.0);
        assert_eq!(RADIO_INNER_SIZE, 8.0);
        // Inner circle should fit inside outer
        assert!(RADIO_INNER_SIZE < RADIO_OUTER_SIZE - (RADIO_BORDER_WIDTH * 2.0));
    }

    #[test]
    fn test_selected_bg_opacity() {
        // Should be a subtle tint, not a solid fill
        assert_eq!(SELECTED_BG_OPACITY, 0.08);
        assert!(SELECTED_BG_OPACITY < 0.2); // Ensure it's subtle
    }

    #[test]
    fn test_radio_option_builder() {
        #[derive(Clone, PartialEq)]
        enum TestValue {
            A,
        }

        let option = RadioOption::new(TestValue::A, "Test Label")
            .with_description("A description")
            .with_icon("🔥");

        assert_eq!(option.label, "Test Label");
        assert_eq!(option.description, Some("A description"));
        assert_eq!(option.icon, Some("🔥"));
    }

    #[test]
    fn test_radio_option_simple() {
        let option = RadioOption::new("value", "Simple Label");

        assert_eq!(option.value, "value");
        assert_eq!(option.label, "Simple Label");
        assert!(option.description.is_none());
        assert!(option.icon.is_none());
    }

    #[test]
    fn test_font_sizes() {
        assert_eq!(LABEL_FONT_SIZE, 13.0);
        assert_eq!(DESCRIPTION_FONT_SIZE, 12.0);
        // Description should be slightly smaller
        assert!(DESCRIPTION_FONT_SIZE < LABEL_FONT_SIZE);
    }
}
