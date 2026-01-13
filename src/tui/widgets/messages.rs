//! Message list widget with markdown rendering

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidget, Widget, Wrap},
};

use crate::tui::hit_test::{ClickTarget, HitTestRegistry};
use crate::tui::selection::SelectionState;
use crate::tui::state::{MessageRole, MessageSection, TuiMessage};
use crate::tui::theme::Theme;
use crate::tui::widgets::{NestedAgentWidget, ThinkingWidget, ToolCallWidget};

/// State for the message list
#[derive(Debug, Default)]
pub struct MessageListState {
    /// Current scroll offset (in lines)
    pub offset: usize,
    /// Total content height (in lines)
    pub content_height: usize,
    /// Viewport height
    pub viewport_height: usize,
}

impl MessageListState {
    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = (self.offset + amount).min(max_offset);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.content_height.saturating_sub(self.viewport_height);
    }
}

/// Widget for rendering the message list
pub struct MessageList<'a> {
    messages: &'a [TuiMessage],
    theme: &'a Theme,
    is_generating: bool,
    registry: Option<&'a mut HitTestRegistry>,
    selection: Option<&'a SelectionState>,
}

impl<'a> MessageList<'a> {
    pub fn new(messages: &'a [TuiMessage], theme: &'a Theme) -> Self {
        Self {
            messages,
            theme,
            is_generating: false,
            registry: None,
            selection: None,
        }
    }

    pub fn generating(mut self, is_generating: bool) -> Self {
        self.is_generating = is_generating;
        self
    }

    pub fn registry(mut self, registry: &'a mut HitTestRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    pub fn selection(mut self, selection: &'a SelectionState) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Apply selection highlighting to a line
    fn apply_selection(&self, line: Line<'static>, row: u16, start_col: u16) -> Line<'static> {
        let Some(selection) = self.selection else {
            return line;
        };

        if !selection.is_active() && selection.get_selection().is_none() {
            return line;
        }

        let mut new_spans = Vec::new();
        let mut current_col = start_col;

        for span in line.spans {
            let content = span.content;
            let style = span.style;

            let mut current_chunk = String::new();
            let mut chunk_start_col = current_col;
            let mut current_selected = selection.contains(row, current_col);

            for c in content.chars() {
                let is_selected = selection.contains(row, current_col);

                if is_selected != current_selected {
                    if !current_chunk.is_empty() {
                        let mut chunk_style = style;
                        if current_selected {
                            chunk_style = chunk_style.add_modifier(Modifier::REVERSED);
                        }
                        new_spans.push(Span::styled(current_chunk, chunk_style));
                        current_chunk = String::new();
                    }
                    current_selected = is_selected;
                }

                current_chunk.push(c);
                current_col += 1; // Assuming 1 char = 1 col width (mostly true)
            }

            if !current_chunk.is_empty() {
                let mut chunk_style = style;
                if current_selected {
                    chunk_style = chunk_style.add_modifier(Modifier::REVERSED);
                }
                new_spans.push(Span::styled(current_chunk, chunk_style));
            }
        }

        Line::from(new_spans)
    }
}

impl StatefulWidget for MessageList<'_> {
    type State = MessageListState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut virtual_y = 0usize;
        let width = area.width as usize;

