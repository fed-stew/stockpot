//! Terminal instance with alacritty_terminal backend.

use std::sync::Arc;

use alacritty_terminal::event::Event as AlacTermEvent;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte::ansi::Processor;
use parking_lot::FairMutex;
use portable_pty::PtySize;
use tokio::sync::mpsc;
use tracing::warn;

use super::pty::{headless_env, interactive_env, spawn_pty, PtyConfig, PtyEvent, SpawnedPty};
use super::types::{
    CellContent, CursorState, IndexedCell, ProcessKind, ProcessSnapshot, TerminalContent,
    TerminalEventBridge, TerminalSize,
};

/// Maximum output buffer size (50KB, ~15K tokens)
const MAX_OUTPUT_CHARS: usize = 50_000;

/// Terminal instance that manages PTY, emulation, and state
pub struct Terminal {
    /// Process ID for this terminal
    process_id: String,
    /// Process kind
    kind: ProcessKind,
    /// Alacritty terminal state
    term: Arc<FairMutex<Term<TerminalEventBridge>>>,
    /// VTE processor for parsing escape sequences
    processor: Processor,
    /// Channel to send input to PTY
    writer_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Channel to resize PTY
    resize_tx: mpsc::UnboundedSender<PtySize>,
    /// Cached terminal content for rendering
    last_content: TerminalContent,
    /// Raw output buffer (for tool response)
    output_buffer: String,
    /// Exit code (if process has finished)
    exit_code: Option<i32>,
    /// Whether terminal is visible in UI
    visible: bool,
    /// Started timestamp
    started_at_ms: u64,
}

impl Terminal {
    /// Create a new terminal executing a command
    pub async fn spawn_command(
        process_id: String,
        command: String,
        cwd: Option<std::path::PathBuf>,
        kind: ProcessKind,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PtyEvent>), String> {
        let size = PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Choose environment based on terminal type
        let env = match kind {
            ProcessKind::User => interactive_env(),
            ProcessKind::Llm => headless_env(),
        };

        let config = PtyConfig {
            command,
            cwd,
            size,
            env,
        };

        let spawned = spawn_pty(config)?;
        Self::from_spawned_pty(process_id, spawned, kind, size)
    }

    /// Create a terminal from a spawned PTY
    fn from_spawned_pty(
        process_id: String,
        spawned: SpawnedPty,
        kind: ProcessKind,
        pty_size: PtySize,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PtyEvent>), String> {
        // Create event bridge channel
        let (event_tx, _event_rx) = mpsc::unbounded_channel::<AlacTermEvent>();
        let event_bridge = TerminalEventBridge(event_tx);

        // Create terminal size
        let term_size = TerminalSize {
            cols: pty_size.cols,
            rows: pty_size.rows,
            cell_width: 8.0, // Will be updated on layout
            cell_height: 16.0,
        };

        // Create alacritty terminal
        let term_config = TermConfig::default();
        let term = Term::new(term_config, &term_size, event_bridge);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let terminal = Self {
            process_id,
            kind,
            term: Arc::new(FairMutex::new(term)),
            processor: Processor::default(),
            writer_tx: spawned.writer_tx,
            resize_tx: spawned.resize_tx,
            last_content: TerminalContent::default(),
            output_buffer: String::new(),
            exit_code: None,
            visible: true,
            started_at_ms: now_ms,
        };

        Ok((terminal, spawned.output_rx))
    }

    /// Process bytes from PTY output
    pub fn process_output(&mut self, bytes: &[u8]) {
        // Append to raw output buffer (truncate if needed)
        let text = String::from_utf8_lossy(bytes);
        self.output_buffer.push_str(&text);
        if self.output_buffer.len() > MAX_OUTPUT_CHARS {
            // Keep the last MAX_OUTPUT_CHARS
            let start = self.output_buffer.len() - MAX_OUTPUT_CHARS;
            self.output_buffer = self.output_buffer[start..].to_string();
        }

        // Process through VTE for terminal emulation
        let mut term = self.term.lock();
        for byte in bytes {
            self.processor.advance(&mut *term, *byte);
        }
    }

    /// Mark terminal as exited
    pub fn set_exit_code(&mut self, code: Option<i32>) {
        self.exit_code = code;
    }

    /// Write bytes to the PTY
    pub fn write(&self, data: Vec<u8>) {
        if self.writer_tx.send(data).is_err() {
            warn!("Failed to send to PTY writer");
        }
    }

    /// Write string to the PTY
    pub fn write_str(&self, s: &str) {
        self.write(s.as_bytes().to_vec());
    }

    /// Resize the terminal
    pub fn resize(&mut self, size: TerminalSize) {
        let pty_size = PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: (size.cols as f32 * size.cell_width) as u16,
            pixel_height: (size.rows as f32 * size.cell_height) as u16,
        };

        if self.resize_tx.send(pty_size).is_err() {
            warn!("Failed to send resize to PTY");
        }

        // Resize alacritty term
        let mut term = self.term.lock();
        term.resize(size);
    }

    /// Get terminal content for rendering
    pub fn content(&mut self) -> TerminalContent {
        let term = self.term.lock();

        let mut cells = Vec::new();
        for indexed in term.grid().display_iter() {
            let cell = indexed.cell;
            cells.push(IndexedCell {
                point: alacritty_terminal::index::Point {
                    line: indexed.point.line.0 as usize,
                    column: indexed.point.column,
                },
                cell: CellContent {
                    character: cell.c,
                    fg: cell.fg,
                    bg: cell.bg,
                    flags: cell.flags,
                },
            });
        }

        let cursor = term.grid().cursor.point;

        let content = TerminalContent {
            cells,
            mode: *term.mode(),
            cursor: CursorState {
                point: alacritty_terminal::index::Point {
                    line: cursor.line.0 as usize,
                    column: cursor.column,
                },
                // Cursor is visible by default unless explicitly hidden
                visible: true,
            },
            selection: None, // TODO: Implement selection
            display_offset: term.grid().display_offset(),
            size: TerminalSize {
                cols: term.columns() as u16,
                rows: term.screen_lines() as u16,
                cell_width: 8.0,
                cell_height: 16.0,
            },
        };

        self.last_content = content.clone();
        content
    }

    /// Get raw output text (for tool responses)
    pub fn output_text(&self) -> &str {
        &self.output_buffer
    }

    /// Create a process snapshot for the store
    pub fn snapshot(&self) -> ProcessSnapshot {
        ProcessSnapshot {
            process_id: self.process_id.clone(),
            name: None, // TODO: Wire up terminal naming feature
            kind: self.kind,
            visible: self.visible,
            output: self.output_buffer.clone(),
            exit_code: self.exit_code,
            started_at_ms: self.started_at_ms,
            finished_at_ms: self.exit_code.map(|_| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            }),
        }
    }

    /// Get process ID
    pub fn process_id(&self) -> &str {
        &self.process_id
    }

    /// Check if process has exited
    pub fn has_exited(&self) -> bool {
        self.exit_code.is_some()
    }

    /// Get exit code
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Dump terminal grid as plain text (for debugging/tool output)
#[allow(dead_code)]
pub fn dump_terminal_text(term: &Term<TerminalEventBridge>) -> String {
    let mut output = String::new();
    let grid = term.grid();

    for line in grid.display_iter() {
        // Only process visible lines
        if line.point.line.0 >= 0 {
            output.push(line.cell.c);
        }

        // Add newlines at the end of each row
        if line.point.column.0 == term.columns() - 1 {
            output.push('\n');
        }
    }

    output
}
