use std::time::Instant;

use gpui::*;
use pulldown_cmark::{
    Alignment as CmarkAlignment, Event as CmarkEvent, Options as CmarkOptions,
    Parser as CmarkParser, Tag, TagEnd,
};
use streamdown_parser::{ParseEvent, Parser};

use super::selectable_text::SelectableText;
use crate::gui::theme::Theme;

/// Steady character reveal rate (characters per second).
const REVEAL_CHARS_PER_SECOND: f32 = 500.0;

/// If the reveal buffer exceeds this many characters, flush immediately.
const MAX_REVEAL_BUFFER: usize = 600;

/// Duration (ms) for the fade-in on new blocks and the pending view.
/// Kept short so text reaches readable opacity within a few frames.
const FADE_IN_MS: f32 = 80.0;

pub struct StreamingMarkdownView {
    parser: Parser,
    line_buffer: String,
    blocks: Vec<Block>,
    text_views: Vec<Entity<SelectableText>>,
    theme: Theme,
    source_text: String,
    /// Transient view for the current partial line (before newline).
    pending_view: Option<Entity<SelectableText>>,
    /// Smooth-reveal character buffer.
    reveal_buffer: String,
    last_drain_at: Instant,
    prev_pending_len: usize,
    /// When the pending view was first created (drives its fade-in).
    pending_created_at: Option<Instant>,
    /// Birth timestamps for each block (parallel to `blocks`).
    /// Set once on creation, NEVER reset on extend — drives per-block fade.
    block_born_at: Vec<Instant>,
    /// Accumulated raw lines that look like a table (start with `|`).
    /// Buffered silently during streaming — NO rendering until an empty
    /// line (or non-table line) signals the table is complete, at which
    /// point pulldown_cmark parses and converts to Block::Table.
    table_line_buffer: Vec<String>,
}

#[derive(Debug)]
enum Block {
    Text(String),
    Table(TableData),
    Divider,
}

/// Column alignment for table cells.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ColumnAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
struct TableData {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    /// Per-column alignment. May be shorter than headers if not specified.
    alignments: Vec<ColumnAlign>,
}

// TableBuilder removed — tables are now buffered as raw lines in
// table_line_buffer and parsed by pulldown_cmark when complete.

impl StreamingMarkdownView {
    pub fn new(theme: Theme) -> Self {
        Self {
            parser: Parser::new(),
            line_buffer: String::new(),
            blocks: Vec::new(),
            text_views: Vec::new(),
            theme,
            source_text: String::new(),
            pending_view: None,
            reveal_buffer: String::new(),
            last_drain_at: Instant::now(),
            prev_pending_len: 0,
            pending_created_at: None,
            block_born_at: Vec::new(),
            table_line_buffer: Vec::new(),
        }
    }

    pub fn update_content(&mut self, content: &str, cx: &mut Context<Self>) {
        if !self.reveal_buffer.is_empty() {
            let flushed = std::mem::take(&mut self.reveal_buffer);
            self.append_content(&flushed, cx);
        }

        if content.len() > self.source_text.len() && content.starts_with(&self.source_text) {
            let new_part = &content[self.source_text.len()..];
            self.append_content(new_part, cx);
            self.source_text = content.to_string();
        } else if content != self.source_text {
            self.reset(cx);
            self.append_content(content, cx);
            self.source_text = content.to_string();
        }

        // Flush partial line as text ONLY when not buffering a table.
        if !self.line_buffer.is_empty() && self.table_line_buffer.is_empty() {
            let trimmed = self.line_buffer.trim();
            if !trimmed.starts_with('|') {
                let remaining = std::mem::take(&mut self.line_buffer);
                self.append_text(&remaining, cx);
                self.pending_view = None;
                self.prev_pending_len = 0;
                self.pending_created_at = None;
            }
        }
    }

