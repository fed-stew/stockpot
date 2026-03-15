//! Terminal view component - renders alacritty_terminal grid in GPUI.
//!
//! Renders the terminal grid using div-based layout with proper:
//! - Font metrics from the actual resolved monospace font
//! - Cell background colors (ANSI, 256-color, truecolor)
//! - All text attributes: bold, italic, underline, strikethrough, dim, hidden, inverse
//! - Wide character handling
//! - Cursor rendering
//! - Scroll wheel support

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
    /// Convert an ANSI color to Hsla. `bold` brightens named foreground colors.
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
            NamedColor::Black => {
                if bold {
                    self.bright_black
                } else {
                    self.black
                }
            }
            NamedColor::Red => {
                if bold {
                    self.bright_red
                } else {
                    self.red
                }
            }
            NamedColor::Green => {
                if bold {
                    self.bright_green
                } else {
                    self.green
                }
            }
            NamedColor::Yellow => {
                if bold {
                    self.bright_yellow
                } else {
                    self.yellow
                }
            }
            NamedColor::Blue => {
                if bold {
                    self.bright_blue
                } else {
                    self.blue
                }
            }
            NamedColor::Magenta => {
                if bold {
                    self.bright_magenta
                } else {
                    self.magenta
                }
            }
            NamedColor::Cyan => {
                if bold {
                    self.bright_cyan
                } else {
                    self.cyan
                }
            }
            NamedColor::White => {
                if bold {
                    self.bright_white
                } else {
                    self.white
                }
            }
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

/// Resolve the first available monospace font.
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
}

/// Style for a span of cells with the same visual appearance.
#[derive(PartialEq)]
struct CellStyle {
    fg: Hsla,
    bg: Hsla,
    bg_is_default: bool,
    bold: bool,
    italic: bool,
    underline: bool,
    wavy_underline: bool,
    strikethrough: bool,
    is_cursor: bool,
}

