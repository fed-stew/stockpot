//! Dialoguer-based pickers for interactive selection.

use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use super::{COMMANDS, MCP_COMMANDS};

/// Show command picker using dialoguer FuzzySelect
pub fn pick_command(prefix: &str) -> Option<String> {
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
