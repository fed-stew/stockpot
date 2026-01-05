//! UI element rendering for terminal output.
//!
//! Handles rendering of:
//! - Text messages with levels (info, success, warning, error)
//! - Reasoning output
//! - Spinners and progress indicators
//! - Input prompts
//! - Dividers and screen clearing

use super::TerminalRenderer;
use crate::messaging::MessageLevel;
use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    ExecutableCommand,
};
use std::io::stdout;

impl TerminalRenderer {
    /// Render a text message with the appropriate color based on level.
    pub(super) fn render_text(&self, level: MessageLevel, text: &str) -> std::io::Result<()> {
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

    /// Render reasoning/thought process output.
    pub(super) fn render_reasoning(
        &self,
        reasoning: &str,
        next_steps: Option<&str>,
    ) -> std::io::Result<()> {
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

    /// Render a response (delegates to markdown rendering).
    pub(super) fn render_response(&self, content: &str) -> std::io::Result<()> {
        self.render_markdown(content)
    }

    /// Render a spinner/loading indicator.
    pub(super) fn render_spinner(&self, text: &str, is_active: bool) -> std::io::Result<()> {
        let icon = if is_active { "⏳" } else { "✓" };
        stdout()
            .execute(SetForegroundColor(Color::Cyan))?
            .execute(Print(format!("{} {}\n", icon, text)))?
            .execute(ResetColor)?;
        Ok(())
    }

    /// Render an input request prompt.
    pub(super) fn render_input_request(&self, prompt: &str) -> std::io::Result<()> {
        stdout()
            .execute(SetForegroundColor(Color::Yellow))?
            .execute(Print(format!("❓ {}\n", prompt)))?
            .execute(ResetColor)?;
        Ok(())
    }

    /// Render a horizontal divider.
    pub(super) fn render_divider(&self) -> std::io::Result<()> {
        println!(
            "───────────────────────────────────────────────────────────────────────────────"
        );
        Ok(())
    }

    /// Clear the terminal screen.
    pub(super) fn clear_screen(&self) -> std::io::Result<()> {
        use crossterm::terminal::{Clear, ClearType};
        stdout().execute(Clear(ClearType::All))?;
        Ok(())
    }
}
