//! Stockpot CLI - Auto-detecting GUI/TUI launcher
//!
//! Automatically selects GUI or TUI based on environment and available features.

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "spot")]
#[command(about = "Stockpot AI Coding Assistant")]
#[command(version)]
struct Args {
    /// Change to this directory before running
    #[arg(short = 'C', long, visible_alias = "directory")]
    cwd: Option<String>,

    /// Force TUI mode even if GUI is available
    #[arg(long)]
    tui: bool,

    /// Force GUI mode
    #[arg(long)]
    gui: bool,

    /// Enable debug logging
    #[arg(short = 'd', long)]
    debug: bool,

    /// Enable verbose (trace-level) logging
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Skip the automatic update check on startup
    #[arg(long)]
    skip_update_check: bool,

    /// Run the render performance test (GUI only)
    #[arg(long)]
    render_test: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Change directory if requested
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    // Build AppConfig from args
    let config = stockpot_core::runner::AppConfig {
        debug: args.debug,
        verbose: args.verbose,
        skip_update_check: args.skip_update_check,
    };

    // Handle render test specially (GUI only)
    if args.render_test {
        #[cfg(feature = "gui")]
        {
            return stockpot_gui::gui::run_render_test();
        }
        #[cfg(not(feature = "gui"))]
        {
            anyhow::bail!("Render test requires GUI feature. Rebuild with --features gui");
        }
    }

    // Determine which mode to run
    let use_gui = if args.tui {
        false
    } else if args.gui {
        true
    } else {
        // Auto-detect: prefer GUI if available and we have a display
        #[cfg(feature = "gui")]
        {
            stockpot_core::display_detect::has_display()
        }
        #[cfg(not(feature = "gui"))]
        {
            false
        }
    };

    if use_gui {
        #[cfg(feature = "gui")]
        {
            stockpot_gui::gui::run_gui(config)
        }
        #[cfg(not(feature = "gui"))]
        {
            anyhow::bail!("GUI feature not enabled. Rebuild with --features gui")
        }
    } else {
        #[cfg(feature = "tui")]
        {
            stockpot_tui::tui::run_tui(config)
        }
        #[cfg(not(feature = "tui"))]
        {
            anyhow::bail!("TUI feature not enabled. Rebuild with --features tui")
        }
    }
}
