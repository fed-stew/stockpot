//! Activity type definitions
//!
//! Core types for representing activities in the feed.

use chrono::{DateTime, Local};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Global counter for unique activity IDs
static ACTIVITY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique activity ID
fn generate_id() -> String {
    let id = ACTIVITY_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("activity-{}", id)
}

/// Represents a line in a diff view
#[derive(Debug, Clone)]
pub enum DiffLine {
    /// Added line with line number and content
    Added(u32, String),
    /// Removed line with line number and content
    Removed(u32, String),
    /// Context line (unchanged) with line number and content
    Context(u32, String),
}

/// Represents a file operation within an Explored activity
#[derive(Debug, Clone)]
pub enum FileAction {
    /// Read a file
    Read(String),
    /// Listed a directory
    List(String),
}

/// A rendered line with display and copyable text
#[derive(Debug, Clone)]
pub struct RenderedLine {
    /// What's shown on screen (with bullets, connectors, formatting)
    pub display_text: String,
    /// Clean text for clipboard (no formatting artifacts)
    pub copyable_text: String,
}

impl RenderedLine {
    /// Create a new rendered line
    pub fn new(display: impl Into<String>, copyable: impl Into<String>) -> Self {
        Self {
            display_text: display.into(),
            copyable_text: copyable.into(),
        }
    }

    /// Create a line where display and copyable are the same
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            display_text: text.clone(),
            copyable_text: text,
        }
    }
}

/// Main activity types that appear in the feed
#[derive(Debug, Clone)]
pub enum Activity {
    // ─────────────────────────────────────────────────────────────────────────
    // Core activities (from rustpuppy)
    // ─────────────────────────────────────────────────────────────────────────
    /// File exploration (reads, directory listings)
    Explored {
        id: String,
        timestamp: DateTime<Local>,
        actions: Vec<FileAction>,
    },

    /// Shell command execution
    Ran {
        id: String,
        timestamp: DateTime<Local>,
        command: String,
        output: Vec<String>,
        notes: Option<String>,
    },

    /// File edit with diff
    Edited {
        id: String,
        timestamp: DateTime<Local>,
        file_path: String,
        additions: i32,
        deletions: i32,
        diff_lines: Vec<DiffLine>,
    },

    /// Streaming content (live generation)
    Streaming {
        id: String,
        timestamp: DateTime<Local>,
        title: String,
        content: String,
        elapsed: Duration,
        can_interrupt: bool,
    },

    /// Task/todo item
    Task {
        id: String,
        timestamp: DateTime<Local>,
        description: String,
        completed: bool,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Stockpot-specific activities
    // ─────────────────────────────────────────────────────────────────────────
    /// Model thinking/reasoning block
    Thinking {
        id: String,
        timestamp: DateTime<Local>,
        content: String,
        collapsed: bool,
    },

    /// Nested agent invocation
    NestedAgent {
        id: String,
        timestamp: DateTime<Local>,
        agent_name: String,
        display_name: String,
        content: String,
        collapsed: bool,
        completed: bool,
    },

    /// User message in conversation
    UserMessage {
        id: String,
        timestamp: DateTime<Local>,
        content: String,
    },

    /// Assistant response (plain text without tool calls)
    AssistantMessage {
        id: String,
        timestamp: DateTime<Local>,
        content: String,
    },
}

impl Activity {
    // ─────────────────────────────────────────────────────────────────────────
    // Constructors
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a new Explored activity
    pub fn explored(actions: Vec<FileAction>) -> Self {
        Self::Explored {
            id: generate_id(),
            timestamp: Local::now(),
            actions,
        }
    }

    /// Create a new Ran activity
    pub fn ran(command: impl Into<String>, output: Vec<String>, notes: Option<String>) -> Self {
        Self::Ran {
            id: generate_id(),
            timestamp: Local::now(),
            command: command.into(),
            output,
            notes,
        }
    }

    /// Create a new Edited activity
    pub fn edited(
        file_path: impl Into<String>,
        additions: i32,
        deletions: i32,
        diff_lines: Vec<DiffLine>,
    ) -> Self {
        Self::Edited {
            id: generate_id(),
            timestamp: Local::now(),
            file_path: file_path.into(),
            additions,
            deletions,
            diff_lines,
        }
    }

    /// Create a new Streaming activity
    pub fn streaming(title: impl Into<String>, can_interrupt: bool) -> Self {
        Self::Streaming {
            id: generate_id(),
            timestamp: Local::now(),
            title: title.into(),
            content: String::new(),
            elapsed: Duration::ZERO,
            can_interrupt,
        }
    }