        // Iterate to calculate layout and render visible items
        for msg in self.messages {
            // Render Role
            let height = 1;
            if virtual_y >= state.offset && (virtual_y - state.offset) < area.height as usize {
                let render_y = (area.y as usize + virtual_y - state.offset) as u16;
                let (role_text, role_style) = match msg.role {
                    MessageRole::User => (
                        "You",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    MessageRole::Assistant => (
                        "Assistant",
                        Style::default()
                            .fg(Color::Rgb(156, 220, 254))
                            .add_modifier(Modifier::BOLD),
                    ),
                    MessageRole::System => ("System", Style::default().fg(Color::DarkGray)),
                };

                // Construct line and apply selection
                let line = Line::from(Span::styled(role_text, role_style));
                let line = self.apply_selection(line, render_y, area.x);
                buf.set_line(area.x, render_y, &line, area.width);
            }
            virtual_y += height;

            // Render Sections
            if msg.sections.is_empty() {
                // Legacy content fallback
                let lines: Vec<std::borrow::Cow<'_, str>> = textwrap::wrap(&msg.content, width);
                for line in lines {
                    if virtual_y >= state.offset
                        && (virtual_y - state.offset) < area.height as usize
                    {
                        let render_y = (area.y as usize + virtual_y - state.offset) as u16;
                        let line = Line::from(line.to_string());
                        let line = self.apply_selection(line, render_y, area.x);
                        buf.set_line(area.x, render_y, &line, area.width);
                    }
                    virtual_y += 1;
                }
            } else {
                for section in &msg.sections {
                    match section {
                        MessageSection::Text(text) => {
                            let lines: Vec<std::borrow::Cow<'_, str>> = textwrap::wrap(text, width);
                            for line in lines {
                                if virtual_y >= state.offset
                                    && (virtual_y - state.offset) < area.height as usize
                                {
                                    let render_y =
                                        (area.y as usize + virtual_y - state.offset) as u16;
                                    let line = Line::from(line.to_string());
                                    let line = self.apply_selection(line, render_y, area.x);
                                    buf.set_line(area.x, render_y, &line, area.width);
                                }
                                virtual_y += 1;
                            }
                        }
                        MessageSection::ToolCall(tool) => {
                            let widget = ToolCallWidget::new(tool, self.theme);
                            let height = 1;
                            if virtual_y >= state.offset
                                && (virtual_y - state.offset) < area.height as usize
                            {
                                let render_y = (area.y as usize + virtual_y - state.offset) as u16;
                                widget.render(
                                    Rect::new(area.x, render_y, area.width, height as u16),
                                    buf,
                                );
                            }
                            virtual_y += height;
                        }
                        MessageSection::Thinking(thinking) => {
                            let widget = ThinkingWidget::new(thinking, self.theme);
                            let height = widget.height() as usize;
                            if virtual_y < state.offset + area.height as usize
                                && virtual_y + height > state.offset
                            {
                                if virtual_y >= state.offset {
                                    let render_y =
                                        (area.y as usize + virtual_y - state.offset) as u16;
                                    // Register toggle click target
                                    if let Some(registry) = &mut self.registry {
                                        registry.register(
                                            Rect::new(area.x, render_y, area.width, 1),
                                            ClickTarget::SectionToggle(thinking.id.clone()),
                                        );
                                    }
                                    widget.render(
                                        Rect::new(area.x, render_y, area.width, height as u16),
                                        buf,
                                    );
                                }
                            }
                            virtual_y += height;
                        }
                        MessageSection::NestedAgent(agent) => {
                            let widget = NestedAgentWidget::new(agent, self.theme);
                            let height = widget.height() as usize;
                            if virtual_y < state.offset + area.height as usize
                                && virtual_y + height > state.offset
                            {
                                if virtual_y >= state.offset {
                                    let render_y =
                                        (area.y as usize + virtual_y - state.offset) as u16;
                                    // Register toggle click target
                                    if let Some(registry) = &mut self.registry {
                                        registry.register(
                                            Rect::new(area.x, render_y, area.width, 1),
                                            ClickTarget::SectionToggle(agent.id.clone()),
                                        );
                                    }
                                    widget.render(
                                        Rect::new(area.x, render_y, area.width, height as u16),
                                        buf,
                                    );
                                }
                            }
                            virtual_y += height;
                        }
                    }
                }
            }

            // Spacer
            virtual_y += 1;
        }

        if self.is_generating {
            if virtual_y >= state.offset && (virtual_y - state.offset) < area.height as usize {
                let render_y = (area.y as usize + virtual_y - state.offset) as u16;
                buf.set_line(
                    area.x,
                    render_y,
                    &Line::from(Span::styled(
                        "● Generating...",
                        Style::default().fg(Color::Cyan),
                    )),
                    area.width,
                );
            }
            virtual_y += 1;
        }

        state.content_height = virtual_y;
        state.viewport_height = area.height as usize;
    }
}
