//! Stockpot - AI-powered coding assistant CLI
//!
//! This is the main entry point for the Stockpot CLI application.

#![allow(dead_code)]
#![allow(unused_imports)]

mod agents;
mod auth;
mod cli;
mod config;
mod db;
mod mcp;
mod messaging;
mod models;
mod session;
mod tokens;
mod tools;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Stockpot - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    /// Run in interactive mode
    #[arg(short, long)]
    interactive: bool,

    /// Execute a single prompt and exit
    #[arg(short, long)]
    prompt: Option<String>,

    /// Specify which agent to use
    #[arg(short, long)]
    agent: Option<String>,

    /// Specify which model to use
    #[arg(short, long)]
    model: Option<String>,

    /// Run in bridge mode for external UI
    #[arg(long)]
    bridge: bool,

    /// Working directory (like git -C)
    #[arg(short = 'C', long, visible_alias = "directory")]
    cwd: Option<String>,

    /// Enable debug logging (equivalent to RUST_LOG=debug)
    #[arg(short = 'd', long)]
    debug: bool,

    /// Enable verbose logging (equivalent to RUST_LOG=trace)
    #[arg(short = 'v', long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse args first to check debug flag
    let args = Args::parse();

    // Determine log level from args or env
    let default_filter = if args.verbose {
        "trace"
    } else if args.debug {
        "debug"
    } else {
        "warn" // Quiet by default for normal use
    };

    // Initialize tracing with stderr output (so it doesn't interfere with REPL)
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_writer(std::io::stderr),
        )
        .init();

    if args.debug || args.verbose {
        tracing::info!("Debug logging enabled");
    }

    // Change working directory if specified
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    // Initialize database
    let db = db::Database::open()?;
    db.migrate()?;

    // Handle different modes
    if args.bridge {
        cli::bridge::run_bridge_mode().await?;
    } else if let Some(prompt) = args.prompt {
        cli::runner::run_single_prompt(&db, &prompt, args.agent.as_deref(), args.model.as_deref())
            .await?;
    } else {
        cli::runner::run_interactive(&db, args.agent.as_deref(), args.model.as_deref()).await?;
    }

    Ok(())
}
