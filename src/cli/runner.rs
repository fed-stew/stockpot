//! CLI runner for interactive and single-prompt modes.

use crate::cli::repl::Repl;
use crate::db::Database;

/// Run a single prompt and exit.
pub async fn run_single_prompt(
    db: &Database,
    prompt: &str,
    agent: Option<&str>,
    model: Option<&str>,
) -> anyhow::Result<()> {
    let mut repl = Repl::new(db);
    
    if let Some(agent_name) = agent {
        repl = repl.with_agent(agent_name);
    }
    if let Some(model_name) = model {
        repl = repl.with_model(model_name);
    }

    // Handle the prompt directly
    repl.handle_prompt(prompt).await?;
    
    Ok(())
}

/// Run in interactive mode.
pub async fn run_interactive(
    db: &Database,
    agent: Option<&str>,
    model: Option<&str>,
) -> anyhow::Result<()> {
    // Print welcome banner
    print_banner();

    let mut repl = Repl::new(db);
    
    if let Some(agent_name) = agent {
        repl = repl.with_agent(agent_name);
    }
    if let Some(model_name) = model {
        repl = repl.with_model(model_name);
    }

    // Run the REPL
    repl.run().await?;

    Ok(())
}

fn print_banner() {
    println!();
    println!("  \x1b[1;33m╔═╗\x1b[2;36mᵗᵒᶜᵏ\x1b[1;33m╔═╗╔═╗╔╦╗\x1b[0m");
    println!("  \x1b[1;33m╚═╗    ╠═╝║ ║ ║ \x1b[0m");
    println!("  \x1b[1;33m╚═╝    ╩  ╚═╝ ╩ \x1b[0m  \x1b[2mv{}\x1b[0m", env!("CARGO_PKG_VERSION"));
    println!();
    println!("  \x1b[2m🍲 AI-powered coding assistant\x1b[0m");
    println!("  \x1b[2mType \x1b[0m\x1b[1;36m/help\x1b[0m\x1b[2m for commands, or start chatting!\x1b[0m");
    println!();
}
