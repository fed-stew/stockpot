//! Reedline completion with Tab-triggered menu.
//!
//! Type "/" then Tab to see commands. Menu filters as you type.

use nu_ansi_term::{Color, Style};
use reedline::{
    ColumnarMenu, Completer, Emacs, Highlighter, KeyCode, KeyModifiers,
    MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
    PromptHistorySearchStatus, Reedline, ReedlineEvent, ReedlineMenu,
    Span, StyledText, Suggestion,
};
use std::borrow::Cow;

/// All slash commands with descriptions
pub const COMMANDS: &[(&str, &str)] = &[
    ("/a", "Select agent"),
    ("/add-model", "Add custom model"),
    ("/agent", "Select agent"),
    ("/agents", "List agents"),
    ("/cd", "Change directory"),
    ("/chatgpt-auth", "ChatGPT OAuth login"),
    ("/claude-code-auth", "Claude Code OAuth"),
    ("/clear", "Clear screen"),
    ("/compact", "Compact message history"),
    ("/context", "Show context usage"),
    ("/delete-session", "Delete session"),
    ("/exit", "Exit"),
    ("/h", "Show help"),
    ("/help", "Show help"),
    ("/load", "Load session"),
    ("/m", "Select model"),
    ("/mcp", "MCP server management"),
    ("/model", "Select model"),
    ("/model_settings", "Model settings"),
    ("/models", "List available models"),
    ("/ms", "Model settings"),
    ("/new", "New conversation"),
    ("/pin", "Pin model to agent"),
    ("/pins", "List all agent pins"),
    ("/quit", "Exit"),
    ("/reasoning", "Set reasoning effort"),
    ("/resume", "Resume session"),
    ("/s", "Session info"),
    ("/save", "Save session"),
    ("/session", "Session info"),
    ("/sessions", "List sessions"),
    ("/set", "Configuration"),
    ("/show", "Show status"),
    ("/tools", "List tools"),
    ("/truncate", "Truncate history"),
    ("/unpin", "Unpin model"),
    ("/v", "Version info"),
    ("/verbosity", "Set verbosity"),
    ("/version", "Version info"),
    ("/yolo", "Toggle YOLO mode"),
];

/// MCP subcommands
pub const MCP_COMMANDS: &[&str] = &[
    "add", "disable", "enable", "help", "list", "remove", "restart",
    "start", "start-all", "status", "stop", "stop-all", "tools",
];

/// Completer for Stockpot commands
#[derive(Clone, Default)]
pub struct SpotCompleter {
    pub models: Vec<String>,
    pub agents: Vec<String>,
    pub sessions: Vec<String>,
    pub mcp_servers: Vec<String>,
}

impl SpotCompleter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.models = models;
    }

    pub fn set_agents(&mut self, agents: Vec<String>) {
        self.agents = agents;
    }

    pub fn set_sessions(&mut self, sessions: Vec<String>) {
        self.sessions = sessions;
    }

    pub fn set_mcp_servers(&mut self, servers: Vec<String>) {
        self.mcp_servers = servers;
    }
}

