//! Spot GUI - GUI-only binary entry point
//!
//! This binary always launches the graphical interface.
//! Use this when you explicitly want the GUI, regardless of environment detection.

use clap::Parser;
use stockpot::runner::{run_gui, run_render_test, AppConfig};

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

    if args.render_test {
        run_render_test()
    } else {
        run_gui(AppConfig::from(&args))
    }
}
