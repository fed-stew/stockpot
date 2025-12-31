//! Streaming markdown renderer for live terminal output.
//!
//! This renderer handles markdown formatting as text streams in,
//! applying styling in real-time while handling partial markdown constructs.

use crossterm::{
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
};
use std::io::{self, stdout, Write};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

/// Streaming markdown renderer for live terminal output.
pub struct StreamingMarkdownRenderer {
    // Code block state
    in_code_block: bool,
    code_block_lang: String,
    code_block_buffer: String,
    backtick_count: u8,

    // Inline formatting state
    in_inline_code: bool,
    pending_backtick: bool,

    // Bold/italic state
    asterisk_count: u8,
    in_bold: bool,
    in_italic: bool,

    // Line state
    line_buffer: String,
    at_line_start: bool,
    line_processed: bool,

    // Syntax highlighting
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Default for StreamingMarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingMarkdownRenderer {
    /// Create a new streaming markdown renderer.
    pub fn new() -> Self {
        Self {
            in_code_block: false,
            code_block_lang: String::new(),
            code_block_buffer: String::new(),
            backtick_count: 0,

            in_inline_code: false,
            pending_backtick: false,

            asterisk_count: 0,
            in_bold: false,
            in_italic: false,

            line_buffer: String::new(),
            at_line_start: true,
            line_processed: false,

            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Process incoming text delta.
    pub fn process(&mut self, text: &str) -> io::Result<()> {
        for ch in text.chars() {
            self.process_char(ch)?;
        }
        Ok(())
    }

    /// Process a single character.
    fn process_char(&mut self, ch: char) -> io::Result<()> {
        // If we're in a code block, handle specially
        if self.in_code_block {
            return self.process_code_block_char(ch);
        }

        // Check for code block start (```)
        if ch == '`' {
            self.backtick_count += 1;
            if self.backtick_count == 3 {
                // Starting a code block
                self.backtick_count = 0;
                self.in_code_block = true;
                self.code_block_lang.clear();
                self.code_block_buffer.clear();
                // Don't print the backticks
                return Ok(());
            }
            return Ok(()); // Wait for more backticks or other char
        } else if self.backtick_count > 0 {
            // We had backticks but not 3, so handle inline code
            if self.backtick_count == 1 {
                // Toggle inline code
                self.toggle_inline_code()?;
            } else {
                // Print the backticks we accumulated
                for _ in 0..self.backtick_count {
                    self.output_char('`')?;
                }
            }
            self.backtick_count = 0;
        }

        // Handle newlines
        if ch == '\n' {
            self.output_newline()?;
            return Ok(());
        }

        // At line start, check for special syntax
        if self.at_line_start {
            self.line_buffer.push(ch);

            // Check for headers
            if ch == '#' {
                return Ok(()); // Buffer until we see space or newline
            }

            // Check for list items
            if (ch == '-' || ch == '*' || ch == '+') && self.line_buffer.len() == 1 {
                return Ok(()); // Wait for space
            }

            // Check for numbered list
            if ch.is_ascii_digit() {
                return Ok(()); // Buffer digits
            }

            // Space after special char = process the line prefix
            if ch == ' ' {
                return self.process_line_prefix();
            }

            // Not a special line, process buffered content through markdown
            // (e.g., "*wags tail*" should render as italic, not show the asterisks)
            self.flush_line_buffer_markdown()?;
            self.at_line_start = false;
            return Ok(());
        }

        // Handle asterisks for bold/italic
        if ch == '*' {
            self.asterisk_count += 1;
            return Ok(()); // Wait to see if more asterisks come
        } else if self.asterisk_count > 0 {
            self.process_asterisks()?;
        }

        // Regular character - just output it
        self.output_char(ch)?;
        Ok(())
    }

    /// Process character while in a code block.
    fn process_code_block_char(&mut self, ch: char) -> io::Result<()> {
        // Check for closing ```
        if ch == '`' {
            self.backtick_count += 1;
            if self.backtick_count == 3 {
                // End of code block - render it
                self.backtick_count = 0;
                self.render_code_block()?;
                self.in_code_block = false;
                return Ok(());
            }
            return Ok(());
        } else if self.backtick_count > 0 {
            // Backticks in code block that aren't closing
            for _ in 0..self.backtick_count {
                self.code_block_buffer.push('`');
            }
            self.backtick_count = 0;
        }

        // First line is the language
        if self.code_block_buffer.is_empty() && ch != '\n' {
            self.code_block_lang.push(ch);
        } else if self.code_block_buffer.is_empty() && ch == '\n' {
            // Language line complete, now buffering code
            // Don't add the newline to buffer yet
        } else {
            self.code_block_buffer.push(ch);
        }
        Ok(())
    }

    /// Render a complete code block with syntax highlighting.
    fn render_code_block(&mut self) -> io::Result<()> {
        let mut stdout = stdout();

        // Remove trailing newline if present
        let code = self.code_block_buffer.trim_end();
        if code.is_empty() {
            return Ok(());
        }

        // Print code block header
        execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print("─".repeat(40)),
            Print(" "),
            SetForegroundColor(Color::Cyan),
            Print(&self.code_block_lang),
            ResetColor,
            Print("\n")
        )?;

        // Try to get syntax for the language
        let syntax = self
            .syntax_set
            .find_syntax_by_token(&self.code_block_lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(&self.code_block_lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        // Highlight and print each line
        for line in code.lines() {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            print!("  {}", escaped);
            println!("\x1b[0m"); // Reset at end of line
        }

        // Print code block footer
        execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print("─".repeat(40)),
            ResetColor,
            Print("\n")
        )?;

        stdout.flush()?;
        Ok(())
    }

    /// Toggle inline code formatting.
    fn toggle_inline_code(&mut self) -> io::Result<()> {
        let mut stdout = stdout();
        self.in_inline_code = !self.in_inline_code;

        if self.in_inline_code {
            execute!(stdout, SetForegroundColor(Color::Cyan))?;
        } else {
            execute!(stdout, ResetColor)?;
        }
        Ok(())
    }

    /// Process asterisks for bold/italic.
    fn process_asterisks(&mut self) -> io::Result<()> {
        let mut stdout = stdout();
        let count = self.asterisk_count;
        self.asterisk_count = 0;

        match count {
            1 => {
                // Toggle italic
                self.in_italic = !self.in_italic;
                if self.in_italic {
                    execute!(stdout, SetAttribute(Attribute::Italic))?;
                } else {
                    execute!(stdout, SetAttribute(Attribute::NoItalic))?;
                }
            }
            2 => {
                // Toggle bold
                self.in_bold = !self.in_bold;
                if self.in_bold {
                    execute!(stdout, SetAttribute(Attribute::Bold))?;
                } else {
                    execute!(stdout, SetAttribute(Attribute::NormalIntensity))?;
                }
            }
            3 => {
                // Bold + italic
                let active = self.in_bold && self.in_italic;
                self.in_bold = !active;
                self.in_italic = !active;
                if !active {
                    execute!(
                        stdout,
                        SetAttribute(Attribute::Bold),
                        SetAttribute(Attribute::Italic)
                    )?;
                } else {
                    execute!(
                        stdout,
                        SetAttribute(Attribute::NormalIntensity),
                        SetAttribute(Attribute::NoItalic)
                    )?;
                }
            }
            _ => {
                // Just print the asterisks
                for _ in 0..count {
                    self.output_char('*')?;
                }
            }
        }
        Ok(())
    }

    /// Process line prefix (header, list, etc.).
    fn process_line_prefix(&mut self) -> io::Result<()> {
        let mut stdout = stdout();
        let prefix = self.line_buffer.clone();
        self.line_buffer.clear();
        self.at_line_start = false;
        self.line_processed = true;

        // Count header level
        let hash_count = prefix.chars().take_while(|&c| c == '#').count();
        if hash_count > 0 && hash_count <= 6 {
            // It's a header
            execute!(
                stdout,
                SetForegroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold)
            )?;
            return Ok(());
        }

        // Check for list item
        let trimmed = prefix.trim();
        if trimmed == "-" || trimmed == "*" || trimmed == "+" {
            execute!(
                stdout,
                SetForegroundColor(Color::Yellow),
                Print("• "),
                ResetColor
            )?;
            return Ok(());
        }

        // Check for numbered list (e.g., "1. ")
        if let Some(rest) = trimmed.strip_suffix('.') {
            if rest.chars().all(|c| c.is_ascii_digit()) {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Yellow),
                    Print(&prefix),
                    ResetColor
                )?;
                return Ok(());
            }
        }

        // Not a special prefix, just print it
        print!("{}", prefix);
        stdout.flush()?;
        Ok(())
    }

