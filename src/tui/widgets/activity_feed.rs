//! Activity feed widget
//!
//! The main content widget that renders activities with pill headers,
//! tree connectors, diffs, and selection highlighting.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::StatefulWidget,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::activity::{Activity, DiffLine, FileAction, RenderedLine};
use crate::tui::markdown::parse_markdown;
use crate::tui::theme::Theme;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Left pill character
pub const PILL_LEFT: &str = "◖";
/// Right pill character
pub const PILL_RIGHT: &str = "◗";
/// Tree branch connector (not last item)
pub const TREE_BRANCH: &str = "├";
/// Tree last item connector
pub const TREE_LAST: &str = "└";
/// Indent width for sub-items (matches timestamp + pill spacing)
const INDENT_WIDTH: usize = 7;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Text selection range for highlighting
#[derive(Debug, Clone, Default)]
pub struct TextSelection {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub active: bool,
}

impl TextSelection {
    /// Check if a position is within the selection
    pub fn contains(&self, line: usize, col: usize) -> bool {
        if !self.active {
            return false;
        }
        let (start_line, start_col, end_line, end_col) = if self.start_line < self.end_line
            || (self.start_line == self.end_line && self.start_col <= self.end_col)
        {
            (self.start_line, self.start_col, self.end_line, self.end_col)
        } else {
            (self.end_line, self.end_col, self.start_line, self.start_col)
        };

        if line < start_line || line > end_line {
            return false;
        }
        if line == start_line && line == end_line {
            return col >= start_col && col <= end_col;
        }
        if line == start_line {
            return col >= start_col;
        }
        if line == end_line {
            return col <= end_col;
        }
        true
    }
}

/// A styled span of text
#[derive(Debug, Clone)]
struct StyledSpan {
    text: String,
    style: Style,
}

impl StyledSpan {
    fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Internal line data for rendering
#[derive(Debug, Clone)]
struct LineData {
    /// Styled spans for display
    spans: Vec<StyledSpan>,
    /// Plain text for copying
    copyable: String,
    /// Indent for wrapped continuation lines
    continuation_indent: usize,
}

impl LineData {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            copyable: String::new(),
            continuation_indent: 0,
        }
    }

    fn push(&mut self, text: impl Into<String>, style: Style) {
        let text = text.into();
        self.copyable.push_str(&text);
        self.spans.push(StyledSpan::new(text, style));
    }

    fn push_display_only(&mut self, text: impl Into<String>, style: Style) {
        self.spans.push(StyledSpan::new(text, style));
    }

    fn with_indent(mut self, indent: usize) -> Self {
        self.continuation_indent = indent;
        self
    }

    fn display_width(&self) -> usize {
        self.spans.iter().map(|s| s.text.width()).sum()
    }

    fn has_content(&self) -> bool {
        !self.copyable.is_empty() || self.spans.iter().any(|s| !s.text.trim().is_empty())
    }
}

/// State for activity feed scrolling with cache
#[derive(Debug, Default)]
pub struct ActivityFeedState {
    /// Current scroll offset in lines
    pub scroll_offset: usize,
    /// Visible viewport height
    pub viewport_height: usize,
    /// Total content height in lines
    pub total_content_height: usize,
    /// Cached width for invalidation check
    pub cached_width: u16,
    /// Cached activity count for invalidation check
    pub cached_activity_count: usize,
    /// Whether cache is dirty and needs rebuild
    pub cache_dirty: bool,
}

impl ActivityFeedState {
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max = self
            .total_content_height
            .saturating_sub(self.viewport_height);
        self.scroll_offset = (self.scroll_offset + amount).min(max);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self
            .total_content_height
            .saturating_sub(self.viewport_height);
    }

    /// Mark cache as dirty (call when activities change)
    pub fn invalidate_cache(&mut self) {
        self.cache_dirty = true;
    }

    /// Check if cache needs rebuild
    pub fn needs_rebuild(&self, width: u16, activity_count: usize) -> bool {
        self.cache_dirty
            || self.cached_width != width
            || self.cached_activity_count != activity_count
    }

    /// Update cache metadata after rebuild
    pub fn mark_cache_valid(&mut self, width: u16, activity_count: usize) {
        self.cached_width = width;
        self.cached_activity_count = activity_count;
        self.cache_dirty = false;
    }
}

