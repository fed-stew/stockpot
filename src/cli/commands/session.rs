//! Session management commands.

use crate::agents::AgentManager;
use crate::config::Settings;
use crate::db::Database;
use crate::session::{format_relative_time, SessionData, SessionManager};
use crate::tokens::estimate_tokens;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use serdes_ai_core::ModelRequest;

/// Save the current session.
pub fn save(
    session_manager: &SessionManager,
    agents: &AgentManager,
    messages: &[ModelRequest],
    model: &str,
    name: &str,
) -> Option<String> {
    let agent_name = agents.current_name();

    let session_name = if name.is_empty() {
        session_manager.generate_name(&agent_name)
    } else {
        name.to_string()
    };

    match session_manager.save(&session_name, messages, &agent_name, model) {
        Ok(meta) => {
            println!("💾 Session saved: \x1b[1m{}\x1b[0m", session_name);
            println!(
                "   {} messages, ~{} tokens",
                meta.message_count, meta.token_estimate
            );
            Some(session_name)
        }
        Err(e) => {
            println!("❌ Failed to save session: {}", e);
            None
        }
    }
}

/// Load a session or show picker.
pub fn load(
    session_manager: &SessionManager,
    agents: &mut AgentManager,
    name: &str,
) -> Option<(String, SessionData)> {
    if name.is_empty() {
        // Show session picker
        match session_manager.list() {
            Ok(sessions) => {
                if sessions.is_empty() {
                    println!("  No saved sessions.");
                    println!("  Use /save to save the current conversation.");
                } else {
                    println!("\n\x1b[1m📚 Available Sessions:\x1b[0m\n");
                    for (i, session) in sessions.iter().take(10).enumerate() {
                        println!("  {}. \x1b[1m{}\x1b[0m", i + 1, session.name);
                        println!(
                            "     {} messages, {} - {}",
                            session.message_count,
                            session.agent,
                            format_relative_time(session.updated_at)
                        );
                    }
                    println!("\n\x1b[2mUse /load <name> to load a session\x1b[0m\n");
                }
            }
            Err(e) => println!("❌ Failed to list sessions: {}", e),
        }
        return None;
    }

    match session_manager.load(name) {
        Ok(session) => {
            // Switch agent if different
            if agents.current_name() != session.meta.agent && agents.exists(&session.meta.agent) {
                let _ = agents.switch(&session.meta.agent);
            }

            println!("📥 Loaded session: \x1b[1m{}\x1b[0m", name);
            println!(
                "   {} messages, agent: {}, model: {}",
                session.meta.message_count, session.meta.agent, session.meta.model
            );

            Some((name.to_string(), session))
        }
        Err(e) => {
            println!("❌ Failed to load session: {}", e);
            None
        }
    }
}

/// List saved sessions.
pub fn list(session_manager: &SessionManager, current_session: Option<&str>) {
    match session_manager.list() {
        Ok(sessions) => {
            if sessions.is_empty() {
                println!("\n  No saved sessions.");
                println!("  Use /save to save the current conversation.\n");
            } else {
                println!("\n\x1b[1m📚 Saved Sessions:\x1b[0m\n");
                for session in &sessions {
                    let current_marker = if current_session == Some(&session.name) {
                        "→ "
                    } else {
                        "  "
                    };
                    println!("{}\x1b[1m{}\x1b[0m", current_marker, session.name);
                    println!(
                        "    {} msgs, ~{} tokens | {} | {}",
                        session.message_count,
                        session.token_estimate,
                        session.agent,
                        format_relative_time(session.updated_at)
                    );
                }
                println!();
            }
        }
        Err(e) => println!("❌ Failed to list sessions: {}", e),
    }
}

/// Delete a session.
pub fn delete(session_manager: &SessionManager, name: &str) {
    if name.is_empty() {
        println!("❌ Please specify a session name: /delete-session <name>");
        return;
    }

    match session_manager.delete(name) {
        Ok(()) => println!("🗑️  Deleted session: {}", name),
        Err(e) => println!("❌ Failed to delete session: {}", e),
    }
}

/// Handle the /context command - show context usage info with visual bar.
pub fn cmd_context(
    db: &Database,
    messages: &[ModelRequest],
    current_session: Option<&str>,
    agent_name: &str,
    context_length: usize,
) {
    let token_count = estimate_tokens(messages);
    let usage_pct = if context_length > 0 {
        (token_count as f64 / context_length as f64) * 100.0
    } else {
        0.0
    };

    println!("\n\x1b[1m📊 Context Usage\x1b[0m\n");
    println!("  Messages:    {}", messages.len());
    println!("  Tokens:      ~{}", token_count);
    println!("  Context:     {} max", context_length);
    println!("  Usage:       {:.1}%", usage_pct);

    // Visual bar
    let bar_width = 30;
    let filled = ((usage_pct / 100.0) * bar_width as f64) as usize;
    let filled = filled.min(bar_width);
    let empty = bar_width - filled;
    let color = if usage_pct > 80.0 {
        "31"
    } else if usage_pct > 60.0 {
        "33"
    } else {
        "32"
    };
    println!(
        "  [\x1b[{}m{}\x1b[0m{}]",
        color,
        "█".repeat(filled),
        "░".repeat(empty)
    );

    if let Some(session) = current_session {
        println!("\n  Session:     {}", session);
    }
    println!("  Agent:       {}", agent_name);

    // Check for pinned model in database
    let settings = Settings::new(db);
    if let Some(pinned) = settings.get_agent_pinned_model(agent_name) {
        println!("  Pinned:      {}", pinned);
    }
    println!();
}