    /// Flush buffered line content literally (no markdown processing).
    /// Used for newlines and end-of-stream where incomplete markdown should show as-is.
    fn flush_line_buffer_raw(&mut self) -> io::Result<()> {
        if !self.line_buffer.is_empty() {
            let buffer = std::mem::take(&mut self.line_buffer);
            for ch in buffer.chars() {
                self.output_char(ch)?;
            }
        }
        Ok(())
    }

    /// Flush buffered line content through markdown processing.
    /// Used when we determine the buffer isn't a special line prefix (header/list)
    /// but may contain inline markdown like *italic* or **bold**.
    fn flush_line_buffer_markdown(&mut self) -> io::Result<()> {
        if !self.line_buffer.is_empty() {
            let buffer = std::mem::take(&mut self.line_buffer);
            // Ensure we don't re-enter line-start handling
            self.at_line_start = false;

            for ch in buffer.chars() {
                // Handle backticks for inline code
                if ch == '`' {
                    self.backtick_count += 1;
                    continue;
                }
                if self.backtick_count > 0 {
                    if self.backtick_count == 1 {
                        self.toggle_inline_code()?;
                    } else {
                        // Multiple backticks that aren't 3 - output them
                        for _ in 0..self.backtick_count {
                            self.output_char('`')?;
                        }
                    }
                    self.backtick_count = 0;
                }

                // Handle asterisks for bold/italic
                if ch == '*' {
                    self.asterisk_count += 1;
                    continue;
                }
                if self.asterisk_count > 0 {
                    self.process_asterisks()?;
                }

                self.output_char(ch)?;
            }
        }
        Ok(())
    }