impl TerminalView {
    pub fn new(
        terminal: Arc<FairMutex<Term<TerminalEventBridge>>>,
        input_tx: UnboundedSender<Vec<u8>>,
        resize_tx: UnboundedSender<portable_pty::PtySize>,
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
        }
    }

    /// Resize the terminal grid and PTY if the view size changed.
    fn resize_if_needed(&mut self, available_width: Pixels, available_height: Pixels, cx: &App) {
        let (_, cell_width, line_height) = Self::measure_cell(cx);
        let cw = f32::from(cell_width);
        let lh = f32::from(line_height);
        if cw <= 0.0 || lh <= 0.0 {
            return;
        }

        let new_cols = ((f32::from(available_width) - PADDING * 2.0) / cw).floor() as u16;
        let new_rows = ((f32::from(available_height) - PADDING * 2.0) / lh).floor() as u16;

        let new_cols = new_cols.max(10);
        let new_rows = new_rows.max(4);

        if new_cols != self.last_cols || new_rows != self.last_rows {
            self.last_cols = new_cols;
            self.last_rows = new_rows;

            // Resize alacritty terminal grid
            let term_size = spot_core::terminal::TerminalSize {
                cols: new_cols,
                rows: new_rows,
                cell_width: cw,
                cell_height: lh,
            };
            {
                let mut term = self.terminal.lock();
                term.resize(term_size);
            }

            // Resize PTY
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

    /// Compute the style for a cell, handling inverse, dim, hidden, etc.
    fn cell_style(
        &self,
        cell: &alacritty_terminal::term::cell::Cell,
        is_cursor: bool,
    ) -> CellStyle {
        let flags = cell.flags;
        let bold = flags.contains(Flags::BOLD);

        let (mut fg_raw, mut bg_raw) = (cell.fg, cell.bg);
        if flags.contains(Flags::INVERSE) {
            mem::swap(&mut fg_raw, &mut bg_raw);
        }

        // Bold brightens foreground only (not background)
        let mut fg = self.colors.convert(fg_raw, bold);
        let bg = self.colors.convert(bg_raw, false);

        let bg_is_default =
            self.colors.is_default_bg(bg_raw) && !flags.contains(Flags::INVERSE);

        if flags.contains(Flags::DIM) {
            fg.a *= 0.7;
        }
        if flags.contains(Flags::HIDDEN) {
            fg.a = 0.0;
        }

        // Cursor: override fg/bg
        if is_cursor {
            return CellStyle {
                fg: self.colors.background,
                bg: self.colors.foreground,
                bg_is_default: false,
                bold,
                italic: flags.contains(Flags::ITALIC),
                underline: flags.intersects(Flags::ALL_UNDERLINES),
                wavy_underline: flags.contains(Flags::UNDERCURL),
                strikethrough: flags.contains(Flags::STRIKEOUT),
                is_cursor: true,
            };
        }

        CellStyle {
            fg,
            bg,
            bg_is_default,
            bold,
            italic: flags.contains(Flags::ITALIC),
            underline: flags.intersects(Flags::ALL_UNDERLINES),
            wavy_underline: flags.contains(Flags::UNDERCURL),
            strikethrough: flags.contains(Flags::STRIKEOUT),
            is_cursor: false,
        }
    }

    /// Build a single row as a div with coalesced styled spans.
    fn render_row(
        &self,
        term: &Term<TerminalEventBridge>,
        row_idx: usize,
        cell_width: Pixels,
        line_height: Pixels,
        font_family: &SharedString,
    ) -> Div {
        let line = Line(row_idx as i32);
        let cols = term.columns();
        let cursor = term.grid().cursor.point;

        let mut spans: Vec<AnyElement> = Vec::new();
        let mut span_text = String::new();
        let mut span_style: Option<CellStyle> = None;
        let mut span_len: usize = 0;

        for col_idx in 0..cols {
            let point = AlacPoint::new(line, Column(col_idx));
            let cell = &term.grid()[point];

            // Skip wide char spacers
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            let is_cursor = cursor.line == line && cursor.column == Column(col_idx);
            let style = self.cell_style(cell, is_cursor);
            let c = if cell.c == '\0' { ' ' } else { cell.c };

            // Check if style changed — flush span
            let style_changed = span_style
                .as_ref()
                .map_or(false, |prev| *prev != style);

            if span_len > 0 && style_changed {
                let prev = span_style.take().unwrap();
                let text = std::mem::take(&mut span_text);
                spans.push(self.build_span(text, &prev, span_len, cell_width));
                span_len = 0;
            }

            if span_len == 0 {
                span_style = Some(style);
            }
            span_text.push(c);
            span_len += 1;
        }

        // Flush final span
        if span_len > 0 {
            if let Some(style) = span_style {
                spans.push(self.build_span(span_text, &style, span_len, cell_width));
            }
        }

        div()
            .h(line_height)
            .flex()
            .flex_row()
            .flex_shrink_0()
            .font_family(font_family.clone())
            .text_size(px(FONT_SIZE))
            .children(spans)
    }

    /// Build a styled span div from coalesced cells.
    fn build_span(
        &self,
        text: String,
        style: &CellStyle,
        cell_count: usize,
        cell_width: Pixels,
    ) -> AnyElement {
        let width = cell_width * cell_count as f32;
        let mut span = div().w(width).text_color(style.fg).flex_shrink_0();

        // Background (skip default to let parent bg show through)
        if !style.bg_is_default {
            span = span.bg(style.bg);
        }

        // Bold
        if style.bold {
            span = span.font_weight(FontWeight::BOLD);
        }

        // Italic
        if style.italic {
            span = span.italic();
        }

        // Underline
        if style.underline {
            span = span.text_decoration_1().text_decoration_color(style.fg);
        }

        // Strikethrough
        if style.strikethrough {
            span = span.line_through();
        }

        span.child(text).into_any_element()
    }

    /// Measure cell dimensions from the actual font.
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
        let ascent = text_system.ascent(font_id, font_size);
        let descent = text_system.descent(font_id, font_size);
        // Standard terminal line height: ~1.2x the font size, or ascent+descent+leading
        let line_height = (ascent + descent) * 1.3;
        (family, cell_width, line_height)
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Resize terminal to fill available space
        let window_size = window.viewport_size();
        // Use a reasonable portion of the window for the terminal
        self.resize_if_needed(window_size.width * 0.95, px(400.0), cx);

        let (font_family, cell_width, line_height) = Self::measure_cell(cx);

        let term = self.terminal.lock();
        let rows = term.screen_lines();
        let cols = term.columns();

        let mut row_elements: Vec<Div> = Vec::with_capacity(rows);
        for row_idx in 0..rows {
            row_elements.push(self.render_row(&term, row_idx, cell_width, line_height, &font_family));
        }
        drop(term);

        let content_height = rows as f32 * f32::from(line_height) + PADDING * 2.0;
        let focus_handle = self.focus_handle.clone();

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
            .w_full()
            .h(px(content_height))
            .overflow_hidden()
            .bg(self.colors.background)
            .p(px(PADDING))
            .rounded(px(4.))
            .border_1()
            .border_color(Hsla::from(rgba(0x40404080)))
            .cursor_text()
            .child(
                div()
                    .id("terminal-content")
                    .flex()
                    .flex_col()
                    .children(row_elements),
            )
    }
}