/// Widget for rendering the activity feed
pub struct ActivityFeed<'a> {
    activities: &'a [Activity],
    rendered_lines: Option<&'a mut Vec<RenderedLine>>,
    selection: Option<&'a TextSelection>,
}

impl<'a> ActivityFeed<'a> {
    /// Create a new activity feed widget
    pub fn new(activities: &'a [Activity]) -> Self {
        Self {
            activities,
            rendered_lines: None,
            selection: None,
        }
    }

    /// Set the rendered lines buffer for copy support
    pub fn rendered_lines(mut self, lines: &'a mut Vec<RenderedLine>) -> Self {
        self.rendered_lines = Some(lines);
        self
    }

    /// Set the selection for highlighting
    pub fn selection(mut self, selection: &'a TextSelection) -> Self {
        self.selection = Some(selection);
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rendering Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Render a pill-style activity header
fn render_header(activity: &Activity) -> (LineData, Color) {
    let mut line = LineData::new();
    let timestamp = activity.timestamp();
    let time_str = timestamp.format("%H:%M").to_string();

    let (action_type, suffix, color) = match activity {
        Activity::Explored { .. } => ("Read File", String::new(), Theme::ACCENT),
        Activity::Ran { command, .. } => {
            let cmd_preview = if command.len() > 40 {
                format!("{}...", &command[..37])
            } else {
                command.clone()
            };
            ("Ran", cmd_preview, Theme::ACCENT)
        }
        Activity::Edited {
            file_path,
            additions,
            deletions,
            ..
        } => {
            let suffix = format!("{} (+{} -{})", file_path, additions, deletions);
            ("Edited", suffix, Theme::ACCENT)
        }
        Activity::Streaming { title, elapsed, .. } => {
            let elapsed_secs = elapsed.as_secs();
            let suffix = if elapsed_secs > 0 {
                format!("{} ({}s)", title, elapsed_secs)
            } else {
                title.clone()
            };
            ("Streaming", suffix, Theme::ACCENT)
        }
        Activity::Task {
            description,
            completed,
            ..
        } => {
            let prefix = if *completed { "✓ Done" } else { "○ Task" };
            let color = if *completed {
                Theme::GREEN
            } else {
                Theme::YELLOW
            };
            (prefix, description.clone(), color)
        }
        Activity::Thinking { .. } => ("Thinking", String::new(), Theme::THINKING),
        Activity::NestedAgent { display_name, .. } => ("Agent", display_name.clone(), Theme::AGENT),
        Activity::UserMessage { .. } => ("You", String::new(), Theme::ACCENT),
        Activity::AssistantMessage { .. } => ("Assistant", String::new(), Theme::MUTED),
    };

    let pill_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let muted = Style::default().fg(Theme::MUTED);
    let text_style = Style::default().fg(Theme::TEXT);

    // Timestamp
    line.push(&time_str, muted);
    line.push(" ", Style::default());

    // Pill
    line.push(PILL_LEFT, pill_style);
    line.push(format!(" {} ", action_type), pill_style);
    line.push(PILL_RIGHT, pill_style);

    // Suffix if present
    if !suffix.is_empty() {
        line.push(" ", Style::default());
        line.push(&suffix, text_style);
    }

    (line.with_indent(INDENT_WIDTH), color)
}

/// Render tree-style sub-items
fn render_tree_items(items: &[String], is_last_fn: impl Fn(usize) -> bool) -> Vec<LineData> {
    let muted = Style::default().fg(Theme::MUTED);
    let text_style = Style::default().fg(Theme::TEXT);

    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let mut line = LineData::new();
            let connector = if is_last_fn(i) {
                TREE_LAST
            } else {
                TREE_BRANCH
            };

            // Indent + connector
            line.push_display_only(" ".repeat(INDENT_WIDTH - 2), Style::default());
            line.push_display_only(connector, muted);
            line.push_display_only(" ", Style::default());

            // Content
            line.push(item, text_style);

            line.with_indent(INDENT_WIDTH)
        })
        .collect()
}

