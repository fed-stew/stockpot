//! Streaming markdown renderer using streamdown-rs.
//!
//! This module wraps the streamdown-rs library to provide real-time
//! markdown rendering for streaming LLM output.

use std::io::{self, stdout, Write};
use streamdown_parser::Parser;
use streamdown_render::Renderer;

/// Streaming markdown renderer for live terminal output.
pub struct StreamingMarkdownRenderer {
    /// Parser for markdown content
    parser: Parser,
    /// Buffer for accumulating partial lines
    line_buffer: String,
    /// Terminal width
    width: usize,
}

impl Default for StreamingMarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingMarkdownRenderer {
    /// Create a new streaming markdown renderer.
    pub fn new() -> Self {
        let width = terminal_size::terminal_size()
            .map(|(w, _)| w.0 as usize)
            .unwrap_or(80);

        Self {
            parser: Parser::new(),
            line_buffer: String::new(),
            width,
        }
    }

    /// Create with a specific width.
    pub fn with_width(width: usize) -> Self {
        Self {
            parser: Parser::new(),
            line_buffer: String::new(),
            width,
        }
    }

    /// Process incoming text delta.
    ///
    /// Buffers text until complete lines are available, then parses
    /// and renders them.
    pub fn process(&mut self, text: &str) -> io::Result<()> {
        self.line_buffer.push_str(text);
        self.render_complete_lines()
    }

    /// Render any complete lines in the buffer.
    fn render_complete_lines(&mut self) -> io::Result<()> {
        let mut stdout = stdout();

        // Process complete lines (ending with \n)
        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..newline_pos].to_string();
            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();

            // Parse the line into events
            let events = self.parser.parse_line(&line);

            // Render each event
            let mut renderer = Renderer::new(&mut stdout, self.width);
            for event in &events {
                renderer.render_event(event)?;
            }
        }

        stdout.flush()?;
        Ok(())
    }

    /// Flush any remaining content and reset state.
    pub fn flush(&mut self) -> io::Result<()> {
        let mut stdout = stdout();

        // If there's remaining content without a trailing newline, render it
        if !self.line_buffer.is_empty() {
            let line = std::mem::take(&mut self.line_buffer);
            let events = self.parser.parse_line(&line);

            let mut renderer = Renderer::new(&mut stdout, self.width);
            for event in &events {
                renderer.render_event(event)?;
            }
        }

        // Finalize the parser (closes any open blocks)
        let final_events = self.parser.finalize();
        if !final_events.is_empty() {
            let mut renderer = Renderer::new(&mut stdout, self.width);
            for event in &final_events {
                renderer.render_event(event)?;
            }
        }

        stdout.flush()?;

        // Reset for next use
        self.parser.reset();

        Ok(())
    }

    /// Reset the renderer state without flushing.
    pub fn reset(&mut self) {
        self.line_buffer.clear();
        self.parser.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_renderer() {
        let renderer = StreamingMarkdownRenderer::new();
        assert!(renderer.line_buffer.is_empty());
    }

    #[test]
    fn test_with_width() {
        let renderer = StreamingMarkdownRenderer::with_width(120);
        assert_eq!(renderer.width, 120);
    }
}
