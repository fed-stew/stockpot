//! Command handling for the REPL.
//!
//! This module handles all slash commands like /help, /model, /agent, etc.
//! Each command is dispatched from the main handle_command function.

use crate::agents::{AgentManager, UserMode};
use crate::cli::commands::{context, core, mcp, session};
use crate::cli::model_picker::{edit_model_settings, pick_agent, pick_model, show_model_settings};
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::MessageBus;
use crate::models::ModelRegistry;
use crate::session::SessionManager;
use crate::tools::SpotToolRegistry;
use serdes_ai_core::ModelRequest;

use super::prompt;

/// Result of handling a command.
pub enum CommandResult {
    /// Continue the REPL loop
    Continue,
    /// Exit the REPL
    Exit,
}

/// Handle a slash command.
///
/// Returns `CommandResult::Exit` if the user wants to quit, otherwise `Continue`.
#[allow(clippy::too_many_arguments)]
pub async fn handle_command(
    db: &Database,
    agents: &mut AgentManager,
    model_registry: &mut ModelRegistry,
    message_bus: &MessageBus,
    message_history: &mut Vec<ModelRequest>,
    current_model: &mut String,
    current_session: &mut Option<String>,
    tool_registry: &SpotToolRegistry,
    mcp_manager: &McpManager,
    session_manager: &SessionManager,
    input: &str,
) -> anyhow::Result<CommandResult> {
    let parts: Vec<&str> = input[1..].splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    // Handle bare "/" - show interactive command picker
    if cmd.is_empty() {
        return show_command_picker(
            db,
            agents,
            model_registry,
            message_bus,
            message_history,
            current_model,
            current_session,
            tool_registry,
            mcp_manager,
            session_manager,
        )
        .await;
    }

    match cmd.as_str() {
        "help" | "h" | "?" => core::show_help(),
        "exit" | "quit" | "q" => return Ok(CommandResult::Exit),
        "clear" | "cls" => print!("\x1b[2J\x1b[1;1H"),
        "new" => {
            message_history.clear();
            *current_session = None;
            core::cmd_new();
        }
        "model" | "m" => {
            if args.is_empty() {
                // Interactive picker
                if let Some(selected) = pick_model(db, model_registry, current_model) {
                    *current_model = selected.clone();
                    let settings = Settings::new(db);
                    let _ = settings.set("model", &selected);
                    println!("✅ Switched to model: \x1b[1;33m{}\x1b[0m", selected);
                }
            } else {
                // Direct set
                *current_model = args.to_string();
                let settings = Settings::new(db);
                let _ = settings.set("model", args);
                println!("✅ Switched to model: \x1b[1;33m{}\x1b[0m", args);
            }
        }
        "models" => core::show_models(db, model_registry, current_model),
        "agent" | "a" => {
            if args.is_empty() {
                // Interactive picker
                let user_mode = Settings::new(db).user_mode();
                let agent_list: Vec<(String, String)> = agents
                    .list_filtered(user_mode)
                    .into_iter()
                    .map(|info| (info.name.clone(), info.display_name.clone()))
                    .collect();

                if let Some(selected) = pick_agent(&agent_list, &agents.current_name()) {
                    if agents.exists(&selected) {
                        agents.switch(&selected)?;
                        message_history.clear();
                        println!(
                            "✅ Switched to agent: {}",
                            agents.current().unwrap().display_name()
                        );
                    }
                }
            } else if agents.exists(args) {
                agents.switch(args)?;
                message_history.clear();
                println!(
                    "✅ Switched to agent: {}",
                    agents.current().unwrap().display_name()
                );
            } else {
                println!("❌ Agent not found: {}", args);
                println!("   Use /agents to see available agents");
            }
        }
        "agents" => {
            let settings = Settings::new(db);
            let user_mode = settings.user_mode();
            let agent_list = agents.list_filtered(user_mode);
            core::cmd_agents(
                &agent_list,
                &agents.current_name(),
                user_mode == UserMode::Developer,
            );
        }
        "mcp" => mcp::handle(mcp_manager, args).await,
        "set" => cmd_set(db, args)?,
        "yolo" => cmd_yolo(db)?,
        "version" | "v" => core::cmd_version(),
        "chatgpt-auth" => match crate::auth::run_chatgpt_auth(db).await {
            Ok(_) => {
                if let Err(e) = model_registry.reload_from_db(db) {
                    println!("⚠️  Failed to reload models: {}", e);
                }
                println!();
                println!("🔄 Please restart stockpot to use the new models.");
                println!("   Or type /exit and run spot again.");
            }
            Err(e) => println!("❌ ChatGPT auth failed: {}", e),
        },
        "claude-code-auth" => match crate::auth::run_claude_code_auth(db).await {
            Ok(_) => {
                if let Err(e) = model_registry.reload_from_db(db) {
                    println!("⚠️  Failed to reload models: {}", e);
                }
                println!();
                println!("🔄 Please restart stockpot to use the new models.");
                println!("   Or type /exit and run spot again.");
            }
            Err(e) => println!("❌ Claude Code auth failed: {}", e),
        },
        "add-model" | "add_model" => {
            match crate::cli::add_model::run_add_model(db).await {
                Ok(()) => {
                    // Reload registry to pick up the new model
                    if let Err(e) = model_registry.reload_from_db(db) {
                        println!("\x1b[33m⚠️  Failed to reload models: {}\x1b[0m", e);
                    }
                }
                Err(e) => println!("❌ Failed to add model: {}", e),
            }
        }
        "extra-models" | "extra_models" => {
            if let Err(e) = crate::cli::add_model::list_custom_models(db) {
                println!("❌ Failed to list models: {}", e);
            }
        }
        // Session commands
        "save" => {
            if let Some(name) = session::save(
                session_manager,
                agents,
                message_history,
                current_model,
                args,
            ) {
                *current_session = Some(name);
            }
        }
        "load" => {
            if let Some((name, data)) = session::load(session_manager, agents, args) {
                *message_history = data.messages;
                *current_session = Some(name);
            }
        }
        "sessions" => session::list(session_manager, current_session.as_deref()),
        "delete-session" => session::delete(session_manager, args),
        // Context commands
        "truncate" => context::truncate(message_history, args),
        "context" => {
            let ctx_len = model_registry
                .get(current_model)
                .map(|c| c.context_length)
                .unwrap_or(128000);
            session::cmd_context(
                db,
                message_history,
                current_session.as_deref(),
                &agents.current_name(),
                ctx_len,
            );
        }
        "compact" => session::cmd_compact(message_history, args),
        "session" | "s" => session::show_session(
            current_session.as_deref(),
            Settings::new(db)
                .get_bool("auto_save_session")
                .unwrap_or(true),
        ),
        "resume" => {
            if let Some((n, d)) = session::load_interactive(session_manager, agents) {
                *message_history = d.messages;
                *current_session = Some(n);
            }
        }
        // Model pin commands (persisted to database)
        "pin" => cmd_pin(db, agents, current_model, args),
        "unpin" => cmd_unpin(db, agents, current_model, args),
        "pins" => context::list_pins(db),
        // Core commands (extracted to commands/core.rs)
        "cd" => core::cmd_cd(args),
        "show" => {
            let agent_name = agents.current_name();
            let agent_display_name = agents
                .current()
                .map(|a| a.display_name().to_string())
                .unwrap_or_else(|| "None".to_string());
            core::cmd_show(
                db,
                &agent_name,
                &agent_display_name,
                current_model,
                current_session.as_deref(),
                message_history.len(),
                model_registry,
            );
        }
        "tools" => core::cmd_tools(),
        "model_settings" | "ms" => {
            if args.is_empty() {
                edit_model_settings(db, current_model, model_registry);
            } else if args == "--show" {
                show_model_settings(db, current_model);
            } else {
                // Edit settings for specific model
                edit_model_settings(db, args, model_registry);
            }
        }
        "reasoning" => core::cmd_reasoning(db, args),
        "verbosity" => core::cmd_verbosity(db, args),
        _ => {
            println!("❓ Unknown command: /{}", cmd);
            println!("   Type /help for available commands");
        }
    }
    Ok(CommandResult::Continue)
}

