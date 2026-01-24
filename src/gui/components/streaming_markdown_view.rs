use gpui::*;
use streamdown_parser::{ParseEvent, Parser};
// use pulldown_cmark::{Event, Tag, TagEnd};

use super::selectable_text::SelectableText;
use crate::gui::theme::Theme;

pub struct StreamingMarkdownView {
    parser: Parser,
    line_buffer: String,
    blocks: Vec<Block>,
    text_views: Vec<Entity<SelectableText>>,
    theme: Theme,
    current_table: Option<TableBuilder>,
    source_text: String,
}

#[derive(Debug)]
enum Block {
    Text(String),
    Table(TableData),
    Divider,
}

#[derive(Debug, Clone)]
struct TableData {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl StreamingMarkdownView {
    pub fn new(theme: Theme) -> Self {
        Self {
            parser: Parser::new(),
            line_buffer: String::new(),
            blocks: Vec::new(),
            text_views: Vec::new(),
            theme,
            current_table: None,
            source_text: String::new(),
        }
    }

    pub fn update_content(&mut self, content: &str, cx: &mut Context<Self>) {
        if content.len() > self.source_text.len() && content.starts_with(&self.source_text) {
            let new_part = &content[self.source_text.len()..];
            self.append_content(new_part, cx);
            self.source_text = content.to_string();
        } else if content != self.source_text {
            self.reset(cx);
            self.append_content(content, cx);
            self.source_text = content.to_string();
        }
    }

    pub fn append_delta(&mut self, delta: &str, cx: &mut Context<Self>) {
        self.source_text.push_str(delta);
        self.append_content(delta, cx);
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.line_buffer.clear();
        self.blocks.clear();
        self.text_views.clear();
        self.source_text.clear();
        self.parser = Parser::new();
        self.current_table = None;
        cx.notify();
    }

    pub fn append_content(&mut self, content: &str, cx: &mut Context<Self>) {
        self.line_buffer.push_str(content);

        while let Some(newline_idx) = self.line_buffer.find('\n') {
            let line: String = self.line_buffer.drain(..=newline_idx).collect();
            let line_content = line.trim_end_matches('\n').trim_end_matches('\r');

            // If we are inside a table, process events to build structured data
            if self.current_table.is_some() {
                let events = self.parser.parse_line(line_content);
                for event in events {
                    self.handle_table_event(event);
                }
                continue;
            }

            // Peek at events to detect table start or horizontal rule
            let events = self.parser.parse_line(line_content);

            let is_table_start = events
                .iter()
                .any(|e| matches!(e, ParseEvent::TableHeader(_)));
            let is_rule = events
                .iter()
                .any(|e| matches!(e, ParseEvent::HorizontalRule));

            if is_table_start {
                // Start table mode
                for event in events {
                    self.handle_table_event(event);
                }
            } else if is_rule {
                self.blocks.push(Block::Divider);
            } else {
                // RAW PASS-THROUGH: Append the original line (preserves indentation!)
                self.append_text(&line, cx);
            }
        }

        cx.notify();
    }

    fn handle_table_event(&mut self, event: ParseEvent) {
        match event {
            ParseEvent::TableHeader(headers) => {
                self.current_table = Some(TableBuilder {
                    headers: headers.clone(),
                    rows: Vec::new(),
                });
            }
            ParseEvent::TableRow(row) => {
                if let Some(mut builder) = self.current_table.take() {
                    builder.rows.push(row);
                    self.current_table = Some(builder);
                }
            }
            ParseEvent::TableEnd => {
                if let Some(builder) = self.current_table.take() {
                    self.blocks.push(Block::Table(TableData {
                        headers: builder.headers,
                        rows: builder.rows,
                    }));
                }
            }
            _ => {} // Ignore all other events
        }
    }

    #[allow(unused)]
    fn append_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(Block::Text(current_text)) = self.blocks.last_mut() {
            current_text.push_str(text);

            // Update the last view
            if let Some(last_view) = self.text_views.last() {
                last_view.update(cx, |view, cx| {
                    view.append(text, cx);
                });
            } else {
                // Should exist if Block::Text exists, but just in case
                let theme = self.theme.clone();
                let text_owned = text.to_string();
                let view = cx.new(move |cx| SelectableText::new(cx, text_owned, theme));
                self.text_views.push(view);
            }
        } else {
            // New text block
            self.blocks.push(Block::Text(text.to_string()));
            let theme = self.theme.clone();
            let text_owned = text.to_string();
            let view = cx.new(move |cx| SelectableText::new(cx, text_owned, theme));
            self.text_views.push(view);
        }
    }
}

impl Render for StreamingMarkdownView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut col = div().flex().flex_col().w_full();

        // We need to match blocks with text_views
        // Text blocks map 1:1 to text_views

        let mut text_view_iter = self.text_views.iter();

        for block in &self.blocks {
            match block {
                Block::Text(_) => {
                    if let Some(view) = text_view_iter.next() {
                        col = col.child(div().child(view.clone()));
                    }
                }
                Block::Divider => {
                    col = col.child(div().w_full().h(px(1.)).bg(self.theme.border).my(px(8.)));
                }
                Block::Table(data) => {
                    let mut table_div = div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .border_1()
                        .border_color(self.theme.border);

                    // Header
                    if !data.headers.is_empty() {
                        let mut header_row = div()
                            .flex()
                            .flex_row()
                            .w_full()
                            .bg(self.theme.panel_background)
                            .border_b_1()
                            .border_color(self.theme.border);
                        for (i, header) in data.headers.iter().enumerate() {
                            let is_last = i == data.headers.len() - 1;
                            let mut cell = div()
                                .flex_1()
                                .min_w(px(0.))
                                .p_2()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(self.theme.text);
                            if !is_last {
                                cell = cell.border_r_1().border_color(self.theme.border);
                            }
                            header_row = header_row.child(cell.child(header.clone()));
                        }
                        table_div = table_div.child(header_row);
                    }

                    // Rows
                    for (row_idx, row) in data.rows.iter().enumerate() {
                        let is_even = row_idx % 2 == 0;

                        let mut row_div = div().flex().flex_row().w_full();
                        if is_even {
                            row_div = row_div.bg(self.theme.panel_background);
                        }

                        if row_idx < data.rows.len() - 1 {
                            row_div = row_div.border_b_1().border_color(self.theme.border);
                        }

                        for (i, cell_text) in row.iter().enumerate() {
                            let is_last = i == row.len() - 1;
                            let mut cell = div()
                                .flex_1()
                                .min_w(px(0.))
                                .p_2()
                                .text_color(self.theme.text);
                            if !is_last {
                                cell = cell.border_r_1().border_color(self.theme.border);
                            }
                            row_div = row_div.child(cell.child(cell_text.clone()));
                        }
                        table_div = table_div.child(row_div);
                    }

                    col = col.child(table_div.my(px(4.)));
                }
            }
        }

        col
    }
}
