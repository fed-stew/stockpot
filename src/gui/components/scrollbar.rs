//! Visual scrollbar for a `ScrollHandle`.
//!
//! This is intentionally simple: it draws a track + thumb and supports dragging.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, div, point, prelude::*, px, rgba, App, Bounds, Hsla, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollHandle, Size,
    Window,
};

use crate::gui::theme::Theme;

const SCROLLBAR_WIDTH: Pixels = px(8.);
const THUMB_MIN_HEIGHT: Pixels = px(32.);

#[derive(Default)]
pub struct ScrollbarDragState {
    is_dragging: Cell<bool>,
    drag_start_y: Cell<Pixels>,
    drag_start_offset_y: Cell<Pixels>,
    bounds: Cell<Bounds<Pixels>>,
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn thumb_height(handle: &ScrollHandle, track_height: Pixels) -> Option<Pixels> {
    let viewport_height = handle.bounds().size.height;
    let max = handle.max_offset().height;

    if viewport_height <= Pixels::ZERO || max <= Pixels::ZERO || track_height <= Pixels::ZERO {
        return None;
    }

    let content_height = viewport_height + max;
    if content_height <= viewport_height {
        return None;
    }

    Some(
        (track_height * (viewport_height / content_height))
            .max(THUMB_MIN_HEIGHT)
            .min(track_height),
    )
}

pub fn scroll_ratio(handle: &ScrollHandle) -> f32 {
    let max = handle.max_offset().height;
    if max <= Pixels::ZERO {
        return 0.0;
    }

    // GPUI scroll offsets are negative as you scroll down.
    clamp01(-handle.offset().y / max)
}

fn set_scroll_ratio(handle: &ScrollHandle, ratio: f32) {
    let max = handle.max_offset().height;
    if max <= Pixels::ZERO {
        return;
    }

    handle.set_offset(Point::new(px(0.), -(max * clamp01(ratio))));
}

/// Create a visual scrollbar that tracks a `ScrollHandle`.
///
/// `drag_state` must be stable across frames (store it in your view state and clone the `Rc`).
pub fn scrollbar(
    handle: ScrollHandle,
    drag_state: Rc<ScrollbarDragState>,
    _theme: Theme,
) -> impl IntoElement {
    let track_color: Hsla = rgba(0x00000044).into();
    let thumb_color: Hsla = rgba(0xffffff77).into();
    let thumb_drag_color: Hsla = rgba(0xffffffaa).into();

    let handle_for_canvas_prepaint = handle.clone();
    let handle_for_canvas_paint = handle.clone();
    let handle_for_down = handle.clone();
    let handle_for_move = handle.clone();

    div()
        .w(SCROLLBAR_WIDTH)
        .h_full()
        .flex_shrink_0()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, {
            let drag_state = drag_state.clone();
            move |event: &MouseDownEvent, window: &mut Window, _cx: &mut App| {
                let bounds = drag_state.bounds.get();
                let max_offset = handle_for_down.max_offset();

                if max_offset.height <= Pixels::ZERO || bounds.size.height <= Pixels::ZERO {
                    return;
                }

                let track_height = bounds.size.height;
                let Some(thumb_height) = thumb_height(&handle_for_down, track_height) else {
                    return;
                };

                let current_offset = handle_for_down.offset();
                let ratio = scroll_ratio(&handle_for_down);

                let thumb_range = (track_height - thumb_height).max(Pixels::ZERO);
                let thumb_y = thumb_range * ratio;

                let click_y = (event.position.y - bounds.origin.y)
                    .max(Pixels::ZERO)
                    .min(track_height);

                let in_thumb = click_y >= thumb_y && click_y <= (thumb_y + thumb_height);

                if in_thumb {
                    drag_state.is_dragging.set(true);
                    drag_state.drag_start_y.set(event.position.y);
                    drag_state.drag_start_offset_y.set(current_offset.y);
                } else if thumb_range > Pixels::ZERO {
                    // Click on the track: jump so the thumb centers on the click.
                    let target_thumb_y =
                        (click_y - (thumb_height / 2.0)).clamp(Pixels::ZERO, thumb_range);
                    set_scroll_ratio(&handle_for_down, target_thumb_y / thumb_range);
                }

                window.refresh();
            }
        })
        .on_mouse_move({
            let drag_state = drag_state.clone();
            move |event: &MouseMoveEvent, window: &mut Window, _cx: &mut App| {
                if !drag_state.is_dragging.get() {
                    return;
                }

                let bounds = drag_state.bounds.get();
                let max_offset = handle_for_move.max_offset();

                if max_offset.height <= Pixels::ZERO || bounds.size.height <= Pixels::ZERO {
                    return;
                }

                let track_height = bounds.size.height;
                let Some(thumb_height) = thumb_height(&handle_for_move, track_height) else {
                    return;
                };

                let thumb_range = (track_height - thumb_height).max(Pixels::ZERO);
                if thumb_range <= Pixels::ZERO {
                    return;
                }

                let mouse_delta_y = event.position.y - drag_state.drag_start_y.get();

                // Convert track movement to scroll movement.
                let scroll_delta = (mouse_delta_y / thumb_range) * max_offset.height;

                let start_offset_y = drag_state.drag_start_offset_y.get();
                let new_offset_y =
                    (start_offset_y - scroll_delta).clamp(-max_offset.height, Pixels::ZERO);

                let current_offset = handle_for_move.offset();
                handle_for_move.set_offset(point(current_offset.x, new_offset_y));

                window.refresh();
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let drag_state = drag_state.clone();
            move |_event: &MouseUpEvent, window: &mut Window, _cx: &mut App| {
                drag_state.is_dragging.set(false);
                window.refresh();
            }
        })
        .on_mouse_up_out(MouseButton::Left, {
            let drag_state = drag_state.clone();
            move |_event: &MouseUpEvent, window: &mut Window, _cx: &mut App| {
                drag_state.is_dragging.set(false);
                window.refresh();
            }
        })
        .child(
            canvas(
                {
                    let drag_state = drag_state.clone();
                    move |bounds, _window, _cx| {
                        drag_state.bounds.set(bounds);
                        let viewport_bounds = handle_for_canvas_prepaint.bounds();
                        let max_offset = handle_for_canvas_prepaint.max_offset();
                        let offset = handle_for_canvas_prepaint.offset();
                        let dragging = drag_state.is_dragging.get();
                        (viewport_bounds, max_offset, offset, dragging)
                    }
                },
                move |bounds, (viewport_bounds, max_offset, offset, dragging), window, _cx| {
                    let viewport_height = viewport_bounds.size.height;
                    let max = max_offset.height;

                    if viewport_height <= Pixels::ZERO || max <= Pixels::ZERO {
                        return;
                    }

                    let content_height = viewport_height + max;
                    if content_height <= viewport_height {
                        return;
                    }

                    let track_height = bounds.size.height;
                    let Some(thumb_height) = thumb_height(&handle_for_canvas_paint, track_height)
                    else {
                        return;
                    };

                    // Track
                    window.paint_quad(PaintQuad {
                        bounds,
                        corner_radii: px(4.).into(),
                        background: track_color.into(),
                        border_widths: Default::default(),
                        border_color: gpui::transparent_black(),
                        border_style: gpui::BorderStyle::Solid,
                    });

                    let ratio = if max > Pixels::ZERO {
                        clamp01(-offset.y / max)
                    } else {
                        0.0
                    };

                    let thumb_y = (track_height - thumb_height) * ratio;

                    let thumb_bounds = Bounds {
                        origin: point(bounds.origin.x + px(1.), bounds.origin.y + thumb_y),
                        size: Size {
                            width: bounds.size.width - px(2.),
                            height: thumb_height,
                        },
                    };

                    let color = if dragging {
                        thumb_drag_color
                    } else {
                        thumb_color
                    };
                    window.paint_quad(PaintQuad {
                        bounds: thumb_bounds,
                        corner_radii: px(3.).into(),
                        background: color.into(),
                        border_widths: Default::default(),
                        border_color: gpui::transparent_black(),
                        border_style: gpui::BorderStyle::Solid,
                    });
                },
            )
            .size_full(),
        )
}

/// Drag state for list scrollbar
#[derive(Default)]
pub struct ListScrollbarDragState {
    pub is_dragging: Cell<bool>,
    pub drag_start_y: Cell<Pixels>,
    pub drag_start_ratio: Cell<f32>,
    pub bounds: Cell<Bounds<Pixels>>,
}

/// Create a visual scrollbar for a GPUI `ListState`.
///
/// Uses GPUI's native pixel-based scrollbar APIs on `ListState`.
pub fn list_scrollbar(
    list_state: ListState,
    _item_count: usize,
    drag_state: Rc<ListScrollbarDragState>,
    _theme: Theme,
) -> impl IntoElement {
    let track_color: Hsla = rgba(0x00000044).into();
    let thumb_color: Hsla = rgba(0xffffff77).into();
    let thumb_drag_color: Hsla = rgba(0xffffffaa).into();

    fn list_thumb_height(
        viewport_height: Pixels,
        max_offset_height: Pixels,
        track_height: Pixels,
    ) -> Option<Pixels> {
        if viewport_height <= Pixels::ZERO
            || max_offset_height <= Pixels::ZERO
            || track_height <= Pixels::ZERO
        {
            return None;
        }

        let content_height = viewport_height + max_offset_height;
        if content_height <= viewport_height {
            return None;
        }

        Some(
            (track_height * (viewport_height / content_height))
                .max(THUMB_MIN_HEIGHT)
                .min(track_height),
        )
    }

    fn set_scroll_ratio(list_state: &ListState, max_offset_height: Pixels, ratio: f32) {
        if max_offset_height <= Pixels::ZERO {
            return;
        }

        list_state.set_offset_from_scrollbar(point(px(0.), -(max_offset_height * clamp01(ratio))));
    }

    let list_state_for_down = list_state.clone();
    let list_state_for_move = list_state.clone();
    let list_state_for_up = list_state.clone();
    let list_state_for_up_out = list_state.clone();

    div()
        .w(SCROLLBAR_WIDTH)
        .h_full()
        .flex_shrink_0()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, {
            let drag_state = drag_state.clone();
            move |event: &MouseDownEvent, window: &mut Window, _cx: &mut App| {
                let bounds = drag_state.bounds.get();
                let track_height = bounds.size.height;
                if track_height <= Pixels::ZERO {
                    return;
                }

                let viewport_height = list_state_for_down.viewport_bounds().size.height;
                let max_offset = list_state_for_down.max_offset_for_scrollbar();

                if viewport_height <= Pixels::ZERO || max_offset.height <= Pixels::ZERO {
                    return;
                }

                let Some(thumb_height) =
                    list_thumb_height(viewport_height, max_offset.height, track_height)
                else {
                    return;
                };

                let thumb_range = (track_height - thumb_height).max(Pixels::ZERO);
                let offset = list_state_for_down.scroll_px_offset_for_scrollbar();
                let ratio = clamp01(-offset.y / max_offset.height);

                let thumb_y = thumb_range * ratio;

                let click_y = (event.position.y - bounds.origin.y)
                    .max(Pixels::ZERO)
                    .min(track_height);

                let in_thumb = click_y >= thumb_y && click_y <= (thumb_y + thumb_height);

                if in_thumb {
                    drag_state.is_dragging.set(true);
                    drag_state.drag_start_y.set(event.position.y);
                    drag_state.drag_start_ratio.set(ratio);
                    list_state_for_down.scrollbar_drag_started();
                } else if thumb_range > Pixels::ZERO {
                    // Click on the track: jump so the thumb centers on the click.
                    let target_thumb_y =
                        (click_y - (thumb_height / 2.0)).clamp(Pixels::ZERO, thumb_range);
                    set_scroll_ratio(
                        &list_state_for_down,
                        max_offset.height,
                        target_thumb_y / thumb_range,
                    );
                }

                window.refresh();
            }
        })
        .on_mouse_move({
            let drag_state = drag_state.clone();
            move |event: &MouseMoveEvent, window: &mut Window, _cx: &mut App| {
                if !drag_state.is_dragging.get() {
                    return;
                }

                let bounds = drag_state.bounds.get();
                let track_height = bounds.size.height;
                if track_height <= Pixels::ZERO {
                    return;
                }

                let viewport_height = list_state_for_move.viewport_bounds().size.height;
                let max_offset = list_state_for_move.max_offset_for_scrollbar();

                if viewport_height <= Pixels::ZERO || max_offset.height <= Pixels::ZERO {
                    return;
                }

                let Some(thumb_height) =
                    list_thumb_height(viewport_height, max_offset.height, track_height)
                else {
                    return;
                };

                let thumb_range = (track_height - thumb_height).max(Pixels::ZERO);
                if thumb_range <= Pixels::ZERO {
                    return;
                }

                let mouse_delta_y = event.position.y - drag_state.drag_start_y.get();
                let start_ratio = drag_state.drag_start_ratio.get();
                let new_ratio = clamp01(start_ratio + (mouse_delta_y / thumb_range));

                set_scroll_ratio(&list_state_for_move, max_offset.height, new_ratio);
                window.refresh();
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let drag_state = drag_state.clone();
            move |_event: &MouseUpEvent, window: &mut Window, _cx: &mut App| {
                if drag_state.is_dragging.replace(false) {
                    list_state_for_up.scrollbar_drag_ended();
                }
                window.refresh();
            }
        })
        .on_mouse_up_out(MouseButton::Left, {
            let drag_state = drag_state.clone();
            move |_event: &MouseUpEvent, window: &mut Window, _cx: &mut App| {
                if drag_state.is_dragging.replace(false) {
                    list_state_for_up_out.scrollbar_drag_ended();
                }
                window.refresh();
            }
        })
        .child(
            canvas(
                {
                    let drag_state = drag_state.clone();
                    let list_state = list_state.clone();
                    move |bounds, _window, _cx| {
                        drag_state.bounds.set(bounds);
                        let dragging = drag_state.is_dragging.get();
                        let viewport_bounds = list_state.viewport_bounds();
                        let max_offset = list_state.max_offset_for_scrollbar();
                        let offset = list_state.scroll_px_offset_for_scrollbar();
                        (dragging, viewport_bounds, max_offset, offset)
                    }
                },
                move |bounds, (dragging, viewport_bounds, max_offset, offset), window, _cx| {
                    let viewport_height = viewport_bounds.size.height;
                    let max = max_offset.height;

                    if viewport_height <= Pixels::ZERO || max <= Pixels::ZERO {
                        return;
                    }

                    let track_height = bounds.size.height;
                    let Some(thumb_height) = list_thumb_height(viewport_height, max, track_height)
                    else {
                        return;
                    };

                    // Track background
                    window.paint_quad(PaintQuad {
                        bounds,
                        corner_radii: px(4.).into(),
                        background: track_color.into(),
                        border_widths: Default::default(),
                        border_color: gpui::transparent_black(),
                        border_style: gpui::BorderStyle::Solid,
                    });

                    let ratio = clamp01(-offset.y / max);
                    let thumb_y = (track_height - thumb_height) * ratio;

                    let thumb_bounds = Bounds {
                        origin: point(bounds.origin.x + px(1.), bounds.origin.y + thumb_y),
                        size: Size {
                            width: bounds.size.width - px(2.),
                            height: thumb_height,
                        },
                    };

                    let color = if dragging {
                        thumb_drag_color
                    } else {
                        thumb_color
                    };
                    window.paint_quad(PaintQuad {
                        bounds: thumb_bounds,
                        corner_radii: px(3.).into(),
                        background: color.into(),
                        border_widths: Default::default(),
                        border_color: gpui::transparent_black(),
                        border_style: gpui::BorderStyle::Solid,
                    });
                },
            )
            .size_full(),
        )
}
