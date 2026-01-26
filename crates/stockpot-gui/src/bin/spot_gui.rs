//! Spot GUI - GUI-only binary entry point
//!
//! This binary always launches the graphical interface.
//! Use this when you explicitly want the GUI, regardless of environment detection.

use anyhow::Result;
use clap::Parser;
use stockpot_core::runner::AppConfig;

/// Stockpot GUI - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot-gui")]
#[command(
    version,
    about = "Stockpot GUI - Always launches the graphical interface"
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

    /// Run the render performance test
    #[arg(long)]
    render_test: bool,

    /// Skip the automatic update check on startup
    #[arg(long)]
    skip_update_check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Change directory if requested
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    if args.render_test {
        stockpot_gui::gui::run_render_test()
    } else {
        let config = AppConfig {
            debug: args.debug,
            verbose: args.verbose,
            skip_update_check: args.skip_update_check,
        };
        stockpot_gui::gui::run_gui(config)
    }
}
