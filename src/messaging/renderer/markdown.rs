//! Markdown rendering for terminal output.
//!
//! Handles markdown parsing and rendering including:
//! - Headers, lists, blockquotes
//! - Inline formatting (bold, italic, code, links)
//! - Code blocks with syntax highlighting

use super::TerminalRenderer;
use crossterm::{
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    ExecutableCommand,
};
use std::io::{stdout, Write};
use syntect::easy::HighlightLines;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

impl TerminalRenderer {
    /// Render markdown content with proper formatting.
    pub fn render_markdown(&self, content: &str) -> std::io::Result<()> {
        let mut in_code_block = false;
        let mut code_lang = String::new();
        let mut code_buffer = String::new();

        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("```") {
                if in_code_block {
                    // End of code block - render it
                    self.render_code_block(&code_lang, &code_buffer)?;
                    code_buffer.clear();
                    code_lang.clear();
                    in_code_block = false;
                } else {
                    // Start of code block
                    in_code_block = true;
                    code_lang = rest.trim().to_string();
                }
            } else if in_code_block {
                code_buffer.push_str(line);
                code_buffer.push('\n');
            } else {
                self.render_markdown_line(line)?;
            }
        }

        // Handle unclosed code block
        if in_code_block && !code_buffer.is_empty() {
            self.render_code_block(&code_lang, &code_buffer)?;
        }

