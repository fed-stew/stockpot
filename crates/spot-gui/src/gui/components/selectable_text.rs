//! Selectable text component for read-only text that can be selected and copied
//!
//! Provides a text display component with full mouse-based text selection
//! and system copy (Cmd+C) support.

use std::ops::Range;
use std::time::Instant;

use gpui::{
    div, prelude::*, App, ClipboardItem, Context, CursorStyle, FocusHandle, Focusable, Hsla,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point, SharedString,
    StyledText, TextRun, Window,
};

use crate::gui::theme::Theme;

use super::markdown_text;

gpui::actions!(selectable_text, [Copy, SelectAll]);

/// Duration of the trailing-edge fade-in for newly appended text (milliseconds).
/// Only the last few characters (the "leading edge") are affected — older text
/// stays at full opacity.
const TRAILING_FADE_MS: f32 = 150.0;

/// Selectable text component for displaying text that can be selected and copied
pub struct SelectableText {
    focus_handle: FocusHandle,
    content: SharedString,
    selected_range: Range<usize>,
    is_selecting: bool,
    theme: Theme,
    drag_start_offset: usize,
    drag_start_position: Option<Point<gpui::Pixels>>,
    element_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    cached_rendered: Option<markdown_text::RenderedMarkdown>,
    cached_content: SharedString,
    cached_font_size: gpui::Pixels,
    /// Whether the mouse is currently hovering over a link
    hovering_link: bool,
    /// Mutable buffer for efficient append operations (avoids SharedString round-trips)
    content_buffer: String,
    /// Whether `content_buffer` has new data not yet reflected in `content`
    content_dirty: bool,
    /// Byte length of source text that has been stably rendered (complete lines)
    stable_source_len: usize,
    /// Cached rendered output for the stable prefix (all complete lines)
    stable_rendered: Option<markdown_text::RenderedMarkdown>,
    /// Number of trailing source bytes that are "new" and should fade in.
    /// Set by `append()` and `set_content_with_trailing_fade()`.
    fade_trailing_bytes: usize,
    /// When the current trailing fade started. Cleared when the fade completes.
    fade_trailing_start: Option<Instant>,
}

