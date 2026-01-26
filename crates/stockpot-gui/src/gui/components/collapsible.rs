//! Collapsible container component for nested agent output.
//!
//! Provides a toggleable container with header and body sections.
//! Used primarily for displaying sub-agent invocations in a compact,
//! expandable format.
//!
//! # Example
//! ```ignore
//! collapsible(
//!     CollapsibleProps {
//!         id: "agent-123".into(),
//!         title: "Code Review Agent".into(),
//!         is_collapsed: false,
//!         is_loading: true,
//!         ..CollapsibleProps::with_theme(&theme)
//!     },
//!     div().child("Agent output content here..."),
//!     |_, _| { /* toggle handler */ },
//! )
//! ```

use gpui::{div, prelude::*, px, Hsla, MouseButton, Rgba, SharedString, Styled};

use super::current_spinner_frame;

/// Chevron indicators for collapsed/expanded states.
const CHEVRON_COLLAPSED: &str = "▶";
const CHEVRON_EXPANDED: &str = "▼";

/// Helper to apply opacity to an Rgba color.
///
/// Converts to Hsla, applies opacity, then returns the Hsla.
fn with_opacity(color: Rgba, opacity: f32) -> Hsla {
    let hsla: Hsla = color.into();
    hsla.opacity(opacity)
}

/// Props for configuring the collapsible component.
///
/// Use `CollapsibleProps::with_theme()` for sensible defaults based on the app theme.
#[derive(Clone)]
pub struct CollapsibleProps {
    /// Unique ID for this collapsible (used for GPUI element identification).
    pub id: SharedString,
    /// Title shown in the header (e.g., agent display name).
    pub title: SharedString,
    /// Whether the content is currently collapsed (hidden).
    pub is_collapsed: bool,
    /// Whether the content is still streaming/loading.
    pub is_loading: bool,
    /// Accent color for the left edge bar.
    pub border_color: Rgba,
    /// Primary text color.
    pub text_color: Rgba,
    /// Muted text color (for chevron and secondary elements).
    pub muted_color: Rgba,
}

impl Default for CollapsibleProps {
    fn default() -> Self {
        // Dark theme defaults matching our app theme
        Self {
            id: "collapsible".into(),
            title: "Collapsible".into(),
            is_collapsed: true,
            is_loading: false,
            border_color: gpui::rgb(0x0078d4),
            text_color: gpui::rgb(0xcccccc),
            muted_color: gpui::rgb(0x808080),
        }
    }
}

impl CollapsibleProps {
    /// Create props with colors derived from the app theme.
    ///
    /// This is the preferred constructor to ensure visual consistency.
    pub fn with_theme(theme: &crate::gui::theme::Theme) -> Self {
        Self {
            id: "collapsible".into(),
            title: "Collapsible".into(),
            is_collapsed: true,
            is_loading: false,
            border_color: theme.accent,
            text_color: theme.text,
            muted_color: theme.text_muted,
        }
    }

    /// Builder: set the unique ID.
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// Builder: set the title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Builder: set collapsed state.
    pub fn collapsed(mut self, is_collapsed: bool) -> Self {
        self.is_collapsed = is_collapsed;
        self
    }

    /// Builder: set loading state.
    pub fn loading(mut self, is_loading: bool) -> Self {
        self.is_loading = is_loading;
        self
    }
}

/// Render a collapsible container.
///
/// # Arguments
/// * `props` - Styling and state configuration.
/// * `content` - The content to show when expanded.
/// * `on_toggle` - Callback when header is clicked to toggle collapsed state.
///
/// # Example
/// ```ignore
/// collapsible(
///     CollapsibleProps::with_theme(&theme)
///         .id("my-collapsible")
///         .title("Section Title")
///         .collapsed(is_collapsed),
///     div().child("Hidden content here"),
///     |window, cx| {
///         // toggle logic here
///     },
/// )
/// ```
pub fn collapsible<E, F>(props: CollapsibleProps, content: E, on_toggle: F) -> impl IntoElement
where
    E: IntoElement + 'static,
    F: Fn(&mut gpui::Window, &mut gpui::App) + 'static,
{
    let is_collapsed = props.is_collapsed;
    let border_color = props.border_color;

    // Clean container - no border, no rounded corners
    div()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        // Header section (always visible)
        .child(render_header(&props, Some(on_toggle)))
        // Body section with left accent bar
        .when(!is_collapsed, |container| {
            container.child(
                div()
                    .flex()
                    .w_full()
                    // Left accent bar
                    .child(
                        div()
                            .w(px(2.))
                            .ml(px(6.)) // Align with chevron
                            .bg(with_opacity(border_color, 0.4)),
                    )
                    // Content
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pl(px(10.))
                            .py(px(4.))
                            .child(content),
                    ),
            )
        })
}

