//! Selectable list item component for list-based selection interfaces.
//!
//! Provides consistent styling for selectable items in lists (agent lists, model lists, etc.)
//! Uses subtle selection styling: thin border + tint, NOT solid fill.
//!
//! # Example
//! ```ignore
//! selectable_list_item(
//!     "agent-code-puppy",
//!     self.selected_agent == "code-puppy",
//!     &theme,
//!     |_window, _cx| { /* handle selection */ },
//!     div().child("Code Puppy 🐶"),
//! )
//! ```

use gpui::{div, prelude::*, px, App, Hsla, MouseButton, Rgba, SharedString, Styled, Window};

use crate::gui::theme::Theme;

// =============================================================================
// Constants
// =============================================================================

/// Item horizontal padding.
const ITEM_PADDING_X: f32 = 12.0;

/// Item vertical padding.
const ITEM_PADDING_Y: f32 = 10.0;

/// Item border radius.
const ITEM_BORDER_RADIUS: f32 = 8.0;

/// Selected background opacity (8% tint).
const SELECTED_BG_OPACITY: f32 = 0.08;

/// Label font size.
const LABEL_FONT_SIZE: f32 = 13.0;

/// Subtitle font size.
const SUBTITLE_FONT_SIZE: f32 = 11.0;

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert Rgba to Hsla with a specific opacity.
fn with_opacity(color: Rgba, opacity: f32) -> Hsla {
    let hsla: Hsla = color.into();
    hsla.opacity(opacity)
}

// =============================================================================
// Public API
// =============================================================================

/// Render a selectable list item with custom content.
///
/// This is the most flexible variant - you provide any content as a child element.
/// Uses subtle selection styling (thin border + tint) rather than solid fill.
///
/// # Arguments
/// * `id` - Unique identifier for this item
/// * `selected` - Whether this item is currently selected
/// * `theme` - App theme for colors
/// * `on_click` - Callback invoked when the item is clicked
/// * `content` - The content to render inside the item
///
/// # Styling
/// - Unselected: `theme.tool_card` background, no visible border
/// - Selected: 8% accent tint background, thin accent border
///
/// # Example
/// ```ignore
/// selectable_list_item(
///     "item-1",
///     is_selected,
///     &theme,
///     |_window, _cx| { /* click handler */ },
///     div()
///         .flex()
///         .gap(px(8.))
///         .child("🐶")
///         .child("My Custom Content"),
/// )
/// ```
pub fn selectable_list_item<F>(
    id: impl Into<SharedString>,
    selected: bool,
    theme: &Theme,
    on_click: F,
    content: impl IntoElement,
) -> impl IntoElement
where
    F: Fn(&mut Window, &mut App) + 'static,
{
    let id: SharedString = id.into();

    // Colors based on selection state
    let (bg_color, border_color) = if selected {
        (
            with_opacity(theme.accent, SELECTED_BG_OPACITY),
            theme.accent,
        )
    } else {
        (
            // Use tool_card as Hsla for consistency
            with_opacity(theme.tool_card, 1.0),
            theme.tool_card, // Border same as bg = invisible
        )
    };

    div()
        .id(id)
        .w_full()
        .px(px(ITEM_PADDING_X))
        .py(px(ITEM_PADDING_Y))
        .rounded(px(ITEM_BORDER_RADIUS))
        .border_1()
        .border_color(border_color)
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
            on_click(window, cx);
        })
        .child(content)
}

