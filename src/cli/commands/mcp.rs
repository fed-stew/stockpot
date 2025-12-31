//! MCP command handlers with interactive management.

use crate::mcp::{McpConfig, McpManager, McpServerEntry};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

/// Handle MCP subcommands.
pub async fn handle(manager: &McpManager, args: &str) {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let subcommand = parts.first().copied().unwrap_or("");
    let subargs = parts.get(1).copied().unwrap_or("");

    match subcommand {
        "" | "help" => show_help(),
        "list" | "ls" => list_servers(manager),
        "status" => show_status(manager).await,
        "start" => start_server(manager, subargs).await,
        "stop" => stop_server(manager, subargs).await,
        "restart" => restart_server(manager, subargs).await,
        "start-all" => start_all(manager).await,
        "stop-all" => stop_all(manager).await,
        "tools" => list_tools(manager).await,
        "add" => add_server_interactive(),
        "remove" | "rm" => remove_server(subargs),
        "enable" => toggle_server(subargs, true),
        "disable" => toggle_server(subargs, false),
        _ => println!("Unknown MCP command: {}. Try /mcp help", subcommand),
    }
}

fn show_help() {
    println!(
        "
\x1b[1m🔌 MCP Server Management\x1b[0m

  \x1b[36m/mcp list\x1b[0m              List configured servers
  \x1b[36m/mcp status\x1b[0m            Show running servers with tool counts
  \x1b[36m/mcp tools\x1b[0m             List tools from running servers
  \x1b[36m/mcp start [name]\x1b[0m      Start a server (interactive if no name)
  \x1b[36m/mcp stop [name]\x1b[0m       Stop a server (interactive if no name)
  \x1b[36m/mcp restart [name]\x1b[0m    Restart a server
  \x1b[36m/mcp start-all\x1b[0m         Start all enabled servers
  \x1b[36m/mcp stop-all\x1b[0m          Stop all servers
  \x1b[36m/mcp add\x1b[0m               Add new server (interactive wizard)
  \x1b[36m/mcp remove [name]\x1b[0m     Remove a server
  \x1b[36m/mcp enable <name>\x1b[0m     Enable a server
  \x1b[36m/mcp disable <name>\x1b[0m    Disable a server

\x1b[2mConfig: ~/.stockpot/mcp_servers.json\x1b[0m
"
    );
}

fn list_servers(manager: &McpManager) {
    let config = manager.config();

    if config.servers.is_empty() {
        println!("\n  No MCP servers configured.");
        println!("  Use \x1b[36m/mcp add\x1b[0m to add one.\n");
        return;
    }

    println!("\n\x1b[1m📋 Configured MCP Servers\x1b[0m\n");

    for (name, entry) in &config.servers {
        let status = if entry.enabled { "✓" } else { "○" };
        let status_color = if entry.enabled { "32" } else { "90" };

        println!("  \x1b[{}m{}\x1b[0m \x1b[1;36m{}\x1b[0m", status_color, status, name);
        println!("    \x1b[2m{} {}\x1b[0m", entry.command, entry.args.join(" "));

        if let Some(ref desc) = entry.description {
            println!("    \x1b[2;3m{}\x1b[0m", desc);
        }

        if !entry.env.is_empty() {
            let env_keys: Vec<_> = entry.env.keys().collect();
            println!("    \x1b[2menv: {}\x1b[0m", env_keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
    }
    println!();
}

async fn show_status(manager: &McpManager) {
    let running = manager.running_servers().await;
    let config = manager.config();

    println!("\n\x1b[1m📊 MCP Server Status\x1b[0m\n");

    if running.is_empty() {
        println!("  No servers running.");
        let enabled: Vec<_> = config.enabled_servers().collect();
        if !enabled.is_empty() {
            let names: Vec<_> = enabled.iter().map(|(n, _)| n.as_str()).collect();
            println!("  \x1b[2mEnabled servers: {}\x1b[0m", names.join(", "));
            println!("  \x1b[2mUse /mcp start-all to start them\x1b[0m");
        }
        println!();
        return;
    }

    // Get all tools for counts
    let tools = manager.list_all_tools().await;

    for name in &running {
        print!("  \x1b[32m●\x1b[0m \x1b[1m{}\x1b[0m", name);
        if let Some(server_tools) = tools.get(name.as_str()) {
            print!(" ({} tools)", server_tools.len());
        }
        println!();
    }

    // Show stopped but enabled
    let stopped: Vec<_> = config
        .enabled_servers()
        .filter(|(n, _)| !running.contains(&n.to_string()))
        .collect();

    if !stopped.is_empty() {
        println!();
        for (name, _) in stopped {
            println!("  \x1b[90m○\x1b[0m \x1b[2m{}\x1b[0m (stopped)", name);
        }
    }
    println!();
}

async fn start_server(manager: &McpManager, name: &str) {
    if name.is_empty() {
        // Interactive selection
        let config = manager.config();
        let servers: Vec<_> = config.servers.keys().cloned().collect();

        if servers.is_empty() {
            println!("No servers configured. Use /mcp add first.");
            return;
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select server to start")
            .items(&servers)
            .interact_opt();

        match selection {
            Ok(Some(idx)) => do_start(manager, &servers[idx]).await,
            _ => println!("Cancelled."),
        }
    } else {
        do_start(manager, name).await;
    }
}

async fn do_start(manager: &McpManager, name: &str) {
    match manager.start_server(name).await {
        Ok(()) => println!("✅ Started: {}", name),
        Err(e) => println!("❌ Failed to start {}: {}", name, e),
    }
}

async fn stop_server(manager: &McpManager, name: &str) {
    if name.is_empty() {
        let running = manager.running_servers().await;

        if running.is_empty() {
            println!("No servers running.");
            return;
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select server to stop")
            .items(&running)
            .interact_opt();

        match selection {
            Ok(Some(idx)) => do_stop(manager, &running[idx]).await,
            _ => println!("Cancelled."),
        }
    } else {
        do_stop(manager, name).await;
    }
}

async fn do_stop(manager: &McpManager, name: &str) {
    match manager.stop_server(name).await {
        Ok(()) => println!("⏹️  Stopped: {}", name),
        Err(e) => println!("❌ Failed to stop {}: {}", name, e),
    }
}

async fn restart_server(manager: &McpManager, name: &str) {
    if name.is_empty() {
        let running = manager.running_servers().await;
        if running.is_empty() {
            println!("No servers running to restart.");
            return;
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select server to restart")
            .items(&running)
            .interact_opt();

        match selection {
            Ok(Some(idx)) => do_restart(manager, &running[idx]).await,
            _ => println!("Cancelled."),
        }
    } else {
        do_restart(manager, name).await;
    }
}

async fn do_restart(manager: &McpManager, name: &str) {
    println!("🔄 Restarting {}...", name);
    let _ = manager.stop_server(name).await;
    match manager.start_server(name).await {
        Ok(()) => println!("✅ Restarted: {}", name),
        Err(e) => println!("❌ Failed to restart {}: {}", name, e),
    }
}

async fn start_all(manager: &McpManager) {
    let enabled: Vec<_> = manager.config().enabled_servers().map(|(n, _)| n.clone()).collect();

    if enabled.is_empty() {
        println!("No enabled servers to start.");
        return;
    }

    println!("🔌 Starting {} server(s)...", enabled.len());

    match manager.start_all().await {
        Ok(()) => {
            let running = manager.running_servers().await;
            println!("✅ Running: {}", running.join(", "));
        }
        Err(e) => println!("⚠️  Some servers failed: {}", e),
    }
}

async fn stop_all(manager: &McpManager) {
    let running = manager.running_servers().await;

    if running.is_empty() {
        println!("No servers running.");
        return;
    }

    println!("⏹️  Stopping {} server(s)...", running.len());

    match manager.stop_all().await {
        Ok(()) => println!("✅ All servers stopped"),
        Err(e) => println!("⚠️  Error stopping servers: {}", e),
    }
}

async fn list_tools(manager: &McpManager) {
    let running = manager.running_servers().await;

    if running.is_empty() {
        println!("\n  No MCP servers running.");
        println!("  Use /mcp start to start servers.\n");
        return;
    }

    println!("\n\x1b[1m🔧 MCP Tools\x1b[0m\n");

    let all_tools = manager.list_all_tools().await;

    for (server_name, tools) in all_tools {
        println!("  \x1b[1;36m{}\x1b[0m ({} tools):", server_name, tools.len());
        for tool in tools {
            let desc = tool.description.as_deref().unwrap_or("");
            println!("    • \x1b[1m{}\x1b[0m", tool.name);
            if !desc.is_empty() {
                let short = if desc.len() > 60 { format!("{}...", &desc[..57]) } else { desc.to_string() };
                println!("      \x1b[2m{}\x1b[0m", short);
            }
        }
        println!();
    }
}

fn add_server_interactive() {
    println!("\n\x1b[1m➕ Add MCP Server\x1b[0m\n");

    // Server name
    let name: String = match Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("Server name (e.g., 'github', 'filesystem')")
        .interact_text()
    {
        Ok(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            println!("Cancelled.");
            return;
        }
    };

    // Check if exists
    let config = McpConfig::load_or_default();
    if config.has_server(&name) {
        println!("❌ Server '{}' already exists. Use /mcp remove first.", name);
        return;
    }

    // Command
    let command: String = match Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("Command (e.g., 'npx', 'uvx', 'python')")
        .interact_text()
    {
        Ok(c) if !c.trim().is_empty() => c.trim().to_string(),
        _ => {
            println!("Cancelled.");
            return;
        }
    };

    // Arguments
    let args_str: String = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("Arguments (space-separated)")
        .default(String::new())
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    let args: Vec<String> = if args_str.trim().is_empty() {
        vec![]
    } else {
        args_str.split_whitespace().map(String::from).collect()
    };

    // Description
    let description: String = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("Description (optional)")
        .default(String::new())
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    // Environment variables
    let mut env = std::collections::HashMap::new();
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Add environment variables?")
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        loop {
            let key: String = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt("Env var name (empty to finish)")
                .default(String::new())
                .allow_empty(true)
                .interact_text()
                .unwrap_or_default();

            if key.trim().is_empty() {
                break;
            }

            let value: String = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Value for {} (use $VAR for env ref)", key))
                .interact_text()
                .unwrap_or_default();

            env.insert(key.trim().to_string(), value);
        }
    }

    // Build entry
    let mut entry = McpServerEntry::new(command).with_args(args);
    if !description.trim().is_empty() {
        entry = entry.with_description(description.trim());
    }
    entry.env = env;

    // Save
    let mut config = McpConfig::load_or_default();
    config.add_server(&name, entry);

    match config.save_default() {
        Ok(()) => {
            println!("\n✅ Added server: \x1b[1m{}\x1b[0m", name);
            println!("   Use \x1b[36m/mcp start {}\x1b[0m to start it\n", name);
        }
        Err(e) => println!("❌ Failed to save config: {}", e),
    }
}

fn remove_server(name: &str) {
    if name.is_empty() {
        let config = McpConfig::load_or_default();
        let servers: Vec<_> = config.servers.keys().cloned().collect();

        if servers.is_empty() {
            println!("No servers configured.");
            return;
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select server to remove")
            .items(&servers)
            .interact_opt();

        match selection {
            Ok(Some(idx)) => do_remove_server(&servers[idx]),
            _ => println!("Cancelled."),
        }
    } else {
        do_remove_server(name);
    }
}

fn do_remove_server(name: &str) {
    let mut config = McpConfig::load_or_default();

    if config.remove_server(name).is_some() {
        match config.save_default() {
            Ok(()) => println!("🗑️  Removed server: {}", name),
            Err(e) => println!("❌ Failed to save: {}", e),
        }
    } else {
        println!("Server not found: {}", name);
    }
}

fn toggle_server(name: &str, enable: bool) {
    let action = if enable { "enable" } else { "disable" };

    if name.is_empty() {
        println!("Usage: /mcp {} <name>", action);
        return;
    }

    let mut config = McpConfig::load_or_default();

    if let Some(entry) = config.servers.get_mut(name) {
        entry.enabled = enable;
        match config.save_default() {
            Ok(()) => {
                let status = if enable { "✓ Enabled" } else { "○ Disabled" };
                println!("{} server: {}", status, name);
            }
            Err(e) => println!("❌ Failed to save: {}", e),
        }
    } else {
        println!("Server not found: {}", name);
    }
}