    pub fn append_delta(&mut self, delta: &str, cx: &mut Context<Self>) {
        self.source_text.push_str(delta);
        self.reveal_buffer.push_str(delta);
        cx.notify();
    }

    /// Finalize any in-progress table and extract tables from text blocks.
    /// Called when we know no more streaming content is coming (e.g., before
    /// reset, or when the message is complete).
    pub fn finalize_tables(&mut self, cx: &mut Context<Self>) {
        // Flush any buffered table lines
        self.flush_table_buffer(cx);

        // Flush remaining line buffer as text
        if !self.line_buffer.is_empty() {
            let remaining = std::mem::take(&mut self.line_buffer);
            self.append_text(&remaining, cx);
        }

        // Extract tables from any Block::Text that contains table markdown
        self.extract_embedded_tables(cx);

        self.pending_view = None;
        self.prev_pending_len = 0;
        self.pending_created_at = None;
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.finalize_tables(cx);

        self.line_buffer.clear();
        self.blocks.clear();
        self.text_views.clear();
        self.source_text.clear();
        self.parser = Parser::new();
        self.pending_view = None;
        self.reveal_buffer.clear();
        self.last_drain_at = Instant::now();
        self.prev_pending_len = 0;
        self.pending_created_at = None;
        self.block_born_at.clear();
        self.table_line_buffer.clear();
        cx.notify();
    }

    fn drain_reveal_buffer(&mut self, cx: &mut Context<Self>) {
        if self.reveal_buffer.is_empty() {
            return;
        }

        if self.reveal_buffer.len() > MAX_REVEAL_BUFFER {
            let all = std::mem::take(&mut self.reveal_buffer);
            self.append_content(&all, cx);
            self.last_drain_at = Instant::now();
            return;
        }

        let elapsed = self.last_drain_at.elapsed().as_secs_f32();
        let chars_to_release = (elapsed * REVEAL_CHARS_PER_SECOND).ceil() as usize;
        let chars_to_release = chars_to_release.max(1);
        let release_len = chars_to_release.min(self.reveal_buffer.len());

        let mut end = release_len;
        while end < self.reveal_buffer.len() && !self.reveal_buffer.is_char_boundary(end) {
            end += 1;
        }
        end = end.min(self.reveal_buffer.len());

        if end > 0 {
            let chunk: String = self.reveal_buffer.drain(..end).collect();
            self.append_content(&chunk, cx);
            self.last_drain_at = Instant::now();
        }
    }

    pub fn append_content(&mut self, content: &str, cx: &mut Context<Self>) {
        self.line_buffer.push_str(content);

        while let Some(newline_idx) = self.line_buffer.find('\n') {
            self.pending_view = None;
            self.prev_pending_len = 0;
            self.pending_created_at = None;

            let line: String = self.line_buffer.drain(..=newline_idx).collect();
            let line_content = line.trim_end_matches('\n').trim_end_matches('\r');

            // Is this line a table row? (starts with |)
            let is_table_line = line_content.trim().starts_with('|');
            // Is this an empty line?
            let is_empty = line_content.trim().is_empty();

            if !self.table_line_buffer.is_empty() {
                // We're buffering a table.
                if is_table_line {
                    // More table content — keep buffering silently.
                    self.table_line_buffer.push(line);
                    continue;
                } else {
                    // Non-table line (empty line or other content) →
                    // the table is complete. Flush it, then process this line.
                    self.flush_table_buffer(cx);
                    // Fall through to process the current line normally.
                }
            }

            if is_table_line && self.table_line_buffer.is_empty() {
                // First `|`-prefixed line — start buffering a potential table.
                self.table_line_buffer.push(line);
                continue;
            }

            // Normal (non-table) line — parse with streamdown_parser.
            let events = self.parser.parse_line(line_content);

            let is_rule = events
                .iter()
                .any(|e| matches!(e, ParseEvent::HorizontalRule));

            if is_rule {
                self.blocks.push(Block::Divider);
                self.block_born_at.push(Instant::now());
            } else if !is_empty || !self.blocks.is_empty() {
                // Append non-empty lines (or empty lines after content) as text
                self.append_text(&line, cx);
            }
        }

        // Pending view: show partial line only if it doesn't look like a table
        if !self.line_buffer.is_empty() {
            let trimmed = self.line_buffer.trim();
            if !trimmed.starts_with('|') && self.table_line_buffer.is_empty() {
                let buf = self.line_buffer.clone();
                if let Some(ref view) = self.pending_view {
                    view.update(cx, |v, cx| {
                        v.set_content(buf, cx);
                    });
                } else {
                    let theme = self.theme.clone();
                    let view = cx.new(move |cx| SelectableText::new(cx, buf, theme));
                    self.pending_view = Some(view);
                    self.pending_created_at = Some(Instant::now());
                }
                self.prev_pending_len = self.line_buffer.len();
            } else {
                self.pending_view = None;
                self.prev_pending_len = 0;
                self.pending_created_at = None;
            }
        } else {
            self.pending_view = None;
            self.prev_pending_len = 0;
            self.pending_created_at = None;
        }

        cx.notify();
    }

