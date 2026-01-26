//! Settings row layout component for consistent settings UI.
//!
//! A horizontal layout wrapper that provides consistent structure for settings:
//! - Left side: label with optional icon, warning indicator, and description
//! - Right side: any control element (toggle, segmented control, etc.)
//!
//! # Example
//! ```ignore
//! settings_row(
//!     "Show Agent Reasoning",
//!     Some("Display the AI's thought process and planned steps"),
//!     None,   // no icon
//!     false,  // no warning
//!     &theme,
//!     toggle("show-reasoning", self.show_reasoning, &theme, |_, _| {}),
//! )
//! ```

use gpui::{div, prelude::*, px, SharedString, Styled};

use crate::gui::theme::Theme;

// =============================================================================
// Constants
// =============================================================================

/// Container vertical padding.
const CONTAINER_PADDING_Y: f32 = 16.0;

/// Gap between label and description.
const LABEL_DESC_GAP: f32 = 4.0;

/// Gap between icon/warning and label text.
const ICON_LABEL_GAP: f32 = 8.0;

/// Label font size.
const LABEL_FONT_SIZE: f32 = 14.0;

/// Description font size.
const DESCRIPTION_FONT_SIZE: f32 = 12.0;

/// Max width for description to prevent it from pushing into control area.
const DESCRIPTION_MAX_WIDTH: f32 = 400.0;

/// Warning icon (orange).
const WARNING_ICON: &str = "⚠️";

// =============================================================================
// Public API
// =============================================================================

/// Render a settings row with label, optional description, and control.
///
/// This is a layout component that ensures consistent horizontal structure
/// for all settings rows in the application.
///
/// # Arguments
/// * `label` - Primary label text for the setting
/// * `description` - Optional longer description text below the label
/// * `icon` - Optional emoji/icon to show before the label
/// * `warning` - If true, shows an orange warning indicator (e.g., for YOLO mode)
/// * `theme` - App theme for colors
/// * `control` - The control element to render on the right side
///
/// # Example
/// ```ignore
/// // Simple toggle setting
/// settings_row(
///     "Auto-save",
///     Some("Automatically save changes"),
///     None,
///     false,
///     &theme,
///     toggle("auto-save", self.auto_save, &theme, |_, _| {}),
/// )
///
/// // Setting with icon and warning
/// settings_row(
///     "YOLO Mode",
///     Some("Auto-accept shell commands without confirmation"),
///     Some("⚡"),  // Lightning icon
///     true,        // Show warning indicator
///     &theme,
///     toggle("yolo-mode", self.yolo_mode, &theme, |_, _| {}),
/// )
///
/// // With segmented control
/// settings_row(
///     "Theme",
///     None,
///     Some("🎨"),
///     false,
///     &theme,
///     segmented_control("theme", theme_options, self.theme, &theme, |_, _, _| {}),
/// )
/// ```
pub fn settings_row(
    label: &'static str,
    description: Option<&'static str>,
    icon: Option<&'static str>,
    warning: bool,
    theme: &Theme,
    control: impl IntoElement,
) -> impl IntoElement {
    let has_description = description.is_some();

    div()
        .w_full()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .py(px(CONTAINER_PADDING_Y))
        .border_b_1()
        .border_color(theme.border)
        // Left side: label area
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(LABEL_DESC_GAP))
                // Label row (icon + warning + label text)
                .child(render_label_row(label, icon, warning, theme))
                // Description (if present)
                .when(has_description, |left| {
                    left.child(
                        div()
                            .text_size(px(DESCRIPTION_FONT_SIZE))
                            .text_color(theme.text_muted)
                            .max_w(px(DESCRIPTION_MAX_WIDTH))
                            .child(description.unwrap_or_default()),
                    )
                }),
        )
        // Right side: control area
        .child(div().flex_shrink_0().child(control))
}

