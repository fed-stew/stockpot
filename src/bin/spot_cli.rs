//! Spot CLI - TUI-only binary entry point
//!
//! This binary always launches the terminal user interface.
//! Use this when you explicitly want the TUI, or in environments without a display.

use clap::Parser;
use stockpot::runner::{run_tui, AppConfig};

/// Stockpot CLI - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot-cli")]
#[command(
    version,
    about = "Stockpot CLI - Always launches the terminal interface"
)]
#[command(propagate_version = true)]
struct Args {
    /// Change to this directory before running
    #[arg(short = 'C', long, visible_alias = "directory")]
    cwd: Option<String>,

    /// Enable debug logging
    #[arg(short = 'd', long)]
    debug: bool,

    /// Enable verbose (trace-level) logging
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Skip the automatic update check on startup
    #[arg(long)]
    skip_update_check: bool,
}

impl From<&Args> for AppConfig {
    fn from(args: &Args) -> Self {
        AppConfig {
            debug: args.debug,
            verbose: args.verbose,
            skip_update_check: args.skip_update_check,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Change directory if requested
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    run_tui(AppConfig::from(&args))
}