    /// Output a single character with current styling.
    fn output_char(&mut self, ch: char) -> io::Result<()> {
        print!("{}", ch);
        stdout().flush()?;
        Ok(())
    }

    /// Output a newline and reset line state.
    fn output_newline(&mut self) -> io::Result<()> {
        let mut stdout = stdout();

        // If we were in a header, reset styling
        if self.line_processed {
            execute!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;
            self.line_processed = false;
        }

        // Flush any buffered line content literally
        // (if we hit newline while still buffering, the prefix wasn't completed,
        // so output it as-is rather than processing as markdown)
        self.flush_line_buffer_raw()?;

        // Flush any pending backticks
        if self.backtick_count > 0 {
            for _ in 0..self.backtick_count {
                print!("`");
            }
            self.backtick_count = 0;
        }

        // Flush any pending asterisks
        if self.asterisk_count > 0 {
            for _ in 0..self.asterisk_count {
                print!("*");
            }
            self.asterisk_count = 0;
        }

        println!();
        self.at_line_start = true;
        // line_buffer already cleared by flush_line_buffer_raw
        stdout.flush()?;
        Ok(())
    }

    /// Flush any remaining content and reset state.
    pub fn flush(&mut self) -> io::Result<()> {
        let mut stdout = stdout();

        // Flush pending backticks
        if self.backtick_count > 0 {
            for _ in 0..self.backtick_count {
                print!("`");
            }
            self.backtick_count = 0;
        }

        // Flush pending asterisks
        if self.asterisk_count > 0 {
            for _ in 0..self.asterisk_count {
                print!("*");
            }
            self.asterisk_count = 0;
        }

        // Flush line buffer literally (incomplete markdown at end of stream)
        self.flush_line_buffer_raw()?;

        // If we're still in a code block, render what we have
        if self.in_code_block && !self.code_block_buffer.is_empty() {
            self.render_code_block()?;
            self.in_code_block = false;
        }

        // Reset all styling
        execute!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;

        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_renderer() {
        let renderer = StreamingMarkdownRenderer::new();
        assert!(!renderer.in_code_block);
        assert!(!renderer.in_inline_code);
        assert!(renderer.at_line_start);
    }

    #[test]
    fn test_italic_at_line_start() {
        // Test that *italic* at line start triggers italic mode, not raw asterisk output
        let mut renderer = StreamingMarkdownRenderer::new();
        
        // Process "*wags tail" - the asterisk should trigger italic mode
        renderer.process("*wags").unwrap();
        
        // After processing "*w", italic should be ON (asterisk was processed as markdown)
        assert!(renderer.in_italic, "italic mode should be enabled after *word at line start");
        assert!(!renderer.at_line_start, "should no longer be at line start");
    }

    #[test]
    fn test_bold_at_line_start() {
        // Test that **bold** at line start triggers bold mode
        let mut renderer = StreamingMarkdownRenderer::new();
        
        // Process "**bold" - the double asterisk should trigger bold mode
        renderer.process("**bold").unwrap();
        
        // After processing "**b", bold should be ON
        assert!(renderer.in_bold, "bold mode should be enabled after **word at line start");
    }

    #[test]
    fn test_list_marker_not_confused_with_italic() {
        // Test that "* item" is treated as a list, not italic
        let mut renderer = StreamingMarkdownRenderer::new();
        
        // Process "* " - this should be a list marker
        renderer.process("* ").unwrap();
        
        // Should NOT be in italic mode (it's a list)
        assert!(!renderer.in_italic, "list marker should not trigger italic");
    }

    #[test]
    fn test_asterisk_followed_by_newline() {
        // Test that "*\n" outputs the asterisk literally (not italic)
        let mut renderer = StreamingMarkdownRenderer::new();
        
        renderer.process("*\n").unwrap();
        
        // After newline, should be back at line start
        assert!(renderer.at_line_start, "should be at line start after newline");
        // Should NOT be in italic mode (lone asterisk followed by newline)
        assert!(!renderer.in_italic, "lone asterisk before newline should not trigger italic");
    }
}
