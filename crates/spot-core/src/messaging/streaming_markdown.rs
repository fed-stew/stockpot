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

    /// Get the current terminal width.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the current buffer contents (for testing).
    pub fn buffer(&self) -> &str {
        &self.line_buffer
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

    // =========================================================================
    // Constructor Tests
    // =========================================================================

    #[test]
    fn test_new_renderer() {
        let renderer = StreamingMarkdownRenderer::new();
        assert!(renderer.line_buffer.is_empty());
        assert!(renderer.width > 0); // Should have some reasonable default
    }

    #[test]
    fn test_with_width() {
        let renderer = StreamingMarkdownRenderer::with_width(120);
        assert_eq!(renderer.width, 120);
        assert!(renderer.line_buffer.is_empty());
    }

    #[test]
    fn test_with_width_small() {
        let renderer = StreamingMarkdownRenderer::with_width(40);
        assert_eq!(renderer.width, 40);
    }

    #[test]
    fn test_with_width_large() {
        let renderer = StreamingMarkdownRenderer::with_width(200);
        assert_eq!(renderer.width, 200);
    }

    #[test]
    fn test_default_trait() {
        let renderer = StreamingMarkdownRenderer::default();
        assert!(renderer.line_buffer.is_empty());
    }

    // =========================================================================
    // Accessor Tests
    // =========================================================================

    #[test]
    fn test_width_accessor() {
        let renderer = StreamingMarkdownRenderer::with_width(100);
        assert_eq!(renderer.width(), 100);
    }

    #[test]
    fn test_buffer_accessor_empty() {
        let renderer = StreamingMarkdownRenderer::new();
        assert_eq!(renderer.buffer(), "");
    }

    // =========================================================================
    // Reset Tests
    // =========================================================================

    #[test]
    fn test_reset_clears_buffer() {
        let mut renderer = StreamingMarkdownRenderer::new();
        renderer.line_buffer = "some content".to_string();
        renderer.reset();
        assert!(renderer.line_buffer.is_empty());
    }

    #[test]
    fn test_reset_idempotent() {
        let mut renderer = StreamingMarkdownRenderer::new();
        renderer.reset();
        renderer.reset();
        renderer.reset();
        assert!(renderer.line_buffer.is_empty());
    }

    // =========================================================================
    // Process Method Tests
    // =========================================================================

    #[test]
    fn test_process_accumulates_without_newline() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("hello").unwrap();
        // Without newline, content stays in buffer
        assert_eq!(renderer.buffer(), "hello");
    }

    #[test]
    fn test_process_multiple_deltas() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("hel").unwrap();
        renderer.process("lo ").unwrap();
        renderer.process("world").unwrap();
        assert_eq!(renderer.buffer(), "hello world");
    }

    #[test]
    fn test_process_empty_string_no_change() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("content").unwrap();
        renderer.process("").unwrap();
        assert_eq!(renderer.buffer(), "content");
    }

    #[test]
    fn test_process_only_newlines() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("\n\n\n").unwrap();
        // All newlines processed, buffer should be empty
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_process_newline_flushes_line() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("line one\n").unwrap();
        // After newline, that line is rendered, buffer should be empty
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_process_partial_after_newline() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("line one\npartial").unwrap();
        // "line one" rendered, "partial" stays in buffer
        assert_eq!(renderer.buffer(), "partial");
    }

    #[test]
    fn test_process_multiple_lines() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("line1\nline2\nline3\n").unwrap();
        assert!(renderer.buffer().is_empty());
    }

    // =========================================================================
    // Buffer Accessor Tests
    // =========================================================================

    #[test]
    fn test_buffer_accessor_after_process() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("test content").unwrap();
        assert_eq!(renderer.buffer(), "test content");
    }

    #[test]
    fn test_buffer_partial_line_preserved() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("complete\nincomplete").unwrap();
        // "incomplete" should remain in buffer
        assert_eq!(renderer.buffer(), "incomplete");
    }

    #[test]
    fn test_buffer_after_multiple_newlines() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("a\nb\nc\npartial").unwrap();
        assert_eq!(renderer.buffer(), "partial");
    }

    // =========================================================================
    // Reset Method Tests
    // =========================================================================

    #[test]
    fn test_reset_clears_buffer_with_content() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("some content here").unwrap();
        assert!(!renderer.buffer().is_empty());
        renderer.reset();
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_reset_after_process() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("first").unwrap();
        renderer.process(" second").unwrap();
        renderer.reset();
        assert!(renderer.buffer().is_empty());
        // Can process again after reset
        renderer.process("new content").unwrap();
        assert_eq!(renderer.buffer(), "new content");
    }

    #[test]
    fn test_reset_after_partial_line() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("line\npartial").unwrap();
        renderer.reset();
        assert!(renderer.buffer().is_empty());
    }

    // =========================================================================
    // Flush Method Tests
    // =========================================================================

    #[test]
    fn test_flush_clears_buffer() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("unflushed content").unwrap();
        assert!(!renderer.buffer().is_empty());
        renderer.flush().unwrap();
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_flush_resets_parser_state() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        // Process some markdown that might leave parser in a state
        renderer.process("```rust\nlet x = 1;").unwrap();
        renderer.flush().unwrap();
        // After flush, should be able to process fresh content
        renderer.process("new content\n").unwrap();
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_flush_empty_buffer() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        // Flushing empty buffer should not panic
        renderer.flush().unwrap();
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_flush_after_complete_line() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("complete line\n").unwrap();
        // Buffer already empty from newline
        renderer.flush().unwrap();
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_flush_multiple_times() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("content").unwrap();
        renderer.flush().unwrap();
        renderer.flush().unwrap();
        renderer.flush().unwrap();
        assert!(renderer.buffer().is_empty());
    }

    #[test]
    fn test_process_after_flush() {
        let mut renderer = StreamingMarkdownRenderer::with_width(80);
        renderer.process("first batch").unwrap();
        renderer.flush().unwrap();
        renderer.process("second batch").unwrap();
        assert_eq!(renderer.buffer(), "second batch");
    }
}