    /// Flush the table line buffer: join all buffered lines into one string,
    /// run pulldown_cmark to detect a proper GFM table, and emit either a
    /// Block::Table (if valid) or Block::Text (if not actually a table).
    fn flush_table_buffer(&mut self, cx: &mut Context<Self>) {
        if self.table_line_buffer.is_empty() {
            return;
        }

        let raw = self.table_line_buffer.join("");
        self.table_line_buffer.clear();

        // Try to parse as a GFM table with pulldown_cmark
        let mut opts = CmarkOptions::empty();
        opts.insert(CmarkOptions::ENABLE_TABLES);
        opts.insert(CmarkOptions::ENABLE_STRIKETHROUGH);

        let parser = CmarkParser::new_ext(&raw, opts).into_offset_iter();

        let mut headers: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut alignments: Vec<ColumnAlign> = Vec::new();
        let mut in_table = false;
        let mut in_head = false;
        let mut current_cell = String::new();
        let mut current_row: Vec<String> = Vec::new();

        for (event, _range) in parser {
            match event {
                CmarkEvent::Start(Tag::Table(aligns)) => {
                    in_table = true;
                    alignments = aligns
                        .into_iter()
                        .map(|a| match a {
                            CmarkAlignment::Center => ColumnAlign::Center,
                            CmarkAlignment::Right => ColumnAlign::Right,
                            _ => ColumnAlign::Left,
                        })
                        .collect();
                }
                CmarkEvent::Start(Tag::TableHead) => {
                    in_head = true;
                    current_row.clear();
                }
                CmarkEvent::Start(Tag::TableRow) => {
                    current_row.clear();
                }
                CmarkEvent::Start(Tag::TableCell) => {
                    current_cell.clear();
                }
                CmarkEvent::Text(t) if in_table => {
                    current_cell.push_str(&t);
                }
                CmarkEvent::Code(t) if in_table => {
                    current_cell.push_str(&t);
                }
                CmarkEvent::SoftBreak if in_table => {
                    current_cell.push(' ');
                }
                CmarkEvent::End(TagEnd::TableCell) => {
                    current_row.push(current_cell.trim().to_string());
                    current_cell.clear();
                }
                CmarkEvent::End(TagEnd::TableHead) => {
                    headers = current_row.clone();
                    current_row.clear();
                    in_head = false;
                }
                CmarkEvent::End(TagEnd::TableRow) if !in_head => {
                    rows.push(current_row.clone());
                    current_row.clear();
                }
                CmarkEvent::End(TagEnd::Table) => {
                    in_table = false;
                }
                _ => {}
            }
        }

        if !headers.is_empty() {
            // Valid table — push as Block::Table
            self.blocks.push(Block::Table(TableData {
                headers,
                rows,
                alignments,
            }));
            self.block_born_at.push(Instant::now());
        } else {
            // Not a valid table — push as regular text
            self.append_text(&raw, cx);
        }
    }

