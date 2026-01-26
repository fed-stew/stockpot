//! Terminal view component - renders alacritty_terminal grid in GPUI.
//!
//! This provides a proper terminal rendering widget that:
//! - Renders the terminal grid with ANSI colors
//! - Handles keyboard input directly to PTY
//! - Shows cursor position
//! - Acts like a real terminal emulator

use gpui::*;
use parking_lot::FairMutex;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::Color as AnsiColor;
use alacritty_terminal::vte::ansi::NamedColor;

use stockpot_core::terminal::TerminalEventBridge;

/// ANSI color palette for terminal rendering
#[derive(Clone)]
struct TerminalColors {
    black: Rgba,
    red: Rgba,
    green: Rgba,
    yellow: Rgba,
    blue: Rgba,
    magenta: Rgba,
    cyan: Rgba,
    white: Rgba,
    bright_black: Rgba,
    bright_red: Rgba,
    bright_green: Rgba,
    bright_yellow: Rgba,
    bright_blue: Rgba,
    bright_magenta: Rgba,
    bright_cyan: Rgba,
    bright_white: Rgba,
    foreground: Rgba,
    background: Rgba,
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self {
            // Standard ANSI colors (VS Code Dark+ theme)
            black: rgba(0x000000ff),
            red: rgba(0xcd3131ff),
            green: rgba(0x0dbc79ff),
            yellow: rgba(0xe5e510ff),
            blue: rgba(0x2472c8ff),
            magenta: rgba(0xbc3fbcff),
            cyan: rgba(0x11a8cdff),
            white: rgba(0xe5e5e5ff),
            // Bright variants
            bright_black: rgba(0x666666ff),
            bright_red: rgba(0xf14c4cff),
            bright_green: rgba(0x23d18bff),
            bright_yellow: rgba(0xf5f543ff),
            bright_blue: rgba(0x3b8eebff),
            bright_magenta: rgba(0xd670d6ff),
            bright_cyan: rgba(0x29b8dbff),
            bright_white: rgba(0xe5e5e5ff),
            // Default colors
            foreground: rgba(0xccccccff),
            background: rgba(0x1e1e1eff),
        }
    }
}

impl TerminalColors {
    fn convert_color(&self, color: AnsiColor, is_bold: bool) -> Rgba {
        match color {
            AnsiColor::Named(named) => self.named_color(named, is_bold),
            AnsiColor::Spec(rgb) => {
                rgba((rgb.r as u32) << 24 | (rgb.g as u32) << 16 | (rgb.b as u32) << 8 | 0xff)
            }
            AnsiColor::Indexed(idx) => self.indexed_color(idx, is_bold),
        }
    }

    fn named_color(&self, named: NamedColor, is_bold: bool) -> Rgba {
        match named {
            NamedColor::Black => {
                if is_bold {
                    self.bright_black
                } else {
                    self.black
                }
            }
            NamedColor::Red => {
                if is_bold {
                    self.bright_red
                } else {
                    self.red
                }
            }
            NamedColor::Green => {
                if is_bold {
                    self.bright_green
                } else {
                    self.green
                }
            }
            NamedColor::Yellow => {
                if is_bold {
                    self.bright_yellow
                } else {
                    self.yellow
                }
            }
            NamedColor::Blue => {
                if is_bold {
                    self.bright_blue
                } else {
                    self.blue
                }
            }
            NamedColor::Magenta => {
                if is_bold {
                    self.bright_magenta
                } else {
                    self.magenta
                }
            }
            NamedColor::Cyan => {
                if is_bold {
                    self.bright_cyan
                } else {
                    self.cyan
                }
            }
            NamedColor::White => {
                if is_bold {
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

    fn indexed_color(&self, idx: u8, is_bold: bool) -> Rgba {
        match idx {
            0 => self.named_color(NamedColor::Black, is_bold),
            1 => self.named_color(NamedColor::Red, is_bold),
            2 => self.named_color(NamedColor::Green, is_bold),
            3 => self.named_color(NamedColor::Yellow, is_bold),
            4 => self.named_color(NamedColor::Blue, is_bold),
            5 => self.named_color(NamedColor::Magenta, is_bold),
            6 => self.named_color(NamedColor::Cyan, is_bold),
            7 => self.named_color(NamedColor::White, is_bold),
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            // 216 color cube (16-231)
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) * 51;
                let g = ((idx / 6) % 6) * 51;
                let b = (idx % 6) * 51;
                rgba((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | 0xff)
            }
            // Grayscale (232-255)
            232..=255 => {
                let gray = (idx - 232) * 10 + 8;
                rgba((gray as u32) << 24 | (gray as u32) << 16 | (gray as u32) << 8 | 0xff)
            }
        }
    }
}

/// Terminal view that renders an alacritty terminal grid
pub struct TerminalView {
    /// The alacritty terminal instance
    terminal: Arc<FairMutex<Term<TerminalEventBridge>>>,
    /// Focus handle for keyboard events
    focus_handle: FocusHandle,
    /// Color palette
    colors: TerminalColors,
    /// Channel to send input to PTY
    input_tx: UnboundedSender<Vec<u8>>,
    /// Cell dimensions (calculated from font)
    /// Cell width for calculating terminal dimensions (reserved for resize support)
    #[allow(dead_code)]
    cell_width: f32,
    cell_height: f32,
}

impl TerminalView {
    pub fn new(
        terminal: Arc<FairMutex<Term<TerminalEventBridge>>>,
        input_tx: UnboundedSender<Vec<u8>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            terminal,
            focus_handle: cx.focus_handle(),
            colors: TerminalColors::default(),
            input_tx,
            cell_width: 8.4,
            cell_height: 17.0,
        }
    }

    /// Send raw bytes to the PTY
    fn send_bytes(&self, bytes: &[u8]) {
        let _ = self.input_tx.send(bytes.to_vec());
    }

    /// Send a string to the PTY
    fn send_input(&self, text: &str) {
        self.send_bytes(text.as_bytes());
    }

    /// Handle a key event and convert to terminal escape sequence
    fn handle_key(&self, event: &KeyDownEvent) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;

        // Handle Ctrl+key combinations
        if mods.control && !mods.alt && !mods.shift {
            if let Some(c) = key.chars().next() {
                if c.is_ascii_alphabetic() {
                    // Ctrl+A = 0x01, Ctrl+B = 0x02, etc.
                    let ctrl_char = (c.to_ascii_lowercase() as u8 - b'a' + 1) as char;
                    self.send_input(&ctrl_char.to_string());
                    return;
                }
            }
        }

        // Handle special keys
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
            // Regular character
            s if s.len() == 1 => s,
            _ => return,
        };

        self.send_input(sequence);
    }