/// Render a settings row with a dynamic label (SharedString instead of &'static str).
///
/// Use this variant when the label needs to be computed at runtime.
///
/// # Arguments
/// * `label` - Primary label text (can be dynamically generated)
/// * `description` - Optional description (also dynamic)
/// * `icon` - Optional emoji/icon
/// * `warning` - If true, shows warning indicator
/// * `theme` - App theme
/// * `control` - The control element
pub fn settings_row_dynamic(
    label: impl Into<SharedString>,
    description: Option<impl Into<SharedString>>,
    icon: Option<&'static str>,
    warning: bool,
    theme: &Theme,
    control: impl IntoElement,
) -> impl IntoElement {
    let label: SharedString = label.into();
    let description: Option<SharedString> = description.map(|d| d.into());
    let has_description = description.is_some();

    div()
        .w_full()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .py(px(CONTAINER_PADDING_Y))
        .border_b_1()
        .border_color(theme.border)
        // Left side: label area
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(LABEL_DESC_GAP))
                // Label row
                .child(render_label_row_dynamic(label, icon, warning, theme))
                // Description (if present)
                .when(has_description, |left| {
                    left.child(
                        div()
                            .text_size(px(DESCRIPTION_FONT_SIZE))
                            .text_color(theme.text_muted)
                            .max_w(px(DESCRIPTION_MAX_WIDTH))
                            .child(description.unwrap_or_default()),
                    )
                }),
        )
        // Right side: control area
        .child(div().flex_shrink_0().child(control))
}

/// Render a settings row without the bottom border.
///
/// Useful for the last item in a section where you don't want a trailing border.
pub fn settings_row_no_border(
    label: &'static str,
    description: Option<&'static str>,
    icon: Option<&'static str>,
    warning: bool,
    theme: &Theme,
    control: impl IntoElement,
) -> impl IntoElement {
    let has_description = description.is_some();

    div()
        .w_full()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .py(px(CONTAINER_PADDING_Y))
        // No border!
        // Left side: label area
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(LABEL_DESC_GAP))
                .child(render_label_row(label, icon, warning, theme))
                .when(has_description, |left| {
                    left.child(
                        div()
                            .text_size(px(DESCRIPTION_FONT_SIZE))
                            .text_color(theme.text_muted)
                            .max_w(px(DESCRIPTION_MAX_WIDTH))
                            .child(description.unwrap_or_default()),
                    )
                }),
        )
        // Right side: control area
        .child(div().flex_shrink_0().child(control))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Render the label row with optional icon and warning indicator.
fn render_label_row(
    label: &'static str,
    icon: Option<&'static str>,
    warning: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(ICON_LABEL_GAP))
        // Icon (if present)
        .when_some(icon, |row, icon| {
            row.child(div().text_size(px(LABEL_FONT_SIZE)).child(icon))
        })
        // Warning indicator (if enabled)
        .when(warning, |row| {
            row.child(
                div()
                    .text_size(px(LABEL_FONT_SIZE - 2.0)) // Slightly smaller
                    .text_color(theme.warning_icon)
                    .child(WARNING_ICON),
            )
        })
        // Label text
        .child(
            div()
                .text_size(px(LABEL_FONT_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(label),
        )
}

/// Render the label row with dynamic SharedString label.
fn render_label_row_dynamic(
    label: SharedString,
    icon: Option<&'static str>,
    warning: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(ICON_LABEL_GAP))
        // Icon (if present)
        .when_some(icon, |row, icon| {
            row.child(div().text_size(px(LABEL_FONT_SIZE)).child(icon))
        })
        // Warning indicator (if enabled)
        .when(warning, |row| {
            row.child(
                div()
                    .text_size(px(LABEL_FONT_SIZE - 2.0))
                    .text_color(theme.warning_icon)
                    .child(WARNING_ICON),
            )
        })
        // Label text
        .child(
            div()
                .text_size(px(LABEL_FONT_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(label),
        )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_padding() {
        assert_eq!(CONTAINER_PADDING_Y, 16.0);
    }

    #[test]
    fn test_font_sizes() {
        assert_eq!(LABEL_FONT_SIZE, 14.0);
        assert_eq!(DESCRIPTION_FONT_SIZE, 12.0);
        // Description should be smaller than label
        assert!(DESCRIPTION_FONT_SIZE < LABEL_FONT_SIZE);
    }

    #[test]
    fn test_description_max_width() {
        // Should have a reasonable max width to prevent overflow
        assert_eq!(DESCRIPTION_MAX_WIDTH, 400.0);
        assert!(DESCRIPTION_MAX_WIDTH > 200.0); // Not too narrow
        assert!(DESCRIPTION_MAX_WIDTH < 600.0); // Not too wide
    }

    #[test]
    fn test_warning_icon() {
        assert_eq!(WARNING_ICON, "⚠️");
    }

    #[test]
    fn test_warning_icon_exists() {
        // Warning icon should be the warning emoji
        assert_eq!(WARNING_ICON, "⚠️");
    }

    #[test]
    fn test_gaps() {
        assert_eq!(LABEL_DESC_GAP, 4.0);
        assert_eq!(ICON_LABEL_GAP, 8.0);
    }
}