impl Completer for SpotCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        if pos > line.len() {
            return Vec::new();
        }

        let input = &line[..pos];

        if input.is_empty() || !input.starts_with('/') {
            return Vec::new();
        }

        // Command completion (no space yet)
        if !input.contains(' ') {
            let prefix = input.to_lowercase();
            return COMMANDS
                .iter()
                .filter(|(cmd, _)| cmd.to_lowercase().starts_with(&prefix))
                .take(10)
                .map(|(cmd, desc)| Suggestion {
                    value: cmd.to_string(),
                    description: Some(desc.to_string()),
                    extra: None,
                    span: Span::new(0, pos),
                    append_whitespace: true,
                    style: None,
                })
                .collect();
        }

        // Model completion: /model xxx, /m xxx
        if input.starts_with("/model ") || input.starts_with("/m ") {
            let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
            let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
            return self.models
                .iter()
                .filter(|m| prefix.is_empty() || m.to_lowercase().starts_with(&prefix))
                .take(12)
                .map(|m| Suggestion {
                    value: m.clone(),
                    description: None,
                    extra: None,
                    span: Span::new(start, pos),
                    append_whitespace: false,
                    style: None,
                })
                .collect();
        }

        // /pin completion - two-step flow:
        // 1. /pin <Tab> → show agents only
        // 2. /pin <agent> <Tab> → show models only
        if input.starts_with("/pin ") {
            let parts: Vec<&str> = input.split_whitespace().collect();
            
            match parts.len() {
                1 => {
                    // Just "/pin " - show only agents
                    let start = 5; // After "/pin "
                    return self.agents
                        .iter()
                        .map(|agent| Suggestion {
                            value: agent.clone(),
                            description: Some("agent".to_string()),
                            extra: None,
                            span: Span::new(start, pos),
                            append_whitespace: true,
                            style: None,
                        })
                        .collect();
                }
                2 => {
                    let first_arg = parts[1];
                    let is_valid_agent = self.agents.iter().any(|a| a == first_arg);
                    
                    // Check if input ends with space AND first_arg is a valid agent
                    if input.ends_with(' ') && is_valid_agent {
                        // "/pin <valid-agent> " - show models
                        let start = input.len();
                        return self.models
                            .iter()
                            .take(12)
                            .map(|m| Suggestion {
                                value: m.clone(),
                                description: None,
                                extra: None,
                                span: Span::new(start, pos),
                                append_whitespace: false,
                                style: None,
                            })
                            .collect();
                    } else {
                        // "/pin xxx" - filter agents only by prefix
                        let prefix = first_arg.to_lowercase();
                        let start = input.rfind(' ').map(|i| i + 1).unwrap_or(5);
                        return self.agents
                            .iter()
                            .filter(|a| a.to_lowercase().starts_with(&prefix))
                            .map(|agent| Suggestion {
                                value: agent.clone(),
                                description: Some("agent".to_string()),
                                extra: None,
                                span: Span::new(start, pos),
                                append_whitespace: true,
                                style: None,
                            })
                            .collect();
                    }
                }
                _ => {
                    // "/pin agent xxx" - second arg is always a model
                    let first_arg = parts[1];
                    if self.agents.iter().any(|a| a == first_arg) {
                        let prefix = parts.get(2).map(|s| s.to_lowercase()).unwrap_or_default();
                        let start = input.rfind(' ').map(|i| i + 1).unwrap_or(pos);
                        
                        return self.models
                            .iter()
                            .filter(|m| prefix.is_empty() || m.to_lowercase().starts_with(&prefix))
                            .take(12)
                            .map(|m| Suggestion {
                                value: m.clone(),
                                description: None,
                                extra: None,
                                span: Span::new(start, pos),
                                append_whitespace: false,
                                style: None,
                            })
                            .collect();
                    }
                }
            }
        }

        // /unpin completion - suggest agents
        if input.starts_with("/unpin ") {
            let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
            let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
            return self.agents
                .iter()
                .filter(|a| prefix.is_empty() || a.to_lowercase().starts_with(&prefix))
                .take(10)
                .map(|a| Suggestion {
                    value: a.clone(),
                    description: None,
                    extra: None,
                    span: Span::new(start, pos),
                    append_whitespace: false,
                    style: None,
                })
                .collect();
        }

        // Agent completion
        if input.starts_with("/agent ") || input.starts_with("/a ") {
            let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
            let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
            return self.agents
                .iter()
                .filter(|a| prefix.is_empty() || a.to_lowercase().starts_with(&prefix))
                .take(10)
                .map(|a| Suggestion {
                    value: a.clone(),
                    description: None,
                    extra: None,
                    span: Span::new(start, pos),
                    append_whitespace: false,
                    style: None,
                })
                .collect();
        }

        // Session completion
        if input.starts_with("/load ") || input.starts_with("/resume ") || input.starts_with("/delete-session ") {
            let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
            let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
            return self.sessions
                .iter()
                .filter(|s| prefix.is_empty() || s.to_lowercase().starts_with(&prefix))
                .take(10)
                .map(|s| Suggestion {
                    value: s.clone(),
                    description: None,
                    extra: None,
                    span: Span::new(start, pos),
                    append_whitespace: false,
                    style: None,
                })
                .collect();
        }

        // MCP subcommand completion
        if let Some(after_mcp) = input.strip_prefix("/mcp ") {
            if !after_mcp.contains(' ') {
                let prefix = after_mcp.to_lowercase();
                return MCP_COMMANDS
                    .iter()
                    .filter(|c| prefix.is_empty() || c.starts_with(&prefix))
                    .map(|c| Suggestion {
                        value: c.to_string(),
                        description: None,
                        extra: None,
                        span: Span::new(5, pos),
                        append_whitespace: true,
                        style: None,
                    })
                    .collect();
            }

            // MCP server name completion
            let parts: Vec<&str> = after_mcp.split_whitespace().collect();
            if !parts.is_empty() && ["start", "stop", "remove", "restart", "enable", "disable"].contains(&parts[0]) {
                let prefix = parts.get(1).copied().unwrap_or("").to_lowercase();
                let start = input.rfind(' ').map(|i| i + 1).unwrap_or(pos);
                return self.mcp_servers
                    .iter()
                    .filter(|s| prefix.is_empty() || s.to_lowercase().starts_with(&prefix))
                    .map(|s| Suggestion {
                        value: s.clone(),
                        description: None,
                        extra: None,
                        span: Span::new(start, pos),
                        append_whitespace: false,
                        style: None,
                    })
                    .collect();
            }
        }

        Vec::new()
    }
}