/// Handle the /compact command.
pub fn cmd_compact(messages: &mut Vec<ModelRequest>, args: &str) {
    if messages.is_empty() {
        println!("Nothing to compact.");
        return;
    }

    let keep: usize = args.parse().unwrap_or(10);
    println!("🗜️  Compacting (keeping last {} messages)...", keep);
    let (before, after) = compact_truncate(messages, keep);
    let after_tokens = crate::tokens::estimate_tokens(messages);
    println!(
        "✅ Compacted: {} → {} messages (~{} tokens)",
        before, after, after_tokens
    );
}

/// Compact message history using truncation strategy.
/// Keeps the first message (often system prompt) and the last N messages.
/// Returns (before_count, after_count).
fn compact_truncate(messages: &mut Vec<ModelRequest>, keep_recent: usize) -> (usize, usize) {
    let before = messages.len();

    // Need at least first + keep_recent messages to compact
    if before <= keep_recent + 1 {
        return (before, before); // Nothing to do
    }

    // Keep first message (usually system prompt) + last N messages
    let first_msg = messages.remove(0);
    let keep_count = keep_recent.min(messages.len());
    let start_idx = messages.len().saturating_sub(keep_count);
    let recent: Vec<_> = messages.drain(start_idx..).collect();

    messages.clear();
    messages.push(first_msg);
    messages.extend(recent);

    (before, messages.len())
}

/// Interactive session loader using fuzzy select.
pub fn load_interactive(
    session_manager: &SessionManager,
    agents: &mut AgentManager,
) -> Option<(String, SessionData)> {
    let sessions = match session_manager.list() {
        Ok(s) if !s.is_empty() => s,
        Ok(_) => {
            println!("  No saved sessions found.");
            return None;
        }
        Err(e) => {
            println!("❌ Failed to list sessions: {}", e);
            return None;
        }
    };

    let display: Vec<String> = sessions
        .iter()
        .map(|s| {
            format!(
                "{} ({} msgs, {} - {})",
                s.name,
                s.message_count,
                s.agent,
                format_relative_time(s.updated_at)
            )
        })
        .collect();

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select session to load")
        .items(&display)
        .interact_opt()
        .ok()??;

    let name = &sessions[selection].name;
    match session_manager.load(name) {
        Ok(data) => {
            if agents.current_name() != data.meta.agent && agents.exists(&data.meta.agent) {
                let _ = agents.switch(&data.meta.agent);
            }

            println!(
                "📥 Loaded: \x1b[1m{}\x1b[0m ({} messages)",
                name, data.meta.message_count
            );
            Some((name.clone(), data))
        }
        Err(e) => {
            println!("❌ Failed to load: {}", e);
            None
        }
    }
}

/// Show current session info.
pub fn show_session(current_session: Option<&str>, autosave_enabled: bool) {
    match current_session {
        Some(name) => {
            println!("📋 Current session: \x1b[1m{}\x1b[0m", name);
        }
        None => {
            println!("📋 No active session");
            if autosave_enabled {
                println!("   \x1b[2m(Auto-save will create one after first response)\x1b[0m");
            } else {
                println!("   \x1b[2mUse /save to create a session\x1b[0m");
            }
        }
    }
}

/// Show interactive command picker - returns the command to execute
pub fn command_picker() -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, Select};

    let commands = vec![
        ("model", "Select model (interactive)"),
        ("agent", "Select agent (interactive)"),
        ("show", "Show current status"),
        ("context", "Show context usage"),
        ("resume", "Load a saved session"),
        ("save", "Save current session"),
        ("compact", "Compact message history"),
        ("ms", "Edit model settings"),
        ("mcp", "MCP server management"),
        ("tools", "List available tools"),
        ("help", "Show all commands"),
        ("yolo", "Toggle YOLO mode"),
        ("set", "Show/edit configuration"),
        ("new", "Start new conversation"),
        ("exit", "Exit stockpot"),
    ];

    let display: Vec<String> = commands
        .iter()
        .map(|(cmd, desc)| format!("/{:<12} {}", cmd, desc))
        .collect();

    println!(); // Add spacing

    match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select command")
        .items(&display)
        .default(0)
        .interact_opt()
    {
        Ok(Some(idx)) => {
            let (cmd, _) = commands[idx];
            println!("\x1b[2m> /{}\x1b[0m\n", cmd);
            Some(format!("/{}", cmd))
        }
        _ => {
            println!("Cancelled.");
            None
        }
    }
}

/// Auto-save session after a response.
/// Returns the new session name if one was created.
pub fn auto_save(
    session_manager: &SessionManager,
    current_session: &Option<String>,
    messages: &[ModelRequest],
    agent_name: &str,
    model: &str,
) -> Option<String> {
    if let Some(ref session_name) = current_session {
        // Update existing session silently
        let _ = session_manager.save(session_name, messages, agent_name, model);
        None
    } else if messages.len() >= 2 {
        // Create auto-session after first exchange
        let name = format!("auto-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        if session_manager
            .save(&name, messages, agent_name, model)
            .is_ok()
        {
            Some(name)
        } else {
            None
        }
    } else {
        None
    }
}