impl SelectableText {
    pub fn new(cx: &mut Context<Self>, content: impl Into<SharedString>, theme: Theme) -> Self {
        let content: SharedString = content.into();
        let content_buffer = content.to_string();
        Self {
            focus_handle: cx.focus_handle(),
            content,
            selected_range: 0..0,
            is_selecting: false,
            theme,
            drag_start_offset: 0,
            drag_start_position: None,
            element_bounds: None,
            cached_rendered: None,
            cached_content: SharedString::from(""),
            cached_font_size: gpui::Pixels::ZERO,
            hovering_link: false,
            content_buffer,
            content_dirty: false,
            stable_source_len: 0,
            stable_rendered: None,
            fade_trailing_bytes: 0,
            fade_trailing_start: None,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = content.into();
        self.content_buffer = self.content.to_string();
        self.content_dirty = false;
        self.selected_range = 0..0;
        self.cached_rendered = None;
        self.cached_content = SharedString::from("");
        self.stable_source_len = 0;
        self.stable_rendered = None;
        self.fade_trailing_bytes = 0;
        self.fade_trailing_start = None;
        cx.notify();
    }

    /// Replace content and mark the last `trailing_fade_bytes` as a fade-in zone.
    /// Used by the streaming pending view to fade only the newly added characters.
    pub fn set_content_with_trailing_fade(
        &mut self,
        content: impl Into<SharedString>,
        trailing_fade_bytes: usize,
        cx: &mut Context<Self>,
    ) {
        self.content = content.into();
        self.content_buffer = self.content.to_string();
        self.content_dirty = false;
        self.selected_range = 0..0;
        self.cached_rendered = None;
        self.cached_content = SharedString::from("");
        self.stable_source_len = 0;
        self.stable_rendered = None;
        self.fade_trailing_bytes = trailing_fade_bytes;
        self.fade_trailing_start = Some(Instant::now());
        cx.notify();
    }

    pub fn append(&mut self, delta: &str, cx: &mut Context<Self>) {
        self.content_buffer.push_str(delta);
        self.content_dirty = true;
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        let rendered = markdown_text::render_markdown(
            self.content.as_ref(),
            &window.text_style(),
            &self.theme,
        );
        self.selected_range = 0..rendered.text.len();
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            return;
        }

        let rendered = markdown_text::render_markdown(
            self.content.as_ref(),
            &window.text_style(),
            &self.theme,
        );
        let start = self.selected_range.start.min(rendered.text.len());
        let end = self.selected_range.end.min(rendered.text.len());
        if start >= end {
            return;
        }

        cx.write_to_clipboard(ClipboardItem::new_string(
            rendered.text[start..end].to_string(),
        ));
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.hit_test_position(event.position, window);

        // Check if clicking on a link (single click, no modifiers)
        if event.click_count == 1 && !event.modifiers.shift {
            if let Some(url) = self.get_link_at_offset(offset, window) {
                // Open URL and don't start selection
                let _ = open::that(&url);
                return;
            }
        }

        self.is_selecting = true;
        self.drag_start_position = Some(event.position);
        self.focus_handle.focus(window, cx);

        if event.modifiers.shift {
            self.select_to(offset);
        } else if event.click_count == 2 {
            let rendered = markdown_text::render_markdown(
                self.content.as_ref(),
                &window.text_style(),
                &self.theme,
            );
            let (start, end) = Self::word_boundaries(rendered.text.as_ref(), offset);
            self.selected_range = start..end;
            self.drag_start_offset = start;
        } else if event.click_count == 3 {
            let rendered = markdown_text::render_markdown(
                self.content.as_ref(),
                &window.text_style(),
                &self.theme,
            );
            self.selected_range = 0..rendered.text.len();
            self.drag_start_offset = 0;
        } else {
            self.selected_range = offset..offset;
            self.drag_start_offset = offset;
        }
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Check if hovering over a link (when not dragging)
        if event.pressed_button.is_none() {
            let offset = self.hit_test_position(event.position, window);
            let over_link = self.get_link_at_offset(offset, window).is_some();
            if over_link != self.hovering_link {
                self.hovering_link = over_link;
                cx.notify();
            }
        }

        if self.is_selecting && event.pressed_button == Some(MouseButton::Left) {
            const DRAG_THRESHOLD: f32 = 3.0;

            if let Some(start_pos) = self.drag_start_position {
                let dx = (event.position.x - start_pos.x).abs();
                let dy = (event.position.y - start_pos.y).abs();
                let threshold = gpui::Pixels::from(DRAG_THRESHOLD);

                if dx < threshold && dy < threshold {
                    return;
                }
            }

            let offset = self.hit_test_position(event.position, window);
            self.select_to(offset);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_selecting = false;
        self.drag_start_position = None;
    }

    fn select_to(&mut self, offset: usize) {
        if offset < self.drag_start_offset {
            self.selected_range = offset..self.drag_start_offset;
        } else {
            self.selected_range = self.drag_start_offset..offset;
        }
    }

    fn hit_test_position(&self, position: Point<gpui::Pixels>, window: &Window) -> usize {
        let text_style = window.text_style();
        let rendered =
            markdown_text::render_markdown(self.content.as_ref(), &text_style, &self.theme);
        if rendered.text.is_empty() {
            return 0;
        }

        let Some(bounds) = self.element_bounds else {
            return 0;
        };

        let relative_pos = position - bounds.origin;
        if relative_pos.x < gpui::Pixels::ZERO || relative_pos.y < gpui::Pixels::ZERO {
            return 0;
        }

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = if window.line_height() == gpui::Pixels::ZERO {
            // Fallback: use font_size * 1.3 as a reasonable line height
            font_size * 1.3
        } else {
            window.line_height()
        };
        let wrap_width = if bounds.size.width == gpui::Pixels::ZERO {
            gpui::Pixels::from(500.0)
        } else {
            bounds.size.width
        };

        let Ok(lines) = window.text_system().shape_text(
            rendered.text.clone(),
            font_size,
            &rendered.runs,
            Some(wrap_width),
            None,
        ) else {
            return 0;
        };

        let rendered_len = rendered.text.len();

        let mut y_offset = gpui::Pixels::ZERO;
        let mut char_offset = 0usize;

        for (line_ix, line) in lines.iter().enumerate() {
            let line_height_px = line_height;
            let line_size = line.size(line_height_px);

            if relative_pos.y < y_offset + line_size.height {
                let pos_in_line = Point {
                    x: relative_pos.x,
                    y: relative_pos.y - y_offset,
                };

                let index_in_line = line
                    .closest_index_for_position(pos_in_line, line_height_px)
                    .unwrap_or_else(|i| i);

                return (char_offset + index_in_line).min(rendered_len);
            }

            y_offset += line_size.height;
            char_offset += line.text.len();
            if line_ix + 1 < lines.len() {
                char_offset += '\n'.len_utf8();
            }
        }

        rendered_len
    }

    /// Returns the URL if the given offset is within a link region
    fn get_link_at_offset(&self, offset: usize, window: &Window) -> Option<String> {
        let rendered = markdown_text::render_markdown(
            self.content.as_ref(),
            &window.text_style(),
            &self.theme,
        );

        for link in &rendered.links {
            if link.range.contains(&offset) {
                return Some(link.url.clone());
            }
        }
        None
    }

    fn word_boundaries(content: &str, offset: usize) -> (usize, usize) {
        let offset = offset.min(content.len());

        let start = content[..offset]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);

        let end = content[offset..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| offset + i)
            .unwrap_or(content.len());

        (start, end)
    }
}

