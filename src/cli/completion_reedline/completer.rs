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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Construction and setters ====================

    #[test]
    fn test_spot_completer_new() {
        let completer = SpotCompleter::new();
        assert!(completer.models.is_empty());
        assert!(completer.agents.is_empty());
        assert!(completer.sessions.is_empty());
        assert!(completer.mcp_servers.is_empty());
    }

    #[test]
    fn test_spot_completer_default() {
        let completer = SpotCompleter::default();
        assert!(completer.models.is_empty());
        assert!(completer.agents.is_empty());
        assert!(completer.sessions.is_empty());
        assert!(completer.mcp_servers.is_empty());
    }

    #[test]
    fn test_spot_completer_set_models() {
        let mut completer = SpotCompleter::new();
        completer.set_models(vec!["gpt-4".to_string(), "claude-3".to_string()]);
        assert_eq!(completer.models.len(), 2);
        assert_eq!(completer.models[0], "gpt-4");
        assert_eq!(completer.models[1], "claude-3");
    }

    #[test]
    fn test_spot_completer_set_agents() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec!["coder".to_string(), "assistant".to_string()]);
        assert_eq!(completer.agents.len(), 2);
        assert_eq!(completer.agents[0], "coder");
        assert_eq!(completer.agents[1], "assistant");
    }

    #[test]
    fn test_spot_completer_set_sessions() {
        let mut completer = SpotCompleter::new();
        completer.set_sessions(vec!["session1".to_string(), "session2".to_string()]);
        assert_eq!(completer.sessions.len(), 2);
        assert_eq!(completer.sessions[0], "session1");
    }

    #[test]
    fn test_spot_completer_set_mcp_servers() {
        let mut completer = SpotCompleter::new();
        completer.set_mcp_servers(vec!["server1".to_string(), "server2".to_string()]);
        assert_eq!(completer.mcp_servers.len(), 2);
        assert_eq!(completer.mcp_servers[0], "server1");
    }

    // ==================== Completer::complete() method tests ====================

    #[test]
    fn test_complete_empty_input() {
        let mut completer = SpotCompleter::new();
        let result = completer.complete("", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_complete_no_slash_prefix() {
        let mut completer = SpotCompleter::new();
        let result = completer.complete("help", 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_complete_pos_beyond_line_length() {
        let mut completer = SpotCompleter::new();
        let result = completer.complete("/help", 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_complete_command_no_space() {
        let mut completer = SpotCompleter::new();
        let result = completer.complete("/hel", 4);
        assert!(!result.is_empty());
        assert!(result.iter().any(|s| s.value == "/help"));
    }

    #[test]
    fn test_complete_model_with_prefix() {
        let mut completer = SpotCompleter::new();
        completer.set_models(vec![
            "gpt-4".to_string(),
            "gpt-3.5".to_string(),
            "claude-3".to_string(),
        ]);
        let result = completer.complete("/model gpt", 10);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| s.value.starts_with("gpt")));
    }

    #[test]
    fn test_complete_model_empty_prefix() {
        let mut completer = SpotCompleter::new();
        let models: Vec<String> = (0..15).map(|i| format!("model-{}", i)).collect();
        completer.set_models(models);
        let result = completer.complete("/model ", 7);
        // Should return up to 12 models
        assert_eq!(result.len(), 12);
    }

    #[test]
    fn test_complete_model_short_alias() {
        let mut completer = SpotCompleter::new();
        completer.set_models(vec!["gpt-4".to_string(), "claude-3".to_string()]);
        let result = completer.complete("/m ", 3);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_complete_pin_just_command() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec!["coder".to_string(), "assistant".to_string()]);
        completer.set_models(vec!["gpt-4".to_string()]);
        let result = completer.complete("/pin ", 5);
        // Should show agents, not models
        assert_eq!(result.len(), 2);
        assert!(result
            .iter()
            .all(|s| s.description == Some("agent".to_string())));
    }

    #[test]
    fn test_complete_pin_with_valid_agent() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec!["coder".to_string(), "assistant".to_string()]);
        completer.set_models(vec!["gpt-4".to_string(), "claude-3".to_string()]);
        let result = completer.complete("/pin coder ", 11);
        // Should show models now
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|s| s.value == "gpt-4"));
    }

    #[test]
    fn test_complete_pin_with_partial_agent() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec!["coder".to_string(), "assistant".to_string()]);
        let result = completer.complete("/pin cod", 8);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "coder");
    }

    #[test]
    fn test_complete_unpin_filters_agents() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec!["coder".to_string(), "assistant".to_string()]);
        let result = completer.complete("/unpin as", 9);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "assistant");
    }

    #[test]
    fn test_complete_agent_prefix() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec![
            "coder".to_string(),
            "assistant".to_string(),
            "code-review".to_string(),
        ]);
        let result = completer.complete("/agent co", 9);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| s.value.starts_with("co")));
    }

    #[test]
    fn test_complete_agent_short_alias() {
        let mut completer = SpotCompleter::new();
        completer.set_agents(vec!["coder".to_string(), "assistant".to_string()]);
        let result = completer.complete("/a ", 3);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_complete_session_load() {
        let mut completer = SpotCompleter::new();
        completer.set_sessions(vec!["session1".to_string(), "session2".to_string()]);
        let result = completer.complete("/load ", 6);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_complete_session_resume() {
        let mut completer = SpotCompleter::new();
        completer.set_sessions(vec!["session1".to_string(), "session2".to_string()]);
        let result = completer.complete("/resume ses", 11);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_complete_session_delete() {
        let mut completer = SpotCompleter::new();
        completer.set_sessions(vec!["session1".to_string(), "other".to_string()]);
        let result = completer.complete("/delete-session ses", 19);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "session1");
    }

    #[test]
    fn test_complete_mcp_subcommand_no_space() {
        let mut completer = SpotCompleter::new();
        let result = completer.complete("/mcp sta", 8);
        assert!(!result.is_empty());
        assert!(result.iter().any(|s| s.value == "start"));
        assert!(result.iter().any(|s| s.value == "start-all"));
        assert!(result.iter().any(|s| s.value == "status"));
    }

    #[test]
    fn test_complete_mcp_server_start() {
        let mut completer = SpotCompleter::new();
        completer.set_mcp_servers(vec!["filesystem".to_string(), "git".to_string()]);
        let result = completer.complete("/mcp start ", 11);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_complete_mcp_server_stop() {
        let mut completer = SpotCompleter::new();
        completer.set_mcp_servers(vec!["filesystem".to_string(), "git".to_string()]);
        let result = completer.complete("/mcp stop fi", 12);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "filesystem");
    }

    #[test]
    fn test_complete_unknown_returns_empty() {
        let mut completer = SpotCompleter::new();
        let result = completer.complete("/unknown-cmd arg", 16);
        assert!(result.is_empty());
    }

    // ==================== complete_command function tests ====================

    #[test]
    fn test_complete_command_full_match() {
        let result = complete_command("/help", 5);
        assert!(result.iter().any(|s| s.value == "/help"));
    }

    #[test]
    fn test_complete_command_partial_match() {
        let result = complete_command("/mod", 4);
        assert!(result.iter().any(|s| s.value == "/model"));
        assert!(result.iter().any(|s| s.value == "/models"));
        assert!(result.iter().any(|s| s.value == "/model_settings"));
    }

    #[test]
    fn test_complete_command_case_insensitive() {
        let result = complete_command("/HELP", 5);
        assert!(result.iter().any(|s| s.value == "/help"));
    }

    #[test]
    fn test_complete_command_limit_10() {
        // "/" should match all commands but be limited to 10
        let result = complete_command("/", 1);
        assert!(result.len() <= 10);
    }

    #[test]
    fn test_complete_command_no_match_returns_empty() {
        let result = complete_command("/zzzznonexistent", 16);
        assert!(result.is_empty());
    }
}