/// Render diff lines with colors
fn render_diff_lines(diff_lines: &[DiffLine]) -> Vec<LineData> {
    diff_lines
        .iter()
        .map(|diff| {
            let mut line = LineData::new();
            let (line_num, prefix, content, color) = match diff {
                DiffLine::Added(n, c) => (*n, "+", c.as_str(), Theme::GREEN),
                DiffLine::Removed(n, c) => (*n, "-", c.as_str(), Theme::RED),
                DiffLine::Context(n, c) => (*n, " ", c.as_str(), Theme::MUTED),
            };

            let line_style = Style::default().fg(color);
            let num_style = Style::default().fg(Theme::LINE_NUM);

            // Indent + line number
            line.push_display_only(" ".repeat(INDENT_WIDTH - 1), Style::default());
            line.push_display_only(format!("{:4} ", line_num), num_style);

            // Prefix and content
            line.push(prefix, line_style);
            line.push(content, line_style);

            line.with_indent(INDENT_WIDTH + 5)
        })
        .collect()
}

/// Render content lines (for messages, thinking, etc.)
/// Uses markdown parsing when use_markdown is true
fn render_content_lines(content: &str, _base_style: Style) -> Vec<LineData> {
    render_markdown_content(content)
}

/// Render content with markdown formatting
/// Parses the ENTIRE content at once to support multi-line constructs (tables, code blocks)
fn render_markdown_content(content: &str) -> Vec<LineData> {
    let mut lines = Vec::new();

    // Parse the entire content at once - this allows tables and code blocks to work!
    let all_spans = parse_markdown(content);

    // Split spans into lines based on \n characters
    let mut current_line = LineData::new();
    current_line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());

    for span in all_spans {
        // Check if this span contains newlines
        if span.text.contains('\n') {
            let parts: Vec<&str> = span.text.split('\n').collect();
            for (i, part) in parts.iter().enumerate() {
                if !part.is_empty() {
                    current_line.push(*part, span.style);
                }
                if i < parts.len() - 1 {
                    // Newline encountered - finalize current line and start new one
                    lines.push(current_line.with_indent(INDENT_WIDTH));
                    current_line = LineData::new();
                    current_line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());
                }
            }
        } else {
            current_line.push(&span.text, span.style);
        }
    }

    // Don't forget the last line if it has content
    if current_line.has_content() {
        lines.push(current_line.with_indent(INDENT_WIDTH));
    }

    // Handle empty content - ensure at least one line
    if lines.is_empty() {
        let mut line = LineData::new();
        line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());
        lines.push(line.with_indent(INDENT_WIDTH));
    }

    lines
}

/// Render output lines (for Ran activity)
fn render_output_lines(output: &[String], max_lines: usize) -> Vec<LineData> {
    let muted = Style::default().fg(Theme::MUTED);
    let mut lines: Vec<LineData> = output
        .iter()
        .take(max_lines)
        .map(|text| {
            let mut line = LineData::new();
            line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());
            line.push(text, muted);
            line.with_indent(INDENT_WIDTH)
        })
        .collect();

    if output.len() > max_lines {
        let mut line = LineData::new();
        line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());
        line.push(
            format!("... ({} more lines)", output.len() - max_lines),
            Style::default().fg(Theme::MUTED),
        );
        lines.push(line.with_indent(INDENT_WIDTH));
    }

    lines
}

