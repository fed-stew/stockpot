//! Selectable text component for read-only text that can be selected and copied
//!
//! Provides a text display component with full mouse-based text selection
//! and system copy (Cmd+C) support.

use std::ops::Range;

use gpui::{
    div, prelude::*, App, ClipboardItem, Context, CursorStyle, FocusHandle, Focusable, Hsla,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point, SharedString,
    StyledText, Window,
};

use crate::gui::theme::Theme;

use super::markdown_text;

gpui::actions!(selectable_text, [Copy, SelectAll]);

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
}

impl SelectableText {
    pub fn new(cx: &mut Context<Self>, content: impl Into<SharedString>, theme: Theme) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: content.into(),
            selected_range: 0..0,
            is_selecting: false,
            theme,
            drag_start_offset: 0,
            drag_start_position: None,
            element_bounds: None,
            cached_rendered: None,
            cached_content: SharedString::from(""),
            cached_font_size: gpui::Pixels::ZERO,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = content.into();
        self.selected_range = 0..0;
        self.cached_rendered = None;
        self.cached_content = SharedString::from("");
        cx.notify();
    }

    pub fn append(&mut self, delta: &str, cx: &mut Context<Self>) {
        let mut new_content = self.content.to_string();
        new_content.push_str(delta);
        self.content = new_content.into();
        // We don't necessarily need to clear selection on append, but safe for now.
        // self.selected_range = 0..0; 
        self.cached_rendered = None;
        self.cached_content = SharedString::from("");
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
        self.is_selecting = true;
        self.drag_start_position = Some(event.position);
        self.focus_handle.focus(window, cx);

        let offset = self.hit_test_position(event.position, window);

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
            gpui::Pixels::from(1.0)
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

impl Focusable for SelectableText {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectableText {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let needs_rebuild = match &self.cached_rendered {
            Some(_) => self.cached_content != self.content || self.cached_font_size != font_size,
            None => true,
        };
        if needs_rebuild {
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
        .w_full()
        .h(gpui::px(0.));

        div()
            .id("selectable-text")
            .key_context("SelectableText")
            .w_full()
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
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
