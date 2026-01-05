//! SpotCompleter - Reedline completion for slash commands.

use reedline::{Completer, Span, Suggestion};

use super::{COMMANDS, MCP_COMMANDS};

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
            return complete_command(input, pos);
        }

        // Model completion: /model xxx, /m xxx
        if input.starts_with("/model ") || input.starts_with("/m ") {
            return self.complete_model(input, pos);
        }

        // /pin completion - two-step flow
        if input.starts_with("/pin ") {
            return self.complete_pin(input, pos);
        }

        // /unpin completion - suggest agents
        if input.starts_with("/unpin ") {
            return self.complete_agent_arg(input, pos);
        }

        // Agent completion
        if input.starts_with("/agent ") || input.starts_with("/a ") {
            return self.complete_agent_arg(input, pos);
        }

        // Session completion
        if input.starts_with("/load ")
            || input.starts_with("/resume ")
            || input.starts_with("/delete-session ")
        {
            return self.complete_session(input, pos);
        }

        // MCP subcommand completion
        if let Some(after_mcp) = input.strip_prefix("/mcp ") {
            return self.complete_mcp(after_mcp, input, pos);
        }

        Vec::new()
    }
}

impl SpotCompleter {
    fn complete_model(&self, input: &str, pos: usize) -> Vec<Suggestion> {
        let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
        let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
        self.models
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
            .collect()
    }

    fn complete_pin(&self, input: &str, pos: usize) -> Vec<Suggestion> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.len() {
            1 => {
                // Just "/pin " - show only agents
                let start = 5; // After "/pin "
                self.agents
                    .iter()
                    .map(|agent| Suggestion {
                        value: agent.clone(),
                        description: Some("agent".to_string()),
                        extra: None,
                        span: Span::new(start, pos),
                        append_whitespace: true,
                        style: None,
                    })
                    .collect()
            }
            2 => {
                let first_arg = parts[1];
                let is_valid_agent = self.agents.iter().any(|a| a == first_arg);

                if input.ends_with(' ') && is_valid_agent {
                    // "/pin <valid-agent> " - show models
                    let start = input.len();
                    self.models
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
                        .collect()
                } else {
                    // "/pin xxx" - filter agents only by prefix
                    let prefix = first_arg.to_lowercase();
                    let start = input.rfind(' ').map(|i| i + 1).unwrap_or(5);
                    self.agents
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
                        .collect()
                }
            }
            _ => {
                // "/pin agent xxx" - second arg is always a model
                let first_arg = parts[1];
                if self.agents.iter().any(|a| a == first_arg) {
                    let prefix = parts.get(2).map(|s| s.to_lowercase()).unwrap_or_default();
                    let start = input.rfind(' ').map(|i| i + 1).unwrap_or(pos);

                    self.models
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
                        .collect()
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn complete_agent_arg(&self, input: &str, pos: usize) -> Vec<Suggestion> {
        let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
        let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
        self.agents
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
            .collect()
    }

    fn complete_session(&self, input: &str, pos: usize) -> Vec<Suggestion> {
        let prefix = input.split_whitespace().nth(1).unwrap_or("").to_lowercase();
        let start = input.find(' ').map(|i| i + 1).unwrap_or(pos);
        self.sessions
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
            .collect()
    }

    fn complete_mcp(&self, after_mcp: &str, input: &str, pos: usize) -> Vec<Suggestion> {
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
        if !parts.is_empty()
            && ["start", "stop", "remove", "restart", "enable", "disable"].contains(&parts[0])
        {
            let prefix = parts.get(1).copied().unwrap_or("").to_lowercase();
            let start = input.rfind(' ').map(|i| i + 1).unwrap_or(pos);
            return self
                .mcp_servers
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

        Vec::new()
    }
}

/// Complete a command (no space yet)
fn complete_command(input: &str, pos: usize) -> Vec<Suggestion> {
    let prefix = input.to_lowercase();
    COMMANDS
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
        .collect()
}