/// Stockpot prompt
pub struct SpotPrompt {
    pub agent_name: String,
    pub model_name: String,
}

impl SpotPrompt {
    pub fn new(agent: &str, model: &str) -> Self {
        Self {
            agent_name: agent.to_string(),
            model_name: model.to_string(),
        }
    }
}

impl Prompt for SpotPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "\x1b[1;33m{}\x1b[0m \x1b[2m[{}]\x1b[0m",
            self.agent_name, self.model_name
        ))
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

/// Syntax highlighter
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

/// Create reedline with Tab-triggered completion menu
pub fn create_reedline(completer: SpotCompleter) -> Reedline {
    // Clean menu style - no heavy borders
    let completion_menu = Box::new(
        ColumnarMenu::default()
            .with_name("completion_menu")
            .with_columns(1)
            .with_column_padding(2)
            .with_text_style(Style::new().fg(Color::Default))
            .with_selected_text_style(
                Style::new()
                    .fg(Color::Black)
                    .on(Color::Cyan)
            )
            .with_description_text_style(Style::new().fg(Color::DarkGray))
    );

    let mut keybindings = reedline::default_emacs_keybindings();

    // Tab to show/navigate menu
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );

    // Shift+Tab to go back
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );

    Reedline::create()
        .with_completer(Box::new(completer))
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_quick_completions(true)
        .with_partial_completions(true)
        .with_highlighter(Box::new(SpotHighlighter))
        .with_edit_mode(Box::new(Emacs::new(keybindings)))
}

// ============================================================================
// Dialoguer-based pickers (fallback for interactive selection)
// ============================================================================

/// Show command picker using dialoguer FuzzySelect
pub fn pick_command(prefix: &str) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let filtered: Vec<(&str, &str)> = COMMANDS
        .iter()
        .filter(|(cmd, _)| prefix.is_empty() || cmd.to_lowercase().contains(&prefix.to_lowercase()))
        .copied()
        .collect();

    if filtered.is_empty() {
        return None;
    }

    let items: Vec<String> = filtered
        .iter()
        .map(|(cmd, desc)| format!("{:<15} {}", cmd, desc))
        .collect();

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Command")
        .items(&items)
        .default(0)
        .max_length(8)
        .interact_opt()
        .ok()
        .flatten()
        .map(|idx| filtered[idx].0.to_string())
}

