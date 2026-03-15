//! Terminal view component - renders alacritty_terminal grid in GPUI.
//!
//! Uses canvas-based direct painting (paint_quad + shape_line) for
//! seamless terminal rendering:
//! - Background rects via paint_quad (no div seams between color spans)
//! - Text via shape_line with force_width for monospace grid alignment
//! - Dynamic resize to fill parent container

use gpui::*;
use parking_lot::FairMutex;
use std::mem;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point as AlacPoint};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::Color as AnsiColor;
use alacritty_terminal::vte::ansi::NamedColor;

use spot_core::terminal::TerminalEventBridge;

// ---------------------------------------------------------------------------
// Color palette
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TerminalColors {
    black: Hsla,
    red: Hsla,
    green: Hsla,
    yellow: Hsla,
    blue: Hsla,
    magenta: Hsla,
    cyan: Hsla,
    white: Hsla,
    bright_black: Hsla,
    bright_red: Hsla,
    bright_green: Hsla,
    bright_yellow: Hsla,
    bright_blue: Hsla,
    bright_magenta: Hsla,
    bright_cyan: Hsla,
    bright_white: Hsla,
    foreground: Hsla,
    background: Hsla,
}

fn h(hex: u32) -> Hsla {
    Hsla::from(rgba(hex))
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self {
            black: h(0x000000ff),
            red: h(0xcd3131ff),
            green: h(0x0dbc79ff),
            yellow: h(0xe5e510ff),
            blue: h(0x2472c8ff),
            magenta: h(0xbc3fbcff),
            cyan: h(0x11a8cdff),
            white: h(0xe5e5e5ff),
            bright_black: h(0x666666ff),
            bright_red: h(0xf14c4cff),
            bright_green: h(0x23d18bff),
            bright_yellow: h(0xf5f543ff),
            bright_blue: h(0x3b8eebff),
            bright_magenta: h(0xd670d6ff),
            bright_cyan: h(0x29b8dbff),
            bright_white: h(0xe5e5e5ff),
            foreground: h(0xccccccff),
            background: h(0x1e1e1eff),
        }
    }
}

impl TerminalColors {
    fn convert(&self, color: AnsiColor, bold: bool) -> Hsla {
        match color {
            AnsiColor::Named(named) => self.named(named, bold),
            AnsiColor::Spec(rgb) => Hsla::from(rgba(
                (rgb.r as u32) << 24 | (rgb.g as u32) << 16 | (rgb.b as u32) << 8 | 0xff,
            )),
            AnsiColor::Indexed(idx) => self.indexed(idx, bold),
        }
    }

    fn named(&self, named: NamedColor, bold: bool) -> Hsla {
        match named {
            NamedColor::Black if bold => self.bright_black,
            NamedColor::Black => self.black,
            NamedColor::Red if bold => self.bright_red,
            NamedColor::Red => self.red,
            NamedColor::Green if bold => self.bright_green,
            NamedColor::Green => self.green,
            NamedColor::Yellow if bold => self.bright_yellow,
            NamedColor::Yellow => self.yellow,
            NamedColor::Blue if bold => self.bright_blue,
            NamedColor::Blue => self.blue,
            NamedColor::Magenta if bold => self.bright_magenta,
            NamedColor::Magenta => self.magenta,
            NamedColor::Cyan if bold => self.bright_cyan,
            NamedColor::Cyan => self.cyan,
            NamedColor::White if bold => self.bright_white,
            NamedColor::White => self.white,
            NamedColor::BrightBlack => self.bright_black,
            NamedColor::BrightRed => self.bright_red,
            NamedColor::BrightGreen => self.bright_green,
            NamedColor::BrightYellow => self.bright_yellow,
            NamedColor::BrightBlue => self.bright_blue,
            NamedColor::BrightMagenta => self.bright_magenta,
            NamedColor::BrightCyan => self.bright_cyan,
            NamedColor::BrightWhite => self.bright_white,
            NamedColor::Foreground => self.foreground,
            NamedColor::Background => self.background,
            _ => self.foreground,
        }
    }