/// Render a collapsible container in display-only mode.
///
/// This version does NOT handle click events internally. Use this when you need
/// to handle toggle logic at a higher level (e.g., with cx.listener() in a gpui app).
///
/// The header will still show the clickable cursor style for visual consistency.
///
/// # Arguments
/// * `props` - Styling and state configuration.
/// * `content` - The content to show when expanded.
///
/// # Example
/// ```ignore
/// // In a gpui component render method:
/// div()
///     .id("my-section")
///     .on_click(cx.listener(|this, _, _, cx| {
///         this.is_collapsed = !this.is_collapsed;
///         cx.notify();
///     }))
///     .child(collapsible_display(
///         CollapsibleProps::with_theme(&theme)
///             .id("my-collapsible")
///             .title("Section Title")
///             .collapsed(is_collapsed),
///         div().child("Hidden content here"),
///     ))
/// ```
pub fn collapsible_display<E>(props: CollapsibleProps, content: E) -> impl IntoElement
where
    E: IntoElement + 'static,
{
    let is_collapsed = props.is_collapsed;
    let border_color = props.border_color;

    // Clean container - no border, no rounded corners
    div()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        // Header section (always visible, no click handler)
        .child(render_header::<fn(&mut gpui::Window, &mut gpui::App)>(
            &props, None,
        ))
        // Body section with left accent bar
        .when(!is_collapsed, |container| {
            container.child(
                div()
                    .flex()
                    .w_full()
                    // Left accent bar
                    .child(
                        div()
                            .w(px(2.))
                            .ml(px(6.)) // Align with chevron
                            .bg(with_opacity(border_color, 0.4)),
                    )
                    // Content
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pl(px(10.))
                            .py(px(4.))
                            .child(content),
                    ),
            )
        })
}

/// Render the clickable header section.
fn render_header<F>(props: &CollapsibleProps, on_toggle: Option<F>) -> impl IntoElement
where
    F: Fn(&mut gpui::Window, &mut gpui::App) + 'static,
{
    let chevron = if props.is_collapsed {
        CHEVRON_COLLAPSED
    } else {
        CHEVRON_EXPANDED
    };

    let text_color = props.text_color;
    let muted_color = props.muted_color;
    let title = props.title.clone();
    let id = props.id.clone();
    let is_loading = props.is_loading;

    div()
        .id(SharedString::from(format!("{}-header", id)))
        .flex()
        .items_center()
        .gap(px(6.))
        .py(px(4.))
        .cursor_pointer()
        .hover(|s| s.opacity(0.7))
        // Only add click handler if on_toggle is provided
        .when_some(on_toggle, |header, toggle_fn| {
            header.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                toggle_fn(window, cx);
            })
        })
        // Chevron indicator (muted)
        .child(
            div()
                .text_size(px(9.))
                .text_color(muted_color)
                .child(chevron),
        )
        // Title
        .child(
            div()
                .flex_1()
                .text_size(px(12.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(text_color)
                .child(title),
        )
        // Loading spinner (if streaming)
        .when(is_loading, |header| {
            header.child(
                div()
                    .text_size(px(11.))
                    .text_color(muted_color)
                    .child(current_spinner_frame()),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_props() {
        let props = CollapsibleProps::default();
        assert!(props.is_collapsed);
        assert!(!props.is_loading);
    }

    #[test]
    fn test_builder_pattern() {
        let props = CollapsibleProps::default()
            .id("test-id")
            .title("Test Title")
            .collapsed(false)
            .loading(true);

        assert_eq!(props.id.as_ref(), "test-id");
        assert_eq!(props.title.as_ref(), "Test Title");
        assert!(!props.is_collapsed);
        assert!(props.is_loading);
    }

    #[test]
    fn test_chevron_constants() {
        // Visual sanity check
        assert_eq!(CHEVRON_COLLAPSED, "▶");
        assert_eq!(CHEVRON_EXPANDED, "▼");
    }
}
