//! Terminal types and event definitions.

use alacritty_terminal::event::Event as AlacTermEvent;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::Color;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

/// Process kind - distinguishes LLM-initiated vs user-initiated terminals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessKind {
    /// Terminal spawned by an LLM tool call
    Llm,
    /// Terminal spawned by user action
    User,
}

/// Snapshot of a process for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub process_id: String,
    #[serde(default)] // For backward compatibility
    pub name: Option<String>, // User-friendly name for the terminal
    pub kind: ProcessKind,
    pub visible: bool,
    pub output: String,
    pub exit_code: Option<i32>,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
}

/// Request from tool to UI to execute a shell command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemExecRequest {
    ExecuteShell {
        request_id: u64,
        command: String,
        cwd: Option<String>,
    },
    KillProcess {
        request_id: u64,
        process_id: String,
    },
}

/// Response from UI to tool about execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemExecResponse {
    Started { process_id: String },
    Killed { process_id: String },
    Error { message: String },
}

/// Bridge from alacritty's event system to our async runtime
/// (Zed's ZedListener pattern)
pub struct TerminalEventBridge(pub UnboundedSender<AlacTermEvent>);

impl alacritty_terminal::event::EventListener for TerminalEventBridge {
    fn send_event(&self, event: AlacTermEvent) {
        let _ = self.0.send(event);
    }
}

/// Cached terminal content for efficient rendering
#[derive(Debug, Clone, Default)]
pub struct TerminalContent {
    /// Visible cells to render
    pub cells: Vec<IndexedCell>,
    /// Terminal mode flags
    pub mode: alacritty_terminal::term::TermMode,
    /// Cursor position and style
    pub cursor: CursorState,
    /// Current selection range (if any)
    pub selection: Option<SelectionRange>,
    /// Scroll position (0 = bottom)
    pub display_offset: usize,
    /// Terminal dimensions
    pub size: TerminalSize,
}

/// A cell with its position in the grid
#[derive(Debug, Clone)]
pub struct IndexedCell {
    pub point: Point<usize>,
    pub cell: CellContent,
}

/// Simplified cell content for rendering
#[derive(Debug, Clone)]
pub struct CellContent {
    pub character: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: Flags,
}

/// Cursor state for rendering
#[derive(Debug, Clone, Default)]
pub struct CursorState {
    pub point: Point<usize>,
    pub visible: bool,
}

/// Selection range in terminal coordinates
#[derive(Debug, Clone)]
pub struct SelectionRange {
    pub start: Point<usize>,
    pub end: Point<usize>,
}

/// Terminal dimensions
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }

    fn last_column(&self) -> Column {
        Column(self.cols.saturating_sub(1) as usize)
    }

    fn topmost_line(&self) -> Line {
        Line(0)
    }

    fn bottommost_line(&self) -> Line {
        Line(self.rows.saturating_sub(1) as i32)
    }

    fn history_size(&self) -> usize {
        10000 // Scrollback history
    }
}

/// Internal events for terminal state management
#[derive(Debug)]
pub enum InternalEvent {
    /// PTY output received
    Output(Vec<u8>),
    /// PTY closed
    Closed,
    /// Resize request
    Resize(TerminalSize),
    /// Scroll request
    Scroll(i32),
}