    fn indexed(&self, idx: u8, bold: bool) -> Hsla {
        match idx {
            0 => self.named(NamedColor::Black, bold),
            1 => self.named(NamedColor::Red, bold),
            2 => self.named(NamedColor::Green, bold),
            3 => self.named(NamedColor::Yellow, bold),
            4 => self.named(NamedColor::Blue, bold),
            5 => self.named(NamedColor::Magenta, bold),
            6 => self.named(NamedColor::Cyan, bold),
            7 => self.named(NamedColor::White, bold),
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) * 51;
                let g = ((idx / 6) % 6) * 51;
                let b = (idx % 6) * 51;
                Hsla::from(rgba(
                    (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | 0xff,
                ))
            }
            232..=255 => {
                let gray = (idx - 232) * 10 + 8;
                Hsla::from(rgba(
                    (gray as u32) << 24 | (gray as u32) << 16 | (gray as u32) << 8 | 0xff,
                ))
            }
        }
    }

    fn is_default_bg(&self, color: AnsiColor) -> bool {
        matches!(color, AnsiColor::Named(NamedColor::Background))
    }
}

// ---------------------------------------------------------------------------
// Font resolution
// ---------------------------------------------------------------------------

const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 8.0;

const FONT_FAMILIES: &[&str] = &[
    "Berkeley Mono",
    "Menlo",
    "Monaco",
    "Consolas",
    "SF Mono",
    "monospace",
];

fn resolve_terminal_font_family(text_system: &Arc<TextSystem>) -> SharedString {
    for &family in FONT_FAMILIES {
        let candidate = font(family);
        let font_id = text_system.resolve_font(&candidate);
        if text_system.advance(font_id, px(FONT_SIZE), 'm').is_ok() {
            return SharedString::from(family);
        }
    }
    SharedString::from("monospace")
}

// ---------------------------------------------------------------------------
// Paint data — extracted from the grid under lock, painted in canvas
// ---------------------------------------------------------------------------

/// A horizontal run of cells with the same background color.
struct BgRect {
    row: i32,
    col: i32,
    len: usize,
    color: Hsla,
}

/// A horizontal run of text with the same style.
struct TextBatch {
    row: i32,
    col: i32,
    text: String,
    cell_count: usize,
    run: TextRun,
}

// ---------------------------------------------------------------------------
// TerminalView
// ---------------------------------------------------------------------------

pub struct TerminalView {
    terminal: Arc<FairMutex<Term<TerminalEventBridge>>>,
    focus_handle: FocusHandle,
    colors: TerminalColors,
    input_tx: UnboundedSender<Vec<u8>>,
    resize_tx: UnboundedSender<portable_pty::PtySize>,
    scroll_offset: i32,
    last_cols: u16,
    last_rows: u16,
    available_width: Pixels,
    available_height: Pixels,
}