/// Build all lines for an activity
fn build_activity_lines(activity: &Activity) -> Vec<LineData> {
    let mut lines = Vec::new();
    let (header, _color) = render_header(activity);
    lines.push(header);

    match activity {
        Activity::Explored { actions, .. } => {
            let items: Vec<String> = actions
                .iter()
                .map(|a| match a {
                    FileAction::Read(path) => format!("Read {}", path),
                    FileAction::List(dir) => format!("List {}", dir),
                })
                .collect();
            let len = items.len();
            lines.extend(render_tree_items(&items, |i| i == len - 1));
        }
        Activity::Ran { output, notes, .. } => {
            lines.extend(render_output_lines(output, 10));
            if let Some(note) = notes {
                let mut line = LineData::new();
                line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());
                line.push(
                    format!("Note: {}", note),
                    Style::default().fg(Theme::YELLOW),
                );
                lines.push(line.with_indent(INDENT_WIDTH));
            }
        }
        Activity::Edited { diff_lines, .. } => {
            let capped: Vec<_> = diff_lines.iter().take(20).cloned().collect();
            lines.extend(render_diff_lines(&capped));
            if diff_lines.len() > 20 {
                let mut line = LineData::new();
                line.push_display_only(" ".repeat(INDENT_WIDTH), Style::default());
                line.push(
                    format!("... ({} more lines)", diff_lines.len() - 20),
                    Style::default().fg(Theme::MUTED),
                );
                lines.push(line.with_indent(INDENT_WIDTH));
            }
        }
        Activity::Streaming { content, .. } => {
            let text_style = Style::default().fg(Theme::TEXT);
            lines.extend(render_content_lines(content, text_style));
        }
        Activity::Task { .. } => {
            // Single line, header only
        }
        Activity::Thinking {
            content, collapsed, ..
        } => {
            if !*collapsed {
                let style = Style::default().fg(Theme::MUTED);
                lines.extend(render_content_lines(content, style));
            }
        }
        Activity::NestedAgent {
            content, collapsed, ..
        } => {
            if !*collapsed {
                let style = Style::default().fg(Theme::MUTED);
                lines.extend(render_content_lines(content, style));
            }
        }
        Activity::UserMessage { content, .. } => {
            let style = Style::default().fg(Theme::TEXT);
            lines.extend(render_content_lines(content, style));
        }
        Activity::AssistantMessage { content, .. } => {
            let style = Style::default().fg(Theme::TEXT);
            lines.extend(render_content_lines(content, style));
        }
    }

    // Add spacing after activity
    lines.push(LineData::new());

    lines
}

