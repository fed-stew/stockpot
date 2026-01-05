//! Terminal renderer for messages with rich markdown support.
//!
//! This module provides a [`TerminalRenderer`] that can render various message
//! types to the terminal with colors, formatting, and syntax highlighting.
//!
//! ## Module Structure
//!
//! - `style` - Render style configuration (colors)
//! - `markdown` - Markdown parsing and rendering
//! - `messages` - Tool, agent, and event rendering
//! - `ui` - UI elements (spinners, prompts, dividers)

mod markdown;
mod messages;
mod style;
mod ui;

pub use style::RenderStyle;

use super::Message;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Terminal renderer for messages.
pub struct TerminalRenderer {
    pub(super) style: RenderStyle,
    pub(super) syntax_set: SyntaxSet,
    pub(super) theme_set: ThemeSet,
}

impl TerminalRenderer {
    /// Create a new renderer with default style.
    pub fn new() -> Self {
        Self {
            style: RenderStyle::default(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Create a renderer with custom style.
    pub fn with_style(style: RenderStyle) -> Self {
        Self {
            style,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Render a message to the terminal.
    ///
    /// Dispatches to the appropriate rendering method based on message type.
    pub fn render(&self, message: &Message) -> std::io::Result<()> {
        match message {
            Message::Text(text) => self.render_text(text.level, &text.text),
            Message::Reasoning(r) => self.render_reasoning(&r.reasoning, r.next_steps.as_deref()),
            Message::Response(r) => self.render_response(&r.content),
            Message::Shell(s) => self.render_shell(&s.command, s.output.as_deref(), s.exit_code),
            Message::File(f) => self.render_file(&f.operation, &f.path, f.content.as_deref()),
            Message::Diff(d) => self.render_diff(&d.path, &d.lines),
            Message::Spinner(s) => self.render_spinner(&s.text, s.is_active),
            Message::InputRequest(r) => self.render_input_request(&r.prompt),
            Message::Divider => self.render_divider(),
            Message::Clear => self.clear_screen(),
            Message::Agent(agent_msg) => self.render_agent_event(agent_msg),
            Message::Tool(tool_msg) => self.render_tool_event(tool_msg),
            Message::TextDelta(delta) => self.render_text_delta(delta),
            Message::Thinking(thinking) => self.render_thinking(&thinking.text),
        }
    }

    /// Run a render loop consuming messages from a receiver.
    ///
    /// This is designed to be spawned as a task that renders all messages
    /// as they arrive from the bus. Text deltas are processed through a
    /// streaming markdown renderer for smooth output.
    pub async fn run_loop(&self, mut receiver: crate::messaging::MessageReceiver) {
        use crate::cli::streaming_markdown::StreamingMarkdownRenderer;

        let mut md_renderer = StreamingMarkdownRenderer::new();
        let mut in_text_stream = false;

        while let Ok(message) = receiver.recv().await {
            // Handle text deltas specially for markdown rendering
            match &message {
                Message::TextDelta(delta) => {
                    in_text_stream = true;
                    if md_renderer.process(&delta.text).is_err() {
                        // Fallback to basic rendering
                        let _ = self.render(&message);
                    }
                }
                _ => {
                    // Flush markdown before non-text messages
                    if in_text_stream {
                        let _ = md_renderer.flush();
                        in_text_stream = false;
                    }
                    let _ = self.render(&message);
                }
            }
        }

        // Final flush
        if in_text_stream {
            let _ = md_renderer.flush();
        }
    }
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::Color;

    #[test]
    fn test_renderer_creation() {
        let renderer = TerminalRenderer::new();
        assert_eq!(renderer.style.success_color, Color::Green);
    }

    #[test]
    fn test_custom_style() {
        let style = RenderStyle {
            success_color: Color::Blue,
            ..Default::default()
        };
        let renderer = TerminalRenderer::with_style(style);
        assert_eq!(renderer.style.success_color, Color::Blue);
    }
}