impl TerminalView {
    pub fn new(
        terminal: Arc<FairMutex<Term<TerminalEventBridge>>>,
        input_tx: UnboundedSender<Vec<u8>>,
        resize_tx: UnboundedSender<portable_pty::PtySize>,
        initial_height: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            terminal,
            focus_handle: cx.focus_handle(),
            colors: TerminalColors::default(),
            input_tx,
            resize_tx,
            scroll_offset: 0,
            last_cols: 0,
            last_rows: 0,
            available_width: px(800.0),
            available_height: px(initial_height),
        }
    }

    pub fn set_available_size(&mut self, width: Pixels, height: Pixels) {
        self.available_width = width;
        self.available_height = height;
    }

    fn resize_if_needed(&mut self, available_width: Pixels, available_height: Pixels, cx: &App) {
        let (_, cell_width, line_height) = Self::measure_cell(cx);
        let cw = f32::from(cell_width);
        let lh = f32::from(line_height);
        if cw <= 0.0 || lh <= 0.0 {
            return;
        }

        let chrome = PADDING * 2.0 + 2.0;
        let new_cols = ((f32::from(available_width) - chrome) / cw).floor() as u16;
        let new_rows = ((f32::from(available_height) - chrome) / lh).floor() as u16;
        let new_cols = new_cols.max(10);
        let new_rows = new_rows.max(4);

        if new_cols != self.last_cols || new_rows != self.last_rows {
            self.last_cols = new_cols;
            self.last_rows = new_rows;

            let term_size = spot_core::terminal::TerminalSize {
                cols: new_cols,
                rows: new_rows,
                cell_width: cw,
                cell_height: lh,
            };
            self.terminal.lock().resize(term_size);

            let _ = self.resize_tx.send(portable_pty::PtySize {
                rows: new_rows,
                cols: new_cols,
                pixel_width: (new_cols as f32 * cw) as u16,
                pixel_height: (new_rows as f32 * lh) as u16,
            });
        }
    }

    fn send_input(&self, text: &str) {
        let _ = self.input_tx.send(text.as_bytes().to_vec());
    }

    fn handle_key(&self, event: &KeyDownEvent) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;

        if mods.control && !mods.alt && !mods.shift {
            if let Some(c) = key.chars().next() {
                if c.is_ascii_alphabetic() {
                    let ctrl_char = (c.to_ascii_lowercase() as u8 - b'a' + 1) as char;
                    self.send_input(&ctrl_char.to_string());
                    return;
                }
            }
        }

        let sequence = match key {
            "enter" => "\r",
            "backspace" => "\x7f",
            "tab" => "\t",
            "escape" => "\x1b",
            "space" => " ",
            "up" => "\x1b[A",
            "down" => "\x1b[B",
            "right" => "\x1b[C",
            "left" => "\x1b[D",
            "home" => "\x1b[H",
            "end" => "\x1b[F",
            "pageup" => "\x1b[5~",
            "pagedown" => "\x1b[6~",
            "delete" => "\x1b[3~",
            "insert" => "\x1b[2~",
            "f1" => "\x1bOP",
            "f2" => "\x1bOQ",
            "f3" => "\x1bOR",
            "f4" => "\x1bOS",
            "f5" => "\x1b[15~",
            "f6" => "\x1b[17~",
            "f7" => "\x1b[18~",
            "f8" => "\x1b[19~",
            "f9" => "\x1b[20~",
            "f10" => "\x1b[21~",
            "f11" => "\x1b[23~",
            "f12" => "\x1b[24~",
            s if s.len() == 1 => s,
            _ => return,
        };

        self.send_input(sequence);
    }

    /// Extract background rects, text batches, and cursor from the grid.
    fn extract_paint_data(
        &self,
        term: &Term<TerminalEventBridge>,
        base_font: &Font,
    ) -> (Vec<BgRect>, Vec<TextBatch>, Option<(i32, i32)>) {
        let rows = term.screen_lines();
        let cols = term.columns();
        let cursor = term.grid().cursor.point;
        let cursor_pos = (cursor.line.0, cursor.column.0 as i32);

        let mut bg_rects: Vec<BgRect> = Vec::new();
        let mut text_batches: Vec<TextBatch> = Vec::new();

        for row_idx in 0..rows {
            let line = Line(row_idx as i32);

            // --- Background rects ---
            let mut bg_start: Option<(i32, Hsla, usize)> = None;
            for col_idx in 0..cols {
                let point = AlacPoint::new(line, Column(col_idx));
                let cell = &term.grid()[point];
                let flags = cell.flags;

                let (_, mut bg_color) = (cell.fg, cell.bg);
                if flags.contains(Flags::INVERSE) {
                    bg_color = cell.fg; // swapped
                }

                let bg_is_default =
                    self.colors.is_default_bg(bg_color) && !flags.contains(Flags::INVERSE);
                let bg_hsla = self.colors.convert(bg_color, false);

                if !bg_is_default {
                    if let Some((sc, pc, cnt)) = &mut bg_start {
                        if *pc == bg_hsla {
                            *cnt += 1;
                        } else {
                            bg_rects.push(BgRect {
                                row: row_idx as i32,
                                col: *sc,
                                len: *cnt,
                                color: *pc,
                            });
                            bg_start = Some((col_idx as i32, bg_hsla, 1));
                        }
                    } else {
                        bg_start = Some((col_idx as i32, bg_hsla, 1));
                    }
                } else if let Some((sc, pc, cnt)) = bg_start.take() {
                    bg_rects.push(BgRect {
                        row: row_idx as i32,
                        col: sc,
                        len: cnt,
                        color: pc,
                    });
                }
            }
            if let Some((sc, pc, cnt)) = bg_start.take() {
                bg_rects.push(BgRect {
                    row: row_idx as i32,
                    col: sc,
                    len: cnt,
                    color: pc,
                });
            }

            // --- Text batches ---
            let mut cur_batch: Option<TextBatch> = None;

            for col_idx in 0..cols {
                let point = AlacPoint::new(line, Column(col_idx));
                let cell = &term.grid()[point];
                let flags = cell.flags;

                if flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }

                let c = if cell.c == '\0' { ' ' } else { cell.c };
                let bold = flags.contains(Flags::BOLD);
                let is_cursor = cursor.line == line && cursor.column == Column(col_idx);

                let (mut fg_raw, mut bg_raw) = (cell.fg, cell.bg);
                if flags.contains(Flags::INVERSE) {
                    mem::swap(&mut fg_raw, &mut bg_raw);
                }

                let mut fg = if is_cursor {
                    self.colors.background
                } else {
                    self.colors.convert(fg_raw, bold)
                };

                if flags.contains(Flags::DIM) && !is_cursor {
                    fg.a *= 0.7;
                }
                if flags.contains(Flags::HIDDEN) && !is_cursor {
                    fg.a = 0.0;
                }

                let weight = if bold {
                    FontWeight::BOLD
                } else {
                    FontWeight::default()
                };
                let font_style = if flags.contains(Flags::ITALIC) {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                };

                let underline = flags
                    .intersects(Flags::ALL_UNDERLINES)
                    .then(|| UnderlineStyle {
                        color: Some(fg),
                        thickness: px(1.0),
                        wavy: flags.contains(Flags::UNDERCURL),
                    });
                let strikethrough =
                    flags
                        .contains(Flags::STRIKEOUT)
                        .then(|| StrikethroughStyle {
                            color: Some(fg),
                            thickness: px(1.0),
                        });

                let run = TextRun {
                    len: c.len_utf8(),
                    font: Font {
                        weight,
                        style: font_style,
                        ..base_font.clone()
                    },
                    color: fg,
                    background_color: None, // backgrounds painted separately
                    underline,
                    strikethrough,
                };

                // Try to merge with current batch
                if let Some(ref mut batch) = cur_batch {
                    let can_merge = batch.row == row_idx as i32
                        && batch.col + batch.cell_count as i32 == col_idx as i32
                        && batch.run.font == run.font
                        && batch.run.color == run.color
                        && batch.run.underline == run.underline
                        && batch.run.strikethrough == run.strikethrough;

                    if can_merge {
                        batch.text.push(c);
                        batch.cell_count += 1;
                        batch.run.len += c.len_utf8();
                        continue;
                    }
                    text_batches.push(cur_batch.take().unwrap());
                }

                cur_batch = Some(TextBatch {
                    row: row_idx as i32,
                    col: col_idx as i32,
                    text: String::from(c),
                    cell_count: 1,
                    run,
                });
            }
            if let Some(batch) = cur_batch.take() {
                text_batches.push(batch);
            }
        }

        (bg_rects, text_batches, Some(cursor_pos))
    }

    fn measure_cell(cx: &App) -> (SharedString, Pixels, Pixels) {
        let text_system = cx.text_system();
        let family = resolve_terminal_font_family(&text_system);
        let base_font = font(family.clone());
        let font_id = text_system.resolve_font(&base_font);
        let font_size = px(FONT_SIZE);
        let cell_width = text_system
            .advance(font_id, font_size, 'm')
            .map(|s| s.width)
            .unwrap_or(px(8.4));
        let line_height = font_size * 1.4;
        (family, cell_width, line_height)
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.resize_if_needed(self.available_width, self.available_height, cx);

        let (font_family, cell_width, line_height) = Self::measure_cell(cx);
        let font_size = px(FONT_SIZE);
        let base_font = font(font_family.clone());
        let colors = self.colors.clone();

        // Extract all paint data under lock
        let term = self.terminal.lock();
        let (bg_rects, text_batches, cursor_pos) =
            self.extract_paint_data(&term, &base_font);
        drop(term);

        let focus_handle = self.focus_handle.clone();
        let terminal_ref = cx.entity().downgrade();

        div()
            .id("terminal-view")
            .track_focus(&focus_handle)
            .key_context("Terminal")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                this.handle_key(event);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                let delta = match event.delta {
                    ScrollDelta::Lines(delta) => delta.y as i32,
                    ScrollDelta::Pixels(delta) => (f32::from(delta.y) / 17.0) as i32,
                };
                let term = this.terminal.lock();
                let history = term.history_size();
                drop(term);
                this.scroll_offset = (this.scroll_offset + delta).clamp(-(history as i32), 0);
                cx.notify();
            }))
            .size_full()
            .overflow_hidden()
            .bg(colors.background)
            .rounded(px(4.))
            .border_1()
            .border_color(Hsla::from(rgba(0x40404080)))
            .cursor_text()
            .child(
                canvas(
                    // Prepaint: shape all text lines (immutable borrow of text_system)
                    move |bounds, window, _cx| {
                        let origin =
                            bounds.origin + point(px(PADDING), px(PADDING));

                        let shaped: Vec<(Point<Pixels>, ShapedLine)> = text_batches
                            .iter()
                            .map(|batch| {
                                let pos = point(
                                    origin.x + batch.col as f32 * cell_width,
                                    origin.y + batch.row as f32 * line_height,
                                );
                                let shaped = window.text_system().shape_line(
                                    batch.text.clone().into(),
                                    font_size,
                                    std::slice::from_ref(&batch.run),
                                    None, // monospace font handles spacing naturally
                                );
                                (pos, shaped)
                            })
                            .collect();

                        (bounds, origin, shaped)
                    },
                    // Paint: backgrounds, cursor, then text
                    move |_bounds,
                          (content_bounds, origin, shaped_lines): (
                        Bounds<Pixels>,
                        Point<Pixels>,
                        Vec<(Point<Pixels>, ShapedLine)>,
                    ),
                          window,
                          cx| {
                        // 1. Background rects
                        for rect in &bg_rects {
                            let pos = point(
                                (origin.x + rect.col as f32 * cell_width).floor(),
                                origin.y + rect.row as f32 * line_height,
                            );
                            let sz = size(
                                (cell_width * rect.len as f32).ceil(),
                                line_height,
                            );
                            window
                                .paint_quad(fill(Bounds::new(pos, sz), rect.color));
                        }

                        // 2. Cursor
                        if let Some((cr, cc)) = cursor_pos {
                            let cx_pos = point(
                                (origin.x + cc as f32 * cell_width).floor(),
                                origin.y + cr as f32 * line_height,
                            );
                            window.paint_quad(fill(
                                Bounds::new(
                                    cx_pos,
                                    size(cell_width.ceil(), line_height),
                                ),
                                colors.foreground,
                            ));
                        }

                        // 3. Text
                        for (pos, shaped) in &shaped_lines {
                            let _ = shaped.paint(
                                *pos,
                                line_height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }

                        // 4. Observe bounds for dynamic resize
                        if let Some(view) = terminal_ref.upgrade() {
                            let w = content_bounds.size.width;
                            let h = content_bounds.size.height;
                            if w > px(0.0) && h > px(0.0) {
                                view.update(cx, |this, cx| {
                                    if (this.available_width - w).abs() > px(20.0)
                                        || (this.available_height - h).abs()
                                            > px(20.0)
                                    {
                                        this.available_width = w;
                                        this.available_height = h;
                                        cx.notify();
                                    }
                                });
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
}
