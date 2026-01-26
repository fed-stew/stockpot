//! TUI Application Runner
//!
//! Provides the entry point for launching the TUI application.

use anyhow::Result;
use std::fs::File;
use stockpot_core::runner::AppConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use super::TuiApp;

/// Run the TUI application.
///
/// # Errors
///
/// Returns an error if the TUI fails to start.
pub fn run_tui(config: AppConfig) -> Result<()> {
    // Set up file logging for TUI debugging
    let log_file = File::create("/tmp/stockpot-tui.log").expect("Failed to create log file");
    let default_filter = if config.verbose {
        "trace"
    } else if config.debug {
        "debug"
    } else {
        "info,stockpot=debug"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(log_file)),
        )
        .init();

    // Enable debug stream event logging if --debug flag is set
    if config.debug {
        stockpot_core::enable_debug_stream_events();
    }

    // Use LocalSet to allow spawn_local for non-Send futures (Database uses RefCell)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async {
        let mut app = TuiApp::new().await?;
        app.run().await
    })
}