/// Wrap a line to fit within width, returning visual lines
fn wrap_line(line: &LineData, width: usize) -> Vec<(String, Vec<StyledSpan>)> {
    if width == 0 {
        return vec![];
    }

    let total_width = line.display_width();
    if total_width <= width {
        // No wrapping needed
        let display: String = line.spans.iter().map(|s| s.text.as_str()).collect();
        return vec![(display, line.spans.clone())];
    }

    // Need to wrap - simplified approach
    let mut result = Vec::new();
    let mut current_line_spans = Vec::new();
    let mut current_width = 0;
    let mut is_first_line = true;

    for span in &line.spans {
        let mut remaining = span.text.as_str();

        while !remaining.is_empty() {
            let available = if is_first_line {
                width
            } else {
                width.saturating_sub(line.continuation_indent)
            };

            if current_width >= available {
                // Start new line
                let display: String = current_line_spans
                    .iter()
                    .map(|s: &StyledSpan| s.text.as_str())
                    .collect();
                result.push((display, current_line_spans));
                current_line_spans = Vec::new();
                current_width = 0;
                is_first_line = false;

                // Add continuation indent
                if line.continuation_indent > 0 {
                    current_line_spans.push(StyledSpan::new(
                        " ".repeat(line.continuation_indent),
                        Style::default(),
                    ));
                    current_width = line.continuation_indent;
                }
            }

            let space_left = available.saturating_sub(current_width);

            // Take characters by display width, not by count
            // This properly handles wide characters (emojis, CJK) that take 2 cells
            let mut taken_width = 0;
            let mut take_chars = String::new();
            let mut byte_offset = 0;

            for ch in remaining.chars() {
                let ch_width = UnicodeWidthChar::width(ch).unwrap_or(1);
                if taken_width + ch_width > space_left {
                    break;
                }
                take_chars.push(ch);
                taken_width += ch_width;
                byte_offset += ch.len_utf8();
            }

            if !take_chars.is_empty() {
                current_line_spans.push(StyledSpan::new(&take_chars, span.style));
                current_width += taken_width;
                remaining = &remaining[byte_offset..];
            } else {
                // Can't fit even one char, force newline
                if !current_line_spans.is_empty() {
                    let display: String = current_line_spans
                        .iter()
                        .map(|s: &StyledSpan| s.text.as_str())
                        .collect();
                    result.push((display, current_line_spans));
                    current_line_spans = Vec::new();
                    current_width = 0;
                    is_first_line = false;
                }
                break;
            }
        }
    }

    // Push remaining content
    if !current_line_spans.is_empty() {
        let display: String = current_line_spans
            .iter()
            .map(|s: &StyledSpan| s.text.as_str())
            .collect();
        result.push((display, current_line_spans));
    }

    result
}

impl StatefulWidget for ActivityFeed<'_> {
    type State = ActivityFeedState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(Theme::BG);
            }
        }

        // Check if we need to rebuild (for future cache optimization)
        let _needs_rebuild = state.needs_rebuild(area.width, self.activities.len());

        // Build all logical lines
        // TODO: Cache visual_lines when !needs_rebuild for performance
        let mut all_lines: Vec<LineData> = Vec::new();
        for activity in self.activities {
            all_lines.extend(build_activity_lines(activity));
        }

        // Wrap lines and build visual lines
        let width = area.width as usize;
        let mut visual_lines: Vec<(String, Vec<StyledSpan>, String)> = Vec::new();

        for line in &all_lines {
            let wrapped = wrap_line(line, width);
            for (display, spans) in wrapped {
                visual_lines.push((display, spans, line.copyable.clone()));
            }
        }

        // Update state
        state.total_content_height = visual_lines.len();
        state.viewport_height = area.height as usize;

        // Clear rendered lines buffer
        if let Some(ref mut rendered) = self.rendered_lines {
            rendered.clear();
        }

        // Render visible lines
        let start = state.scroll_offset;
        let end = (start + area.height as usize).min(visual_lines.len());

        for (visual_row, (display, spans, copyable)) in visual_lines
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start)
        {
            let screen_row = area.y + (visual_row - start) as u16;

            // Store rendered line for copy support
            if let Some(ref mut rendered) = self.rendered_lines {
                rendered.push(RenderedLine::new(display.clone(), copyable.clone()));
            }

            // Render spans with selection highlighting
            let mut col = area.x;
            for span in spans {
                for ch in span.text.chars() {
                    // Get the display width of this character (wide chars like emoji/CJK are 2 cells)
                    let char_width = UnicodeWidthChar::width(ch).unwrap_or(1);

                    // Check if there's room for the full character before rendering
                    if col + char_width as u16 > area.x + area.width {
                        break;
                    }

                    let mut style = span.style;

                    // Apply selection highlighting
                    if let Some(sel) = self.selection {
                        if sel.contains(visual_row, col as usize) {
                            style = style.bg(Theme::SELECTION);
                        }
                    }

                    buf[(col, screen_row)].set_char(ch).set_style(style);
                    // Advance by the actual display width (wide chars take 2 cells,
                    // terminal automatically creates continuation cell)
                    col += char_width as u16;
                }
            }
        }

        // Update cache metadata
        state.mark_cache_valid(area.width, self.activities.len());
    }
}