    /// Build a single row element from the terminal grid
    fn render_row(&self, term: &Term<TerminalEventBridge>, row_idx: usize) -> Div {
        let line = Line(row_idx as i32);
        let cols = term.columns();
        let cursor = term.grid().cursor.point;
        // Cursor is visible by default (HIDE_CURSOR mode check removed for simplicity)
        let cursor_visible = true;

        // Build spans with different colors
        let mut spans: Vec<AnyElement> = Vec::new();
        let mut current_text = String::new();
        let mut current_fg = self.colors.foreground;
        let mut current_flags = Flags::empty();

        for col_idx in 0..cols {
            let point = Point::new(line, Column(col_idx));
            let cell = &term.grid()[point];
            let c = cell.c;

            // Get cell colors
            let is_bold = cell.flags.contains(Flags::BOLD);
            let fg = self.colors.convert_color(cell.fg, is_bold);

            // Check if we need to start a new span (color changed)
            if fg != current_fg || cell.flags != current_flags {
                // Flush current span
                if !current_text.is_empty() {
                    let text = std::mem::take(&mut current_text);
                    let color = current_fg;
                    let bold = current_flags.contains(Flags::BOLD);
                    spans.push(if bold {
                        div()
                            .text_color(color)
                            .font_weight(FontWeight::BOLD)
                            .child(text)
                            .into_any_element()
                    } else {
                        div().text_color(color).child(text).into_any_element()
                    });
                }
                current_fg = fg;
                current_flags = cell.flags;
            }

            // Check if this is the cursor position
            if cursor_visible && cursor.line == line && cursor.column == Column(col_idx) {
                // Flush current span before cursor
                if !current_text.is_empty() {
                    let text = std::mem::take(&mut current_text);
                    let color = current_fg;
                    let bold = current_flags.contains(Flags::BOLD);
                    spans.push(if bold {
                        div()
                            .text_color(color)
                            .font_weight(FontWeight::BOLD)
                            .child(text)
                            .into_any_element()
                    } else {
                        div().text_color(color).child(text).into_any_element()
                    });
                }

                // Render cursor (inverted colors)
                let cursor_char = if c == ' ' || c == '\0' { ' ' } else { c };
                spans.push(
                    div()
                        .bg(self.colors.foreground)
                        .text_color(self.colors.background)
                        .child(cursor_char.to_string())
                        .into_any_element(),
                );
            } else {
                // Regular character
                current_text.push(if c == '\0' { ' ' } else { c });
            }
        }

        // Flush remaining text
        if !current_text.is_empty() {
            let text = current_text;
            let color = current_fg;
            let bold = current_flags.contains(Flags::BOLD);
            spans.push(if bold {
                div()
                    .text_color(color)
                    .font_weight(FontWeight::BOLD)
                    .child(text)
                    .into_any_element()
            } else {
                div().text_color(color).child(text).into_any_element()
            });
        }

        // If row is empty, add a space to maintain height
        if spans.is_empty() {
            spans.push(div().child(" ").into_any_element());
        }

        div()
            .h(px(self.cell_height))
            .flex()
            .flex_row()
            .whitespace_nowrap()
            .font_family("Berkeley Mono, Menlo, Monaco, Consolas, monospace")
            .text_size(px(13.))
            .children(spans)
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let term = self.terminal.lock();
        let rows = term.screen_lines();

        // Build all row elements
        let mut row_elements: Vec<Div> = Vec::with_capacity(rows);
        for row_idx in 0..rows {
            row_elements.push(self.render_row(&term, row_idx));
        }

        let bg = self.colors.background;
        drop(term); // Release lock before building element tree

        let focus_handle = self.focus_handle.clone();

        // Calculate height based on rows (width will be flexible)
        let padding = 16.0;
        let content_height = rows as f32 * self.cell_height + padding;

        div()
            .id("terminal-view")
            .track_focus(&focus_handle)
            .key_context("Terminal")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                this.handle_key(event);
            }))
            .on_click(cx.listener(|this, _, window, cx| {
                this.focus_handle.focus(window, cx);
            }))
            // Responsive width, fixed height based on rows
            .w_full()
            .h(px(content_height))
            .overflow_hidden()
            .bg(bg)
            .p(px(8.))
            .rounded(px(4.))
            .border_1()
            .border_color(rgba(0x40404080))
            .cursor_text()
            .child(
                div()
                    .id("terminal-content")
                    .flex()
                    .flex_col()
                    .overflow_x_scroll() // Scroll horizontally if content is wider
                    .children(row_elements),
            )
    }
}