    /// Create a new Task activity
    pub fn task(description: impl Into<String>) -> Self {
        Self::Task {
            id: generate_id(),
            timestamp: Local::now(),
            description: description.into(),
            completed: false,
        }
    }

    /// Create a new Thinking activity
    pub fn thinking(content: impl Into<String>) -> Self {
        Self::Thinking {
            id: generate_id(),
            timestamp: Local::now(),
            content: content.into(),
            collapsed: false,
        }
    }

    /// Create a new NestedAgent activity
    pub fn nested_agent(agent_name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self::NestedAgent {
            id: generate_id(),
            timestamp: Local::now(),
            agent_name: agent_name.into(),
            display_name: display_name.into(),
            content: String::new(),
            collapsed: false,
            completed: false,
        }
    }

    /// Create a new UserMessage activity
    pub fn user_message(content: impl Into<String>) -> Self {
        Self::UserMessage {
            id: generate_id(),
            timestamp: Local::now(),
            content: content.into(),
        }
    }

    /// Create a new AssistantMessage activity
    pub fn assistant_message(content: impl Into<String>) -> Self {
        Self::AssistantMessage {
            id: generate_id(),
            timestamp: Local::now(),
            content: content.into(),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Accessors
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the activity's unique ID
    pub fn id(&self) -> &str {
        match self {
            Self::Explored { id, .. } => id,
            Self::Ran { id, .. } => id,
            Self::Edited { id, .. } => id,
            Self::Streaming { id, .. } => id,
            Self::Task { id, .. } => id,
            Self::Thinking { id, .. } => id,
            Self::NestedAgent { id, .. } => id,
            Self::UserMessage { id, .. } => id,
            Self::AssistantMessage { id, .. } => id,
        }
    }

    /// Get the activity's timestamp
    pub fn timestamp(&self) -> &DateTime<Local> {
        match self {
            Self::Explored { timestamp, .. } => timestamp,
            Self::Ran { timestamp, .. } => timestamp,
            Self::Edited { timestamp, .. } => timestamp,
            Self::Streaming { timestamp, .. } => timestamp,
            Self::Task { timestamp, .. } => timestamp,
            Self::Thinking { timestamp, .. } => timestamp,
            Self::NestedAgent { timestamp, .. } => timestamp,
            Self::UserMessage { timestamp, .. } => timestamp,
            Self::AssistantMessage { timestamp, .. } => timestamp,
        }
    }

    /// Estimate the number of lines this activity will take when rendered
    pub fn line_count(&self) -> usize {
        match self {
            Self::Explored { actions, .. } => {
                1 + actions.len() // Header + one line per action
            }
            Self::Ran { output, notes, .. } => {
                let base = 1 + output.len().min(10); // Header + output (capped)
                base + if notes.is_some() { 1 } else { 0 }
            }
            Self::Edited { diff_lines, .. } => {
                1 + diff_lines.len().min(20) // Header + diff lines (capped)
            }
            Self::Streaming { content, .. } => {
                1 + content.lines().count().max(1) // Header + content lines
            }
            Self::Task { .. } => 1,
            Self::Thinking {
                content, collapsed, ..
            } => {
                if *collapsed {
                    1 // Just header
                } else {
                    1 + content.lines().count().max(1)
                }
            }
            Self::NestedAgent {
                content, collapsed, ..
            } => {
                if *collapsed {
                    1 // Just header
                } else {
                    1 + content.lines().count().max(1)
                }
            }
            Self::UserMessage { content, .. } => {
                1 + content.lines().count().max(1) // Header + content
            }
            Self::AssistantMessage { content, .. } => {
                1 + content.lines().count().max(1) // Header + content
            }
        }
    }

    /// Check if this activity is collapsed (for collapsible types)
    pub fn is_collapsed(&self) -> bool {
        match self {
            Self::Thinking { collapsed, .. } => *collapsed,
            Self::NestedAgent { collapsed, .. } => *collapsed,
            _ => false,
        }
    }

    /// Toggle collapsed state (for collapsible types)
    pub fn toggle_collapsed(&mut self) {
        match self {
            Self::Thinking { collapsed, .. } => *collapsed = !*collapsed,
            Self::NestedAgent { collapsed, .. } => *collapsed = !*collapsed,
            _ => {}
        }
    }

    /// Append content to streaming/message activities
    pub fn append_content(&mut self, text: &str) {
        match self {
            Self::Streaming { content, .. } => content.push_str(text),
            Self::Thinking { content, .. } => content.push_str(text),
            Self::NestedAgent { content, .. } => content.push_str(text),
            Self::AssistantMessage { content, .. } => content.push_str(text),
            _ => {}
        }
    }
}
