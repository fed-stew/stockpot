//! SpotPrompt and SpotHighlighter for Reedline.

use nu_ansi_term::{Color, Style};
use reedline::{
    Highlighter, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, StyledText,
};
use std::borrow::Cow;

use super::COMMANDS;

/// Stockpot prompt
pub struct SpotPrompt {
    pub agent_name: String,
    pub model_name: String,
    pub is_pinned: bool,
}

impl SpotPrompt {
    pub fn new(agent: &str, model: &str) -> Self {
        Self {
            agent_name: agent.to_string(),
            model_name: model.to_string(),
            is_pinned: false,
        }
    }

    pub fn with_pinned(agent: &str, model: &str, is_pinned: bool) -> Self {
        Self {
            agent_name: agent.to_string(),
            model_name: model.to_string(),
            is_pinned,
        }
    }
}

impl Prompt for SpotPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        if self.is_pinned {
            // Show pinned indicator with magenta color
            Cow::Owned(format!(
                "\x1b[1;33m{}\x1b[0m \x1b[35m[📌 {}]\x1b[0m",
                self.agent_name, self.model_name
            ))
        } else {
            Cow::Owned(format!(
                "\x1b[1;33m{}\x1b[0m \x1b[2m[{}]\x1b[0m",
                self.agent_name, self.model_name
            ))
        }
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(" 🍲 ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(&self, hs: PromptHistorySearch) -> Cow<'_, str> {
        let prefix = match hs.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!("({}search: {}) ", prefix, hs.term))
    }
}

/// Syntax highlighter for slash commands
#[derive(Clone)]
pub struct SpotHighlighter;

impl Highlighter for SpotHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();

        if line.starts_with('/') {
            let cmd_end = line.find(' ').unwrap_or(line.len());
            let cmd = &line[..cmd_end];
            let is_valid = COMMANDS.iter().any(|(c, _)| *c == cmd);

            if is_valid {
                styled.push((Style::new().fg(Color::Cyan).bold(), cmd.to_string()));
            } else {
                styled.push((Style::new().fg(Color::Yellow), cmd.to_string()));
            }

            if cmd_end < line.len() {
                styled.push((Style::default(), line[cmd_end..].to_string()));
            }
        } else {
            styled.push((Style::default(), line.to_string()));
        }

        styled
    }
}