/// Show model picker
pub fn pick_model_completion(models: &[String], prefix: &str) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let filtered: Vec<&String> = models
        .iter()
        .filter(|m| prefix.is_empty() || m.to_lowercase().contains(&prefix.to_lowercase()))
        .collect();

    if filtered.is_empty() {
        return None;
    }

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Model")
        .items(&filtered)
        .default(0)
        .max_length(8)
        .interact_opt()
        .ok()
        .flatten()
        .map(|idx| filtered[idx].clone())
}

/// Show agent picker
pub fn pick_agent_completion(agents: &[String], prefix: &str) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let filtered: Vec<&String> = agents
        .iter()
        .filter(|a| prefix.is_empty() || a.to_lowercase().contains(&prefix.to_lowercase()))
        .collect();

    if filtered.is_empty() {
        return None;
    }

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Agent")
        .items(&filtered)
        .default(0)
        .max_length(8)
        .interact_opt()
        .ok()
        .flatten()
        .map(|idx| filtered[idx].clone())
}

/// Show session picker
pub fn pick_session_completion(sessions: &[String], prefix: &str) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let filtered: Vec<&String> = sessions
        .iter()
        .filter(|s| prefix.is_empty() || s.to_lowercase().contains(&prefix.to_lowercase()))
        .collect();

    if filtered.is_empty() {
        return None;
    }

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Session")
        .items(&filtered)
        .default(0)
        .max_length(8)
        .interact_opt()
        .ok()
        .flatten()
        .map(|idx| filtered[idx].clone())
}

/// Show MCP subcommand picker
pub fn pick_mcp_subcommand(prefix: &str) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let filtered: Vec<&str> = MCP_COMMANDS
        .iter()
        .filter(|c| prefix.is_empty() || c.contains(&prefix.to_lowercase()))
        .copied()
        .collect();

    if filtered.is_empty() {
        return None;
    }

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("MCP")
        .items(&filtered)
        .default(0)
        .max_length(8)
        .interact_opt()
        .ok()
        .flatten()
        .map(|idx| filtered[idx].to_string())
}

/// Show MCP server picker
pub fn pick_mcp_server(servers: &[String], prefix: &str) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let filtered: Vec<&String> = servers
        .iter()
        .filter(|s| prefix.is_empty() || s.to_lowercase().contains(&prefix.to_lowercase()))
        .collect();

    if filtered.is_empty() {
        return None;
    }

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Server")
        .items(&filtered)
        .default(0)
        .max_length(8)
        .interact_opt()
        .ok()
        .flatten()
        .map(|idx| filtered[idx].clone())
}

/// Check if a command is complete (exact match)
pub fn is_complete_command(input: &str) -> bool {
    COMMANDS.iter().any(|(cmd, _)| *cmd == input)
}

/// Context for completing input (data needed from REPL)
pub struct CompletionContext {
    pub models: Vec<String>,
    pub agents: Vec<String>,
    pub sessions: Vec<String>,
    pub mcp_servers: Vec<String>,
}

