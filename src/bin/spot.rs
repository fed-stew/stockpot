//! Spot - Smart binary entry point
//!
//! This binary automatically detects whether a graphical display is available
//! and launches the appropriate interface (GUI or TUI).
//!
//! Use `--tui` flag to force TUI mode regardless of display availability.

use clap::Parser;
use stockpot::display_detect::has_display;
use stockpot::runner::{run_gui, run_render_test, run_tui, AppConfig};

/// Stockpot - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot")]
#[command(version, about, long_about = None)]
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

    /// Force TUI mode (terminal interface)
    #[arg(long)]
    tui: bool,

    /// Run the render performance test (GUI only)
    #[arg(long)]
    render_test: bool,

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

    let config = AppConfig::from(&args);

    // Route to appropriate mode
    if args.render_test {
        run_render_test()
    } else if args.tui {
        // Explicit TUI flag
        run_tui(config)
    } else if has_display() {
        // Display available, use GUI
        run_gui(config)
    } else {
        // No display, fall back to TUI
        run_tui(config)
    }
}