/// Show interactive command picker when user just types "/"
#[allow(clippy::too_many_arguments)]
async fn show_command_picker(
    db: &Database,
    agents: &mut AgentManager,
    model_registry: &mut ModelRegistry,
    message_bus: &MessageBus,
    message_history: &mut Vec<ModelRequest>,
    current_model: &mut String,
    current_session: &mut Option<String>,
    tool_registry: &SpotToolRegistry,
    mcp_manager: &McpManager,
    session_manager: &SessionManager,
) -> anyhow::Result<CommandResult> {
    if let Some(cmd) = crate::cli::completion_reedline::pick_command("") {
        Box::pin(handle_command(
            db,
            agents,
            model_registry,
            message_bus,
            message_history,
            current_model,
            current_session,
            tool_registry,
            mcp_manager,
            session_manager,
            &cmd,
        ))
        .await
    } else {
        Ok(CommandResult::Continue)
    }
}

/// Handle /set command.
fn cmd_set(db: &Database, args: &str) -> anyhow::Result<()> {
    let settings = Settings::new(db);
    if args.is_empty() {
        println!("\n⚙️  \x1b[1mSettings:\x1b[0m\n");
        for (key, value) in settings.list()? {
            println!("  {} = {}", key, value);
        }
        println!();
    } else if let Some((key, value)) = args.split_once('=') {
        let key = key.trim();
        let value = value.trim();

        match key {
            "user_mode" => match value.parse::<UserMode>() {
                Ok(mode) => {
                    settings.set_user_mode(mode)?;
                    println!("✅ Set user_mode = {}", mode);
                }
                Err(_) => {
                    println!("❌ Invalid user_mode: {}", value);
                    println!("   Valid: normal | expert | developer");
                }
            },
            _ => {
                settings.set(key, value)?;
                println!("✅ Set {} = {}", key, value);
            }
        }
    } else if args.trim() == "user_mode" {
        println!("user_mode = {}", settings.user_mode());
    } else if let Some(value) = settings.get(args)? {
        println!("{} = {}", args, value);
    } else {
        println!("❌ Setting not found: {}", args);
    }
    Ok(())
}

