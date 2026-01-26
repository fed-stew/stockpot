//! Animated spinner component for showing loading/processing state.
//!
//! Displays a cycling braille dot animation that can be themed with custom colors.
//! Uses GPUI's entity system with a spawned timer for smooth animation.

use std::time::Duration;

use gpui::{
    div, prelude::*, App, AsyncApp, Context, Entity, IntoElement, Rgba, Styled, WeakEntity, Window,
};

/// Braille dot spinner frames (same as CLI spinner).
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Animation interval in milliseconds.
const FRAME_INTERVAL_MS: u64 = 80;

/// Animated spinner component.
///
/// # Example
/// ```ignore
/// let spinner_entity = cx.new(|cx| Spinner::new(theme.accent, cx));
/// // In render:
/// div().child(spinner_entity.clone())
/// ```
pub struct Spinner {
    /// Current frame index.
    frame_index: usize,
    /// Text color for the spinner.
    color: Rgba,
    /// Whether the animation is running.
    is_running: bool,
}

impl Spinner {
    /// Create a new animated spinner with the given color.
    ///
    /// The spinner will automatically start animating when created.
    pub fn new(color: Rgba, cx: &mut Context<Self>) -> Self {
        let spinner = Self {
            frame_index: 0,
            color,
            is_running: true,
        };

        // Start the animation timer
        Self::start_animation(cx);

        spinner
    }

    /// Start the animation timer.
    fn start_animation(cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Spinner>, cx: &mut AsyncApp| {
            loop {
                // Sleep for the frame interval
                cx.background_executor()
                    .timer(Duration::from_millis(FRAME_INTERVAL_MS))
                    .await;

                // Update frame index and trigger re-render
                let should_continue = this
                    .update(cx, |spinner, cx| {
                        if !spinner.is_running {
                            return false;
                        }
                        spinner.frame_index = (spinner.frame_index + 1) % SPINNER_FRAMES.len();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);

                if !should_continue {
                    break;
                }
            }
        })
        .detach();
    }

    /// Get the current frame character.
    fn current_frame(&self) -> &'static str {
        SPINNER_FRAMES[self.frame_index]
    }
}

/// Get the current spinner frame based on system time (no entity needed).
///
/// Useful for inline rendering where you don't want to manage entity lifecycle.
/// Works best when the parent component is already re-rendering frequently
/// (e.g., during streaming updates).
///
/// # Example
/// ```ignore
/// div()
///     .text_color(theme.accent)
///     .child(current_spinner_frame())
/// ```
pub fn current_spinner_frame() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let frame_index =
        ((millis / FRAME_INTERVAL_MS as u128) % SPINNER_FRAMES.len() as u128) as usize;
    SPINNER_FRAMES[frame_index]
}

impl Render for Spinner {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().text_color(self.color).child(self.current_frame())
    }
}

/// Convenience function to create a spinner element.
///
/// # Example
/// ```ignore
/// // In a component that has access to cx:
/// let spinner = spinner(theme.accent, cx);
/// div().child(spinner)
/// ```
pub fn spinner(color: Rgba, cx: &mut App) -> Entity<Spinner> {
    cx.new(|cx| Spinner::new(color, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_frames_count() {
        assert_eq!(SPINNER_FRAMES.len(), 10);
    }

    #[test]
    fn test_frame_interval() {
        // ~12.5 fps is smooth enough for a spinner
        assert_eq!(FRAME_INTERVAL_MS, 80);
        const _: () = assert!(1000 / FRAME_INTERVAL_MS >= 10); // At least 10 fps
    }
}
