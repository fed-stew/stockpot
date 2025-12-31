//! Terminal renderer for messages with rich markdown support.

use super::{
    AgentEvent, AgentMessage, DiffLineType, FileOperation, Message, MessageLevel,
    TextDeltaMessage, ToolMessage, ToolStatus,
};
use crossterm::{
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    ExecutableCommand,
};
use std::io::{stdout, Write};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

/// Render style configuration.
#[derive(Debug, Clone)]
pub struct RenderStyle {
    pub info_color: Color,
    pub success_color: Color,
    pub warning_color: Color,
    pub error_color: Color,
    pub code_color: Color,
    pub diff_add_color: Color,
    pub diff_remove_color: Color,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            info_color: Color::White,
            success_color: Color::Green,
            warning_color: Color::Yellow,
            error_color: Color::Red,
            code_color: Color::Cyan,
            diff_add_color: Color::Green,
            diff_remove_color: Color::Red,
        }
    }
}

/// Terminal renderer for messages.
pub struct TerminalRenderer {
    style: RenderStyle,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl TerminalRenderer {
    /// Create a new renderer.
    pub fn new() -> Self {
        Self {
            style: RenderStyle::default(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Create with custom style.
    pub fn with_style(style: RenderStyle) -> Self {
        Self {
            style,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Render a message to the terminal.
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

    fn render_text(&self, level: MessageLevel, text: &str) -> std::io::Result<()> {
        let color = match level {
            MessageLevel::Info => self.style.info_color,
            MessageLevel::Success => self.style.success_color,
            MessageLevel::Warning => self.style.warning_color,
            MessageLevel::Error => self.style.error_color,
            MessageLevel::Debug => Color::DarkGrey,
        };

        let prefix = match level {
            MessageLevel::Success => "✓ ",
            MessageLevel::Warning => "⚠ ",
            MessageLevel::Error => "✗ ",
            _ => "",
        };

        stdout()
            .execute(SetForegroundColor(color))?
            .execute(Print(prefix))?
            .execute(Print(text))?
            .execute(Print("\n"))?
            .execute(ResetColor)?;

        Ok(())
    }

    fn render_reasoning(&self, reasoning: &str, next_steps: Option<&str>) -> std::io::Result<()> {
        stdout()
            .execute(SetForegroundColor(Color::DarkCyan))?
            .execute(Print("💭 Reasoning:\n"))?
            .execute(ResetColor)?
            .execute(Print(reasoning))?
            .execute(Print("\n"))?;

        if let Some(steps) = next_steps {
            stdout()
                .execute(SetForegroundColor(Color::DarkCyan))?
                .execute(Print("\n📋 Next Steps:\n"))?
                .execute(ResetColor)?
                .execute(Print(steps))?
                .execute(Print("\n"))?;
        }

        Ok(())
    }

    /// Render markdown response with syntax highlighting.
    fn render_response(&self, content: &str) -> std::io::Result<()> {
        self.render_markdown(content)
    }

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
    fn render_markdown_line(&self, line: &str) -> std::io::Result<()> {
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
                let num = line.chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                stdout.execute(Print(format!("{}. ", num)))?
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

    /// Render inline markdown (bold, italic, code).
    fn render_inline_markdown(&self, text: &str) -> std::io::Result<()> {
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
    fn render_code_block(&self, lang: &str, code: &str) -> std::io::Result<()> {
        let mut stdout = stdout();
        
        // Find syntax for the language
        let syntax = self.syntax_set
            .find_syntax_by_token(lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        // Print code block header
        stdout
            .execute(SetForegroundColor(Color::DarkGrey))?
            .execute(Print(format!("┌── {}\n", if lang.is_empty() { "code" } else { lang })))?
            .execute(ResetColor)?;
        
        // Highlight and print each line
        for line in LinesWithEndings::from(code) {
            stdout.execute(SetForegroundColor(Color::DarkGrey))?
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

    fn render_shell(&self, command: &str, output: Option<&str>, exit_code: Option<i32>) -> std::io::Result<()> {
        stdout()
            .execute(SetForegroundColor(Color::DarkYellow))?
            .execute(Print("$ "))?
            .execute(ResetColor)?
            .execute(Print(command))?
            .execute(Print("\n"))?;

        if let Some(out) = output {
            println!("{}", out);
        }

        if let Some(code) = exit_code {
            if code != 0 {
                stdout()
                    .execute(SetForegroundColor(Color::Red))?
                    .execute(Print(format!("Exit code: {}\n", code)))?
                    .execute(ResetColor)?;
            }
        }

        Ok(())
    }

    fn render_file(&self, op: &FileOperation, path: &str, _content: Option<&str>) -> std::io::Result<()> {
        let (icon, verb) = match op {
            FileOperation::Read => ("📖", "Read"),
            FileOperation::Write => ("✏️", "Wrote"),
            FileOperation::List => ("📁", "Listed"),
            FileOperation::Grep => ("🔍", "Searched"),
            FileOperation::Delete => ("🗑️", "Deleted"),
        };

        stdout()
            .execute(SetForegroundColor(Color::Cyan))?
            .execute(Print(format!("{} {} {}\n", icon, verb, path)))?
            .execute(ResetColor)?;

        Ok(())
    }

    fn render_diff(&self, path: &str, lines: &[super::DiffLine]) -> std::io::Result<()> {
        stdout()
            .execute(SetForegroundColor(Color::Cyan))?
            .execute(Print(format!("📝 {}\n", path)))?
            .execute(ResetColor)?;

        for line in lines {
            let color = match line.line_type {
                DiffLineType::Added => self.style.diff_add_color,
                DiffLineType::Removed => self.style.diff_remove_color,
                DiffLineType::Header => Color::Cyan,
                DiffLineType::Context => Color::White,
            };

            stdout()
                .execute(SetForegroundColor(color))?
                .execute(Print(&line.content))?
                .execute(Print("\n"))?
                .execute(ResetColor)?;
        }

        Ok(())
    }

    fn render_spinner(&self, text: &str, is_active: bool) -> std::io::Result<()> {
        let icon = if is_active { "⏳" } else { "✓" };
        stdout()
            .execute(SetForegroundColor(Color::Cyan))?
            .execute(Print(format!("{} {}\n", icon, text)))?
            .execute(ResetColor)?;
        Ok(())
    }

    fn render_input_request(&self, prompt: &str) -> std::io::Result<()> {
        stdout()
            .execute(SetForegroundColor(Color::Yellow))?
            .execute(Print(format!("❓ {}\n", prompt)))?
            .execute(ResetColor)?;
        Ok(())
    }

    fn render_divider(&self) -> std::io::Result<()> {
        println!("───────────────────────────────────────────────────────────────────────────────");
        Ok(())
    }

    fn clear_screen(&self) -> std::io::Result<()> {
        use crossterm::terminal::{Clear, ClearType};
        stdout().execute(Clear(ClearType::All))?;
        Ok(())
    }

    /// Render agent lifecycle events (start/complete/error).
    fn render_agent_event(&self, msg: &AgentMessage) -> std::io::Result<()> {
        match &msg.event {
            AgentEvent::Started => {
                // Print agent header with display name
                println!();
                stdout()
                    .execute(SetForegroundColor(Color::Magenta))?
                    .execute(SetAttribute(Attribute::Bold))?
                    .execute(Print(&msg.display_name))?
                    .execute(Print(":"))?
                    .execute(ResetColor)?
                    .execute(SetAttribute(Attribute::Reset))?;
                println!();
                println!();
            }
            AgentEvent::Completed { run_id: _ } => {
                // Just add spacing after agent output
                println!();
            }
            AgentEvent::Error { message } => {
                stdout()
                    .execute(SetForegroundColor(Color::Red))?
                    .execute(SetAttribute(Attribute::Bold))?
                    .execute(Print("❌ Agent error: "))?
                    .execute(ResetColor)?
                    .execute(SetAttribute(Attribute::Reset))?
                    .execute(Print(message))?;
                println!();
            }
        }
        Ok(())
    }

    /// Render tool execution events.
    fn render_tool_event(&self, msg: &ToolMessage) -> std::io::Result<()> {
        match msg.status {
            ToolStatus::Started => {
                // Print tool name in dim
                println!();
                stdout()
                    .execute(SetAttribute(Attribute::Dim))?
                    .execute(Print(&msg.tool_name))?
                    .execute(SetAttribute(Attribute::Reset))?
                    .execute(Print(" "))?;
                stdout().flush()?;
            }
            ToolStatus::ArgsStreaming => {
                // Args streaming - could show dots or nothing
            }
            ToolStatus::Executing => {
                // Show parsed args nicely formatted
                if let Some(ref args) = msg.args {
                    self.render_tool_args(&msg.tool_name, args)?;
                }
                stdout().flush()?;
            }
            ToolStatus::Completed => {
                // Just end the line
                println!();
            }
            ToolStatus::Failed => {
                // Show error
                if let Some(ref err) = msg.error {
                    // Truncate long errors
                    let display_err = if err.len() > 60 {
                        format!("{}...", &err[..57])
                    } else {
                        err.clone()
                    };
                    stdout()
                        .execute(Print(" "))?
                        .execute(SetForegroundColor(Color::Red))?
                        .execute(SetAttribute(Attribute::Bold))?
                        .execute(Print("✗ "))?
                        .execute(Print(display_err))?
                        .execute(ResetColor)?
                        .execute(SetAttribute(Attribute::Reset))?;
                } else {
                    stdout()
                        .execute(Print(" "))?
                        .execute(SetForegroundColor(Color::Red))?
                        .execute(SetAttribute(Attribute::Bold))?
                        .execute(Print("✗ failed"))?
                        .execute(ResetColor)?
                        .execute(SetAttribute(Attribute::Reset))?;
                }
                println!();
            }
        }
        Ok(())
    }

    /// Render tool arguments in a nice format based on tool type.
    fn render_tool_args(&self, tool_name: &str, args: &serde_json::Value) -> std::io::Result<()> {
        match tool_name {
            "read_file" => {
                if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                    stdout()
                        .execute(SetForegroundColor(Color::Cyan))?
                        .execute(Print(path))?
                        .execute(ResetColor)?;
                }
            }
            "list_files" => {
                if let Some(dir) = args.get("directory").and_then(|v| v.as_str()) {
                    stdout()
                        .execute(SetForegroundColor(Color::Cyan))?
                        .execute(Print(dir))?
                        .execute(ResetColor)?;
                }
            }
            "grep" => {
                if let Some(pattern) = args.get("search_string").and_then(|v| v.as_str()) {
                    stdout()
                        .execute(SetForegroundColor(Color::Cyan))?
                        .execute(Print("'"))?
                        .execute(Print(pattern))?
                        .execute(Print("'"))?
                        .execute(ResetColor)?;
                }
            }
            "agent_run_shell_command" | "run_shell_command" => {
                if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                    let display_cmd = if cmd.len() > 60 {
                        format!("{}...", &cmd[..57])
                    } else {
                        cmd.to_string()
                    };
                    stdout()
                        .execute(SetForegroundColor(Color::Cyan))?
                        .execute(Print(display_cmd))?
                        .execute(ResetColor)?;
                }
            }
            _ => {
                // Generic: show compact JSON
                let compact = args.to_string();
                let display = if compact.len() > 80 {
                    format!("{}...", &compact[..77])
                } else {
                    compact
                };
                stdout()
                    .execute(SetAttribute(Attribute::Dim))?
                    .execute(Print(display))?
                    .execute(SetAttribute(Attribute::Reset))?;
            }
        }
        Ok(())
    }

    /// Render streaming text delta.
    fn render_text_delta(&self, delta: &TextDeltaMessage) -> std::io::Result<()> {
        print!("{}", delta.text);
        stdout().flush()?;
        Ok(())
    }

    /// Render thinking/reasoning text.
    fn render_thinking(&self, text: &str) -> std::io::Result<()> {
        stdout()
            .execute(SetAttribute(Attribute::Dim))?
            .execute(Print(text))?
            .execute(SetAttribute(Attribute::Reset))?;
        stdout().flush()?;
        Ok(())
    }

    /// Run a render loop consuming messages from a receiver.
    ///
    /// This is designed to be spawned as a task that renders all messages
    /// as they arrive from the bus.
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

    #[test]
    fn test_renderer_creation() {
        let renderer = TerminalRenderer::new();
        assert_eq!(renderer.style.success_color, Color::Green);
    }

    #[test]
    fn test_custom_style() {
        let mut style = RenderStyle::default();
        style.success_color = Color::Blue;
        let renderer = TerminalRenderer::with_style(style);
        assert_eq!(renderer.style.success_color, Color::Blue);
    }
}