/// Apply reduced opacity to the trailing `fade_bytes` of rendered text runs.
/// Splits runs at the fade boundary so that only the trailing portion is affected.
/// Returns the original runs unchanged if the operation would produce invalid output.
fn apply_trailing_opacity(
    runs: &[TextRun],
    total_text_len: usize,
    fade_bytes: usize,
    opacity: f32,
) -> Vec<TextRun> {
    if fade_bytes == 0 || opacity >= 1.0 || runs.is_empty() || total_text_len == 0 {
        return runs.to_vec();
    }

    // Clamp fade_bytes to the actual text length to prevent mismatches
    // between source-byte counts and rendered-byte counts.
    let clamped_fade = fade_bytes.min(total_text_len);
    let fade_start = total_text_len - clamped_fade;

    let mut result = Vec::with_capacity(runs.len() + 1);
    let mut cursor = 0;

    for run in runs {
        if run.len == 0 {
            continue; // skip zero-length runs
        }
        let run_start = cursor;
        let run_end = cursor + run.len;
        cursor = run_end;

        if run_end <= fade_start {
            // Entirely before fade zone — full opacity
            result.push(run.clone());
        } else if run_start >= fade_start {
            // Entirely in fade zone — apply reduced opacity
            let mut faded = run.clone();
            faded.color.a *= opacity;
            result.push(faded);
        } else {
            // Split: part before fade, part in fade
            let split_point = fade_start - run_start;

            if split_point > 0 {
                let mut before = run.clone();
                before.len = split_point;
                result.push(before);
            }

            let after_len = run_end - fade_start;
            if after_len > 0 {
                let mut after = run.clone();
                after.len = after_len;
                after.color.a *= opacity;
                result.push(after);
            }
        }
    }

    // Safety: validate the total run length matches the text.
    // If it doesn't (due to source/rendered byte mismatch), return the
    // original runs unchanged rather than crashing GPUI.
    let result_len: usize = result.iter().map(|r| r.len).sum();
    if result_len != total_text_len {
        return runs.to_vec();
    }

    result
}

impl Focusable for SelectableText {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectableText {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Sync content from buffer if dirty (avoids SharedString alloc in append())
        if self.content_dirty {
            self.content = SharedString::from(self.content_buffer.clone());
            self.content_dirty = false;
        }

        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());

        let needs_rebuild = match &self.cached_rendered {
            Some(_) => self.cached_content != self.content || self.cached_font_size != font_size,
            None => true,
        };

        if needs_rebuild {
            // Always do a full rebuild — safe and correct.
            // (Incremental rendering disabled to isolate a GPUI text run panic.)
            self.cached_rendered = Some(markdown_text::render_markdown(
                self.content.as_ref(),
                &text_style,
                &self.theme,
            ));
            self.cached_content = self.content.clone();
            self.cached_font_size = font_size;
        }

        let rendered = self
            .cached_rendered
            .as_ref()
            .expect("rendered markdown cached");

        let selection_color: Hsla = gpui::hsla(0.6, 0.8, 0.5, 0.3);
        let runs = markdown_text::apply_selection_background(
            &rendered.runs,
            self.selected_range.clone(),
            selection_color,
        );

        let styled_text = StyledText::new(rendered.text.clone()).with_runs(runs);
        let view = cx.entity().clone();

        // Use an absolute-positioned full-size canvas to track the div bounds
        // This ensures we capture the exact bounds where text is rendered
        let bounds_tracker = gpui::canvas(
            move |bounds, _window, cx| {
                let should_update = view.read(cx).element_bounds != Some(bounds);
                if should_update {
                    view.update(cx, |this, _| {
                        this.element_bounds = Some(bounds);
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        // Use pointer cursor when hovering over links, otherwise text cursor
        let cursor = if self.hovering_link {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        };

        div()
            .id("selectable-text")
            .key_context("SelectableText")
            .w_full()
            .relative() // Needed for absolute positioning of bounds_tracker
            .track_focus(&self.focus_handle(cx))
            .cursor(cursor)
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::select_all))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .text_color(self.theme.text)
            .child(bounds_tracker)
            .child(styled_text)
    }
}
