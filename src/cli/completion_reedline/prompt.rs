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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== SpotPrompt tests ====================

    #[test]
    fn test_spot_prompt_new() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        assert_eq!(prompt.agent_name, "coder");
        assert_eq!(prompt.model_name, "gpt-4");
        assert!(!prompt.is_pinned);
    }

    #[test]
    fn test_spot_prompt_with_pinned_true() {
        let prompt = SpotPrompt::with_pinned("coder", "gpt-4", true);
        assert_eq!(prompt.agent_name, "coder");
        assert_eq!(prompt.model_name, "gpt-4");
        assert!(prompt.is_pinned);
    }

    #[test]
    fn test_spot_prompt_with_pinned_false() {
        let prompt = SpotPrompt::with_pinned("assistant", "claude-3", false);
        assert_eq!(prompt.agent_name, "assistant");
        assert_eq!(prompt.model_name, "claude-3");
        assert!(!prompt.is_pinned);
    }

    #[test]
    fn test_render_prompt_left_not_pinned() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        let rendered = prompt.render_prompt_left();
        // Should contain agent name and model name
        assert!(rendered.contains("coder"));
        assert!(rendered.contains("gpt-4"));
        // Should not contain pin emoji when not pinned
        assert!(!rendered.contains("\u{1F4CC}")); // pin emoji
    }

    #[test]
    fn test_render_prompt_left_pinned() {
        let prompt = SpotPrompt::with_pinned("coder", "gpt-4", true);
        let rendered = prompt.render_prompt_left();
        // Should contain agent name and model name
        assert!(rendered.contains("coder"));
        assert!(rendered.contains("gpt-4"));
        // Should contain pin indicator when pinned
        assert!(rendered.contains("\u{1F4CC}")); // pin emoji
    }

    #[test]
    fn test_render_prompt_right_empty() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        let rendered = prompt.render_prompt_right();
        assert!(rendered.is_empty());
    }

    #[test]
    fn test_render_prompt_indicator() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        let indicator = prompt.render_prompt_indicator(PromptEditMode::Default);
        // Contains the pot emoji
        assert!(indicator.contains("\u{1F372}")); // pot of food emoji
    }

    #[test]
    fn test_render_prompt_multiline_indicator() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        let indicator = prompt.render_prompt_multiline_indicator();
        assert_eq!(indicator.as_ref(), "... ");
    }

    #[test]
    fn test_render_prompt_history_search_passing() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        let hs =
            PromptHistorySearch::new(PromptHistorySearchStatus::Passing, "test query".to_string());
        let rendered = prompt.render_prompt_history_search_indicator(hs);
        assert!(rendered.contains("search:"));
        assert!(rendered.contains("test query"));
        // Should not contain "failing" prefix
        assert!(!rendered.contains("failing"));
    }

    #[test]
    fn test_render_prompt_history_search_failing() {
        let prompt = SpotPrompt::new("coder", "gpt-4");
        let hs =
            PromptHistorySearch::new(PromptHistorySearchStatus::Failing, "test query".to_string());
        let rendered = prompt.render_prompt_history_search_indicator(hs);
        assert!(rendered.contains("failing"));
        assert!(rendered.contains("search:"));
        assert!(rendered.contains("test query"));
    }

    // ==================== SpotHighlighter tests ====================

    #[test]
    fn test_highlighter_non_command_no_highlight() {
        let highlighter = SpotHighlighter;
        let styled = highlighter.highlight("hello world", 0);
        // Non-command text should have default style
        assert_eq!(styled.buffer.len(), 1);
        assert_eq!(styled.buffer[0].1, "hello world");
    }

    #[test]
    fn test_highlighter_valid_command_styled() {
        let highlighter = SpotHighlighter;
        let styled = highlighter.highlight("/help", 0);
        assert_eq!(styled.buffer.len(), 1);
        assert_eq!(styled.buffer[0].1, "/help");
        // Valid command should be cyan and bold
        assert_eq!(styled.buffer[0].0.foreground, Some(Color::Cyan));
    }

    #[test]
    fn test_highlighter_invalid_command_styled_differently() {
        let highlighter = SpotHighlighter;
        let styled = highlighter.highlight("/notacommand", 0);
        assert_eq!(styled.buffer.len(), 1);
        assert_eq!(styled.buffer[0].1, "/notacommand");
        // Invalid command should be yellow (not cyan)
        assert_eq!(styled.buffer[0].0.foreground, Some(Color::Yellow));
    }

    #[test]
    fn test_highlighter_command_with_args() {
        let highlighter = SpotHighlighter;
        let styled = highlighter.highlight("/model gpt-4", 0);
        assert_eq!(styled.buffer.len(), 2);
        // First part is the command
        assert_eq!(styled.buffer[0].1, "/model");
        assert_eq!(styled.buffer[0].0.foreground, Some(Color::Cyan));
        // Second part is the argument with default style
        assert_eq!(styled.buffer[1].1, " gpt-4");
    }

    #[test]
    fn test_highlighter_clone_impl() {
        let highlighter = SpotHighlighter;
        let cloned = highlighter.clone();
        // Both should produce identical output
        let styled1 = highlighter.highlight("/help", 0);
        let styled2 = cloned.highlight("/help", 0);
        assert_eq!(styled1.buffer.len(), styled2.buffer.len());
        assert_eq!(styled1.buffer[0].1, styled2.buffer[0].1);
    }
}