/// Render a selectable list item with label and optional subtitle.
///
/// Convenience variant with built-in label/subtitle layout.
///
/// # Arguments
/// * `id` - Unique identifier for this item
/// * `label` - Primary label text
/// * `subtitle` - Optional secondary text (displayed below label in muted color)
/// * `selected` - Whether this item is currently selected
/// * `theme` - App theme for colors
/// * `on_click` - Callback invoked when the item is clicked
///
/// # Example
/// ```ignore
/// selectable_list_item_with_subtitle(
///     "model-gpt4",
///     "GPT-4 Turbo",
///     Some("openai/gpt-4-turbo"),
///     self.selected_model == "gpt-4-turbo",
///     &theme,
///     |_window, _cx| { /* click handler */ },
/// )
/// ```
pub fn selectable_list_item_with_subtitle<F>(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    selected: bool,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&mut Window, &mut App) + 'static,
{
    let label: SharedString = label.into();
    let subtitle: Option<SharedString> = subtitle.map(|s| s.into());
    let has_subtitle = subtitle.is_some();

    // Label color: accent when selected, normal text otherwise
    let label_color = if selected { theme.accent } else { theme.text };

    let content = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        // Label
        .child(
            div()
                .text_size(px(LABEL_FONT_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(label_color)
                .child(label),
        )
        // Subtitle (if present)
        .when(has_subtitle, |container| {
            container.child(
                div()
                    .text_size(px(SUBTITLE_FONT_SIZE))
                    .text_color(theme.text_muted)
                    .child(subtitle.clone().unwrap_or_default()),
            )
        });

    selectable_list_item(id, selected, theme, on_click, content)
}

/// Render a selectable list item with icon, label, and optional subtitle.
///
/// Variant with icon prefix for items that need visual indicators.
///
/// # Arguments
/// * `id` - Unique identifier for this item
/// * `icon` - Icon/emoji to display before the label
/// * `label` - Primary label text
/// * `subtitle` - Optional secondary text
/// * `selected` - Whether this item is currently selected
/// * `theme` - App theme for colors
/// * `on_click` - Callback invoked when the item is clicked
///
/// # Example
/// ```ignore
/// selectable_list_item_with_icon(
///     "agent-code-puppy",
///     "🐶",
///     "Code Puppy",
///     Some("Your helpful coding assistant"),
///     self.selected_agent == "code-puppy",
///     &theme,
///     |_window, _cx| { /* click handler */ },
/// )
/// ```
pub fn selectable_list_item_with_icon<F>(
    id: impl Into<SharedString>,
    icon: &'static str,
    label: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    selected: bool,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&mut Window, &mut App) + 'static,
{
    let label: SharedString = label.into();
    let subtitle: Option<SharedString> = subtitle.map(|s| s.into());
    let has_subtitle = subtitle.is_some();

    // Label color: accent when selected, normal text otherwise
    let label_color = if selected { theme.accent } else { theme.text };

    let content = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        // Icon
        .child(
            div()
                .text_size(px(LABEL_FONT_SIZE + 2.0)) // Slightly larger icon
                .child(icon),
        )
        // Label + subtitle column
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                // Label
                .child(
                    div()
                        .text_size(px(LABEL_FONT_SIZE))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(label_color)
                        .child(label),
                )
                // Subtitle (if present)
                .when(has_subtitle, |container| {
                    container.child(
                        div()
                            .text_size(px(SUBTITLE_FONT_SIZE))
                            .text_color(theme.text_muted)
                            .child(subtitle.clone().unwrap_or_default()),
                    )
                }),
        );

    selectable_list_item(id, selected, theme, on_click, content)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_padding() {
        assert_eq!(ITEM_PADDING_X, 12.0);
        assert_eq!(ITEM_PADDING_Y, 10.0);
    }

    #[test]
    fn test_item_border_radius() {
        assert_eq!(ITEM_BORDER_RADIUS, 8.0);
    }

    #[test]
    fn test_selected_bg_opacity() {
        // Should be a subtle tint, not a solid fill
        assert_eq!(SELECTED_BG_OPACITY, 0.08);
        assert!(SELECTED_BG_OPACITY < 0.2); // Ensure it's subtle
    }

    #[test]
    fn test_font_sizes() {
        assert_eq!(LABEL_FONT_SIZE, 13.0);
        assert_eq!(SUBTITLE_FONT_SIZE, 11.0);
        // Subtitle should be smaller than label
        assert!(SUBTITLE_FONT_SIZE < LABEL_FONT_SIZE);
    }
}