/// Try to complete partial input. Returns None if user cancelled picker.
pub fn try_complete_input(input: &str, ctx: &CompletionContext) -> Option<String> {
    let trimmed = input.trim();

    // Not a command - return as-is
    if !trimmed.starts_with('/') {
        return Some(trimmed.to_string());
    }

    // Just "/" - show command picker
    if trimmed == "/" {
        return pick_command("");
    }

    // /model xxx or /m xxx - model picker
    if trimmed.starts_with("/model ") || trimmed.starts_with("/m ") {
        let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
        if let Some(model) = pick_model_completion(&ctx.models, prefix) {
            let cmd = if trimmed.starts_with("/m ") { "/m" } else { "/model" };
            return Some(format!("{} {}", cmd, model));
        }
        return None;
    }

    // /pin - context-aware picker
    if trimmed.starts_with("/pin ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        match parts.len() {
            1 => {
                // Just "/pin " - could pick agent or model
                // For simplicity, default to model picker
                if let Some(model) = pick_model_completion(&ctx.models, "") {
                    return Some(format!("/pin {}", model));
                }
                return None;
            }
            2 => {
                let first_arg = parts[1];
                // Check if it's an agent name - then pick model for second arg
                if ctx.agents.contains(&first_arg.to_string()) {
                    if let Some(model) = pick_model_completion(&ctx.models, "") {
                        return Some(format!("/pin {} {}", first_arg, model));
                    }
                    return None;
                }
                // Otherwise treat as model prefix
                if let Some(model) = pick_model_completion(&ctx.models, first_arg) {
                    return Some(format!("/pin {}", model));
                }
                return None;
            }
            _ => {
                // /pin agent xxx - complete the model
                let agent = parts[1];
                let model_prefix = parts.get(2).copied().unwrap_or("");
                if let Some(model) = pick_model_completion(&ctx.models, model_prefix) {
                    return Some(format!("/pin {} {}", agent, model));
                }
                return None;
            }
        }
    }

    // /unpin - agent picker
    if trimmed.starts_with("/unpin ") {
        let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
        if let Some(agent) = pick_agent_completion(&ctx.agents, prefix) {
            return Some(format!("/unpin {}", agent));
        }
        return None;
    }

    // /agent xxx or /a xxx - agent picker
    if trimmed.starts_with("/agent ") || trimmed.starts_with("/a ") {
        let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
        if let Some(agent) = pick_agent_completion(&ctx.agents, prefix) {
            let cmd = if trimmed.starts_with("/a ") { "/a" } else { "/agent" };
            return Some(format!("{} {}", cmd, agent));
        }
        return None;
    }

    // /load xxx - session picker
    if trimmed.starts_with("/load ") {
        let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
        if let Some(session) = pick_session_completion(&ctx.sessions, prefix) {
            return Some(format!("/load {}", session));
        }
        return None;
    }

    // /mcp or /mcp xxx - MCP subcommand picker
    if trimmed == "/mcp" {
        if let Some(sub) = pick_mcp_subcommand("") {
            return Some(format!("/mcp {}", sub));
        }
        return None;
    }

    if trimmed.starts_with("/mcp ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() == 2 {
            let sub = parts[1];
            if ["start", "stop", "remove", "restart", "enable", "disable"].contains(&sub) {
                if let Some(server) = pick_mcp_server(&ctx.mcp_servers, "") {
                    return Some(format!("/mcp {} {}", sub, server));
                }
                return None;
            }
            if !MCP_COMMANDS.contains(&sub) {
                if let Some(completed) = pick_mcp_subcommand(sub) {
                    return Some(format!("/mcp {}", completed));
                }
                return None;
            }
        }
    }

    // /xxx without space - check if complete
    if !trimmed.contains(' ') {
        if is_complete_command(trimmed) {
            return Some(trimmed.to_string());
        }
        let prefix = &trimmed[1..];
        return pick_command(prefix);
    }

    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_complete_command() {
        assert!(is_complete_command("/help"));
        assert!(is_complete_command("/model"));
        assert!(!is_complete_command("/hel"));
        assert!(!is_complete_command("/mod"));
    }

    #[test]
    fn test_commands_sorted() {
        assert!(COMMANDS.iter().any(|(c, _)| *c == "/help"));
        assert!(COMMANDS.iter().any(|(c, _)| *c == "/model"));
        assert!(COMMANDS.iter().any(|(c, _)| *c == "/exit"));
    }
}
