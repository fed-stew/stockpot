//! CompletionContext and try_complete_input for input completion logic.

use super::picker::{
    pick_agent_completion, pick_command, pick_mcp_server, pick_mcp_subcommand,
    pick_model_completion, pick_session_completion,
};
use super::{is_complete_command, MCP_COMMANDS};

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
        return complete_model_command(trimmed, ctx);
    }

    // /pin - context-aware picker
    if trimmed.starts_with("/pin ") {
        return complete_pin_command(trimmed, ctx);
    }

    // /unpin - agent picker
    if trimmed.starts_with("/unpin ") {
        return complete_unpin_command(trimmed, ctx);
    }

    // /agent xxx or /a xxx - agent picker
    if trimmed.starts_with("/agent ") || trimmed.starts_with("/a ") {
        return complete_agent_command(trimmed, ctx);
    }

    // /load xxx - session picker
    if trimmed.starts_with("/load ") {
        return complete_load_command(trimmed, ctx);
    }

    // /mcp or /mcp xxx - MCP subcommand picker
    if trimmed == "/mcp" {
        return complete_mcp_bare(ctx);
    }

    if trimmed.starts_with("/mcp ") {
        return complete_mcp_with_args(trimmed, ctx);
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

fn complete_model_command(trimmed: &str, ctx: &CompletionContext) -> Option<String> {
    let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
    if let Some(model) = pick_model_completion(&ctx.models, prefix) {
        let cmd = if trimmed.starts_with("/m ") {
            "/m"
        } else {
            "/model"
        };
        return Some(format!("{} {}", cmd, model));
    }
    None
}

fn complete_pin_command(trimmed: &str, ctx: &CompletionContext) -> Option<String> {
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    match parts.len() {
        1 => {
            // Just "/pin " - could pick agent or model
            // For simplicity, default to model picker
            if let Some(model) = pick_model_completion(&ctx.models, "") {
                return Some(format!("/pin {}", model));
            }
            None
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
            None
        }
        _ => {
            // /pin agent xxx - complete the model
            let agent = parts[1];
            let model_prefix = parts.get(2).copied().unwrap_or("");
            if let Some(model) = pick_model_completion(&ctx.models, model_prefix) {
                return Some(format!("/pin {} {}", agent, model));
            }
            None
        }
    }
}

fn complete_unpin_command(trimmed: &str, ctx: &CompletionContext) -> Option<String> {
    let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
    if let Some(agent) = pick_agent_completion(&ctx.agents, prefix) {
        return Some(format!("/unpin {}", agent));
    }
    None
}

fn complete_agent_command(trimmed: &str, ctx: &CompletionContext) -> Option<String> {
    let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
    if let Some(agent) = pick_agent_completion(&ctx.agents, prefix) {
        let cmd = if trimmed.starts_with("/a ") {
            "/a"
        } else {
            "/agent"
        };
        return Some(format!("{} {}", cmd, agent));
    }
    None
}

fn complete_load_command(trimmed: &str, ctx: &CompletionContext) -> Option<String> {
    let prefix = trimmed.split_whitespace().nth(1).unwrap_or("");
    if let Some(session) = pick_session_completion(&ctx.sessions, prefix) {
        return Some(format!("/load {}", session));
    }
    None
}

fn complete_mcp_bare(_ctx: &CompletionContext) -> Option<String> {
    if let Some(sub) = pick_mcp_subcommand("") {
        return Some(format!("/mcp {}", sub));
    }
    None
}

fn complete_mcp_with_args(trimmed: &str, ctx: &CompletionContext) -> Option<String> {
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
    Some(trimmed.to_string())
}