        Ok(())
    }

    /// Render a single line of markdown.
    pub(super) fn render_markdown_line(&self, line: &str) -> std::io::Result<()> {
        let mut stdout = stdout();

        // Headers
        if let Some(rest) = line.strip_prefix("### ") {
            stdout
                .execute(SetForegroundColor(Color::Cyan))?
                .execute(SetAttribute(Attribute::Bold))?
                .execute(Print(rest))?
                .execute(SetAttribute(Attribute::Reset))?
                .execute(Print("\n"))?;
            return Ok(());
        }
        if let Some(rest) = line.strip_prefix("## ") {
            stdout
                .execute(SetForegroundColor(Color::Cyan))?
                .execute(SetAttribute(Attribute::Bold))?
                .execute(Print(rest))?
                .execute(SetAttribute(Attribute::Reset))?
                .execute(Print("\n"))?;
            return Ok(());
        }
        if let Some(rest) = line.strip_prefix("# ") {
            stdout
                .execute(SetForegroundColor(Color::Cyan))?
                .execute(SetAttribute(Attribute::Bold))?
                .execute(Print(rest))?
                .execute(SetAttribute(Attribute::Reset))?
                .execute(Print("\n"))?;
            return Ok(());
        }

        // Bullet lists
        if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            stdout
                .execute(SetForegroundColor(Color::Yellow))?
                .execute(Print("• "))?
                .execute(ResetColor)?;
            self.render_inline_markdown(rest)?;
            stdout.execute(Print("\n"))?;
            return Ok(());
        }

        // Numbered lists
        if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit()) {
            if let Some(rest) = rest.strip_prefix(". ") {
                stdout.execute(SetForegroundColor(Color::Yellow))?;
                // Get the number
                let num = line
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>();
                stdout
                    .execute(Print(format!("{}. ", num)))?
                    .execute(ResetColor)?;
                self.render_inline_markdown(rest)?;
                stdout.execute(Print("\n"))?;
                return Ok(());
            }
        }

        // Blockquotes
        if let Some(rest) = line.strip_prefix("> ") {
            stdout
                .execute(SetForegroundColor(Color::DarkGrey))?
                .execute(Print("│ "))?
                .execute(ResetColor)?;
            self.render_inline_markdown(rest)?;
            stdout.execute(Print("\n"))?;
            return Ok(());
        }

        // Horizontal rule
        if line == "---" || line == "***" || line == "___" {
            stdout
                .execute(SetForegroundColor(Color::DarkGrey))?
                .execute(Print("─".repeat(40)))?
                .execute(ResetColor)?
                .execute(Print("\n"))?;
            return Ok(());
        }

        // Regular line - render inline markdown
        self.render_inline_markdown(line)?;
        stdout.execute(Print("\n"))?;

        Ok(())
    }

    /// Render inline markdown (bold, italic, code, links).
    pub(super) fn render_inline_markdown(&self, text: &str) -> std::io::Result<()> {
        let mut stdout = stdout();
        let mut chars = text.chars().peekable();
        let mut buffer = String::new();

        while let Some(c) = chars.next() {
            match c {
                '`' => {
                    // Flush buffer
                    if !buffer.is_empty() {
                        stdout.execute(Print(&buffer))?;
                        buffer.clear();
                    }
                    // Inline code
                    let mut code = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc == '`' {
                            chars.next();
                            break;
                        }
                        code.push(chars.next().unwrap());
                    }
                    stdout
                        .execute(SetForegroundColor(Color::Magenta))?
                        .execute(Print(&code))?
                        .execute(ResetColor)?;
                }
                '*' | '_' => {
                    // Check for bold (**) or italic (*)
                    if chars.peek() == Some(&c) {
                        chars.next();
                        // Flush buffer
                        if !buffer.is_empty() {
                            stdout.execute(Print(&buffer))?;
                            buffer.clear();
                        }
                        // Bold
                        let mut bold_text = String::new();
                        while let Some(nc) = chars.next() {
                            if nc == c && chars.peek() == Some(&c) {
                                chars.next();
                                break;
                            }
                            bold_text.push(nc);
                        }
                        stdout
                            .execute(SetAttribute(Attribute::Bold))?
                            .execute(Print(&bold_text))?
                            .execute(SetAttribute(Attribute::Reset))?;
                    } else {
                        // Flush buffer
                        if !buffer.is_empty() {
                            stdout.execute(Print(&buffer))?;
                            buffer.clear();
                        }
                        // Italic
                        let mut italic_text = String::new();
                        for nc in chars.by_ref() {
                            if nc == c {
                                break;
                            }
                            italic_text.push(nc);
                        }
                        stdout
                            .execute(SetAttribute(Attribute::Italic))?
                            .execute(Print(&italic_text))?
                            .execute(SetAttribute(Attribute::Reset))?;
                    }
                }
                '[' => {
                    // Link: [text](url)
                    let mut link_text = String::new();
                    let mut found_close = false;
                    for nc in chars.by_ref() {
                        if nc == ']' {
                            found_close = true;
                            break;
                        }
                        link_text.push(nc);
                    }
                    if found_close && chars.peek() == Some(&'(') {
                        chars.next();
                        let mut url = String::new();
                        for nc in chars.by_ref() {
                            if nc == ')' {
                                break;
                            }
                            url.push(nc);
                        }
                        // Flush buffer
                        if !buffer.is_empty() {
                            stdout.execute(Print(&buffer))?;
                            buffer.clear();
                        }
                        // url is captured but not displayed (just the link text)
                        let _ = url;
                        stdout
                            .execute(SetForegroundColor(Color::Blue))?
                            .execute(SetAttribute(Attribute::Underlined))?
                            .execute(Print(&link_text))?
                            .execute(SetAttribute(Attribute::Reset))?
                            .execute(ResetColor)?;
                    } else {
                        buffer.push('[');
                        buffer.push_str(&link_text);
                        if found_close {
                            buffer.push(']');
                        }
                    }
                }
                _ => buffer.push(c),
            }
        }

        // Flush remaining buffer
        if !buffer.is_empty() {
            stdout.execute(Print(&buffer))?;
        }

        Ok(())
    }

    /// Render a code block with syntax highlighting.
    pub(super) fn render_code_block(&self, lang: &str, code: &str) -> std::io::Result<()> {
        let mut stdout = stdout();

        // Find syntax for the language
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        // Print code block header
        stdout
            .execute(SetForegroundColor(Color::DarkGrey))?
            .execute(Print(format!(
                "┌── {}\n",
                if lang.is_empty() { "code" } else { lang }
            )))?
            .execute(ResetColor)?;

        // Highlight and print each line
        for line in LinesWithEndings::from(code) {
            stdout
                .execute(SetForegroundColor(Color::DarkGrey))?
                .execute(Print("│ "))?
                .execute(ResetColor)?;

            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    print!("{}", escaped);
                }
                Err(_) => {
                    print!("{}", line);
                }
            }
        }

        // Print code block footer
        stdout
            .execute(SetForegroundColor(Color::DarkGrey))?
            .execute(Print("└──\n"))?
            .execute(ResetColor)?;

        Ok(())
    }
}