/// Handle /yolo command - toggle dangerous mode.
fn cmd_yolo(db: &Database) -> anyhow::Result<()> {
    let settings = Settings::new(db);
    let new_value = !settings.yolo_mode();
    settings.set("yolo_mode", if new_value { "true" } else { "false" })?;
    if new_value {
        println!("🔥 YOLO mode \x1b[1;31mENABLED\x1b[0m - Commands will run without confirmation!");
    } else {
        println!("🛡️  YOLO mode \x1b[1;32mDISABLED\x1b[0m - Commands will ask for confirmation");
    }
    Ok(())
}

/// Handle /pin command with flexible argument parsing.
/// Supports: `/pin <model>` or `/pin <agent> <model>`
fn cmd_pin(db: &Database, agents: &AgentManager, current_model: &mut String, args: &str) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let current_agent = agents.current_name();

    match parts.len() {
        0 => {
            // No args - show usage
            println!("❌ Please specify a model: /pin <model>");
            println!("   Or: /pin <agent> <model>");
            println!("   Example: /pin gpt-4o");
            println!("   Example: /pin reviewer gpt-4o");
        }
        1 => {
            // Single arg - pin model to current agent
            let model = parts[0];
            context::pin_model(
                db,
                current_model,
                &current_agent,
                model,
                true, // is current agent
            );
        }
        _ => {
            // Two+ args - first could be agent name, rest is model
            let first_arg = parts[0];

            // Check if first arg is a valid agent name
            if agents.exists(first_arg) {
                // Pin to specific agent
                let model = parts[1..].join(" ");
                let is_current = first_arg == current_agent;
                context::pin_model(db, current_model, first_arg, &model, is_current);
            } else {
                // Assume entire args is a model name (might have spaces?)
                // Fall back to pinning to current agent
                context::pin_model(db, current_model, &current_agent, args, true);
            }
        }
    }
}

/// Handle /unpin command with optional agent argument.
/// Supports: `/unpin` or `/unpin <agent>`
fn cmd_unpin(db: &Database, agents: &AgentManager, current_model: &mut String, args: &str) {
    let current_agent = agents.current_name();

    if args.is_empty() {
        // No args - unpin current agent
        context::unpin_model(
            db,
            current_model,
            &current_agent,
            true, // is current agent
        );
    } else {
        // Unpin specific agent
        let target_agent = args.trim();

        if !agents.exists(target_agent) {
            println!("❌ Unknown agent: {}", target_agent);
            println!("   Use /agents to see available agents");
            return;
        }

        let is_current = target_agent == current_agent;
        context::unpin_model(db, current_model, target_agent, is_current);
    }
}