    // Table handling is now done entirely via table_line_buffer + flush_table_buffer().
    // Lines starting with `|` are buffered. When a non-`|` line arrives (or message
    // ends), the buffer is parsed by pulldown_cmark to create Block::Table.

    #[allow(unused)]
    fn append_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(Block::Text(current_text)) = self.blocks.last_mut() {
            current_text.push_str(text);
            // Extend existing block — do NOT update block_born_at (old text stays solid)
            if let Some(last_view) = self.text_views.last() {
                last_view.update(cx, |view, cx| {
                    view.append(text, cx);
                });
            } else {
                let theme = self.theme.clone();
                let text_owned = text.to_string();
                let view = cx.new(move |cx| SelectableText::new(cx, text_owned, theme));
                self.text_views.push(view);
            }
        } else {
            // New text block — set birth timestamp for fade-in
            self.blocks.push(Block::Text(text.to_string()));
            self.block_born_at.push(Instant::now());
            let theme = self.theme.clone();
            let text_owned = text.to_string();
            let view = cx.new(move |cx| SelectableText::new(cx, text_owned, theme));
            self.text_views.push(view);
        }
    }

    /// Compute opacity for a block/element that was born at `born_at`.
    fn fade_opacity(born_at: Instant) -> (f32, bool) {
        let elapsed = born_at.elapsed().as_secs_f32() * 1000.0;
        let progress = (elapsed / FADE_IN_MS).min(1.0);
        let still_fading = progress < 1.0;
        (progress, still_fading)
    }

    /// Scan all `Block::Text` entries for embedded tables that the streaming
    /// parser missed, and split them into `Block::Text` + `Block::Table` segments.
    /// Uses pulldown_cmark with `ENABLE_TABLES` for reliable GFM detection.
    fn extract_embedded_tables(&mut self, cx: &mut Context<Self>) {
        let mut i = 0;
        while i < self.blocks.len() {
            if let Block::Text(ref content) = self.blocks[i] {
                let content = content.clone();
                if let Some(segments) = Self::split_text_with_tables(&content) {
                    // Remove the original text block and its view
                    self.blocks.remove(i);
                    self.block_born_at.remove(i);
                    // Find and remove the corresponding text_view.
                    // text_views are 1:1 with Block::Text entries.
                    let text_view_idx = self.blocks[..i]
                        .iter()
                        .filter(|b| matches!(b, Block::Text(_)))
                        .count();
                    if text_view_idx < self.text_views.len() {
                        self.text_views.remove(text_view_idx);
                    }

                    // Insert the split segments
                    for seg in segments {
                        let ts = Instant::now();
                        match seg {
                            Block::Text(ref t) if t.trim().is_empty() => {
                                // Skip empty text blocks
                            }
                            Block::Text(ref t) => {
                                let theme = self.theme.clone();
                                let text_owned = t.clone();
                                // Insert text_view at the right position
                                let tv_idx = self.blocks[..i]
                                    .iter()
                                    .filter(|b| matches!(b, Block::Text(_)))
                                    .count();
                                let view =
                                    cx.new(move |cx| SelectableText::new(cx, text_owned, theme));
                                self.text_views.insert(tv_idx, view);
                                self.blocks.insert(i, seg);
                                self.block_born_at.insert(i, ts);
                                i += 1;
                            }
                            _ => {
                                self.blocks.insert(i, seg);
                                self.block_born_at.insert(i, ts);
                                i += 1;
                            }
                        }
                    }
                    // Don't increment i — we already advanced past inserted segments
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Check if a text block contains a GFM table. If so, split it into
    /// segments: text before the table, the table itself, text after.
    /// Returns None if no table is found.
    fn split_text_with_tables(content: &str) -> Option<Vec<Block>> {
        let mut opts = CmarkOptions::empty();
        opts.insert(CmarkOptions::ENABLE_TABLES);
        opts.insert(CmarkOptions::ENABLE_STRIKETHROUGH);

        // Quick check: does pulldown_cmark see any table events?
        let parser = CmarkParser::new_ext(content, opts);
        let has_table = parser
            .into_iter()
            .any(|e| matches!(e, CmarkEvent::Start(Tag::Table(_))));
        if !has_table {
            return None;
        }

        // Full parse: extract table structure and surrounding text
        let parser = CmarkParser::new_ext(content, opts).into_offset_iter();

        let mut segments: Vec<Block> = Vec::new();
        let mut last_end: usize = 0;
        let mut in_table = false;
        let mut in_head = false;
        let mut headers: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut alignments: Vec<ColumnAlign> = Vec::new();
        let mut current_cell = String::new();
        let mut current_row: Vec<String> = Vec::new();

        for (event, range) in parser {
            match event {
                CmarkEvent::Start(Tag::Table(cmark_aligns)) => {
                    if range.start > last_end {
                        let before = &content[last_end..range.start];
                        if !before.trim().is_empty() {
                            segments.push(Block::Text(before.to_string()));
                        }
                    }
                    in_table = true;
                    headers.clear();
                    rows.clear();
                    alignments = cmark_aligns
                        .into_iter()
                        .map(|a| match a {
                            CmarkAlignment::Center => ColumnAlign::Center,
                            CmarkAlignment::Right => ColumnAlign::Right,
                            _ => ColumnAlign::Left,
                        })
                        .collect();
                }
                CmarkEvent::Start(Tag::TableHead) => {
                    in_head = true;
                    current_row.clear();
                }
                CmarkEvent::Start(Tag::TableRow) => {
                    current_row.clear();
                }
                CmarkEvent::Start(Tag::TableCell) => {
                    current_cell.clear();
                }
                CmarkEvent::Text(t) if in_table => {
                    current_cell.push_str(&t);
                }
                CmarkEvent::Code(t) if in_table => {
                    current_cell.push_str(&t);
                }
                CmarkEvent::End(TagEnd::TableCell) => {
                    current_row.push(current_cell.trim().to_string());
                    current_cell.clear();
                }
                CmarkEvent::End(TagEnd::TableHead) => {
                    headers = current_row.clone();
                    current_row.clear();
                    in_head = false;
                }
                CmarkEvent::End(TagEnd::TableRow) if !in_head => {
                    rows.push(current_row.clone());
                    current_row.clear();
                }
                CmarkEvent::End(TagEnd::Table) => {
                    segments.push(Block::Table(TableData {
                        headers: headers.clone(),
                        rows: rows.clone(),
                        alignments: alignments.clone(),
                    }));
                    last_end = range.end;
                    in_table = false;
                }
                _ => {}
            }
        }

        // Push any text after the last table
        if last_end < content.len() {
            let after = &content[last_end..];
            if !after.trim().is_empty() {
                segments.push(Block::Text(after.to_string()));
            }
        }

        if segments.is_empty() {
            None
        } else {
            Some(segments)
        }
    }

    /// Build a GPUI table div from headers, rows, and column alignments.
    /// Used for both completed `Block::Table` and the in-progress builder.
    fn render_table_div(
        headers: &[String],
        rows: &[Vec<String>],
        alignments: &[ColumnAlign],
        theme: &Theme,
    ) -> gpui::Div {
        let mut table_div = div()
            .flex()
            .flex_col()
            .w_full()
            .border_1()
            .border_color(theme.border);

        if !headers.is_empty() {
            let mut header_row = div()
                .flex()
                .flex_row()
                .w_full()
                .bg(theme.panel_background)
                .border_b_1()
                .border_color(theme.border);
            for (i, header) in headers.iter().enumerate() {
                let is_last = i == headers.len() - 1;
                let align = alignments.get(i).copied().unwrap_or(ColumnAlign::Left);
                let mut cell = Self::table_cell(align)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.text);
                if !is_last {
                    cell = cell.border_r_1().border_color(theme.border);
                }
                header_row = header_row.child(cell.child(header.clone()));
            }
            table_div = table_div.child(header_row);
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let is_odd = row_idx % 2 == 1;
            let mut row_div = div().flex().flex_row().w_full();
            if is_odd {
                row_div = row_div.bg(theme.panel_background);
            }
            if row_idx < rows.len() - 1 {
                row_div = row_div.border_b_1().border_color(theme.border);
            }
            for (i, cell_text) in row.iter().enumerate() {
                let is_last = i == row.len() - 1;
                let align = alignments.get(i).copied().unwrap_or(ColumnAlign::Left);
                let mut cell = Self::table_cell(align).text_color(theme.text);
                if !is_last {
                    cell = cell.border_r_1().border_color(theme.border);
                }
                row_div = row_div.child(cell.child(cell_text.clone()));
            }
            table_div = table_div.child(row_div);
        }

        table_div
    }

    /// Create a table cell div with the correct alignment and equal-width flex.
    /// Uses block layout so text wraps naturally within the cell, increasing
    /// row height as needed.
    fn table_cell(align: ColumnAlign) -> gpui::Div {
        let cell = div()
            .flex_basis(relative(0.))
            .flex_grow()
            .flex_shrink()
            .min_w(px(0.))
            .px_2()
            .py_1();

        // Text alignment via the GPUI text_align-like pattern:
        // wrap content in a flex-col + items alignment
        match align {
            ColumnAlign::Left => cell,
            ColumnAlign::Center => cell.flex().flex_col().items_center(),
            ColumnAlign::Right => cell.flex().flex_col().items_end(),
        }
    }
}

impl Render for StreamingMarkdownView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drain_reveal_buffer(cx);

        let mut col = div().flex().flex_col().w_full();
        let mut text_view_iter = self.text_views.iter();
        let mut needs_animation = !self.reveal_buffer.is_empty();

        for (block_idx, block) in self.blocks.iter().enumerate() {
            let (block_opacity, fading) = self
                .block_born_at
                .get(block_idx)
                .map(|t| Self::fade_opacity(*t))
                .unwrap_or((1.0, false));
            if fading {
                needs_animation = true;
            }

            match block {
                Block::Text(_) => {
                    if let Some(view) = text_view_iter.next() {
                        col = col.child(div().opacity(block_opacity).child(view.clone()));
                    }
                }
                Block::Divider => {
                    col = col.child(
                        div()
                            .opacity(block_opacity)
                            .w_full()
                            .h(px(1.))
                            .bg(self.theme.border)
                            .my(px(8.)),
                    );
                }
                Block::Table(data) => {
                    let table_div = Self::render_table_div(
                        &data.headers,
                        &data.rows,
                        &data.alignments,
                        &self.theme,
                    );
                    col = col.child(div().opacity(block_opacity).child(table_div.my(px(4.))));
                }
            }
        }

        // NOTE: In-progress tables (current_table) are NOT rendered here.
        // Showing a partial table during streaming breaks the layout. The table
        // stays hidden in the builder until TableEnd fires (or finalize_tables()
        // is called), then appears as a complete Block::Table.

        // Pending partial line with fade-in (suppressed during table building)
        if let Some(ref view) = self.pending_view {
            let (pending_opacity, fading) = self
                .pending_created_at
                .map(Self::fade_opacity)
                .unwrap_or((1.0, false));
            if fading {
                needs_animation = true;
            }
            col = col.child(div().opacity(pending_opacity).child(view.clone()));
        }

        if needs_animation {
            cx.notify();
        }

        col
    }
}
