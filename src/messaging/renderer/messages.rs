//! Message-specific rendering for terminal output.
//!
//! Handles rendering of:
//! - Shell command output
//! - File operations
//! - Diff output
//! - Agent lifecycle events
//! - Tool execution events
//! - Streaming text and thinking

use super::TerminalRenderer;
use crate::messaging::{
    AgentEvent, AgentMessage, DiffLineType, FileOperation, TextDeltaMessage, ToolMessage,
    ToolStatus,
};
use crossterm::{
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    ExecutableCommand,
};
use std::io::{stdout, Write};

impl TerminalRenderer {
    /// Render shell command output.
    pub(super) fn render_shell(
        &self,
        command: &str,
        output: Option<&str>,
        exit_code: Option<i32>,
    ) -> std::io::Result<()> {
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

    /// Render file operation result.
    pub(super) fn render_file(
        &self,
        op: &FileOperation,
        path: &str,
        _content: Option<&str>,
    ) -> std::io::Result<()> {
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

    /// Render diff output with colored additions/removals.
    pub(super) fn render_diff(
        &self,
        path: &str,
        lines: &[crate::messaging::DiffLine],
    ) -> std::io::Result<()> {
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

    /// Render agent lifecycle events (start/complete/error).
    pub(super) fn render_agent_event(&self, msg: &AgentMessage) -> std::io::Result<()> {
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
    pub(super) fn render_tool_event(&self, msg: &ToolMessage) -> std::io::Result<()> {
        match msg.status {
            ToolStatus::Started => {
                // Just print the tool name
                stdout()
                    .execute(SetForegroundColor(Color::Yellow))?
                    .execute(Print("\n🔧 "))?
                    .execute(Print(&msg.tool_name))?
                    .execute(ResetColor)?;
                stdout().flush()?;
            }
            ToolStatus::ArgsStreaming => {
                // Nothing to show during streaming
            }
            ToolStatus::Executing => {
                // Show the args after the tool name
                if let Some(ref args) = msg.args {
                    stdout().execute(Print(" "))?;
                    self.render_tool_args(&msg.tool_name, args)?;
                }
                stdout().flush()?;
            }
            ToolStatus::Completed => {
                // Print checkmark on same line and newline
                stdout()
                    .execute(SetForegroundColor(Color::Green))?
                    .execute(Print(" ✓"))?
                    .execute(ResetColor)?;
                println!();
            }
            ToolStatus::Failed => {
                // Print error and newline
                if let Some(ref err) = msg.error {
                    let display_err = if err.len() > 60 {
                        format!("{}...", &err[..57])
                    } else {
                        err.clone()
                    };
                    stdout()
                        .execute(SetForegroundColor(Color::Red))?
                        .execute(Print(" ✗ "))?
                        .execute(Print(display_err))?
                        .execute(ResetColor)?;
                } else {
                    stdout()
                        .execute(SetForegroundColor(Color::Red))?
                        .execute(Print(" ✗ failed"))?
                        .execute(ResetColor)?;
                }
                println!();
            }
        }
        Ok(())
    }

    /// Render tool arguments in a nice format based on tool type.
    pub(super) fn render_tool_args(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> std::io::Result<()> {
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
    pub(super) fn render_text_delta(&self, delta: &TextDeltaMessage) -> std::io::Result<()> {
        print!("{}", delta.text);
        stdout().flush()?;
        Ok(())
    }

    /// Render thinking/reasoning text.
    pub(super) fn render_thinking(&self, text: &str) -> std::io::Result<()> {
        stdout()
            .execute(SetAttribute(Attribute::Dim))?
            .execute(Print(text))?
            .execute(SetAttribute(Attribute::Reset))?;
        stdout().flush()?;
        Ok(())
    }
}
