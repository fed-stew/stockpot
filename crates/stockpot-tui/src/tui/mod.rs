//! TUI (Terminal User Interface) mode for Stockpot
//!
//! Provides a full-featured terminal interface with mouse support,
//! feature parity with the GUI mode.

pub mod activity;
mod app;
mod event;
mod layout;
pub mod markdown;
mod run;
mod theme;
mod ui;

pub mod attachments;
pub mod execution;
pub mod hit_test;
pub mod selection;
pub mod settings;
pub mod state;
pub mod widgets;

pub use app::TuiApp;
pub use run::run_tui;

use anyhow::Result;

/// Run the TUI application (async version, for internal use)
pub async fn run() -> Result<()> {
    let mut app = TuiApp::new().await?;
    app.run().await
}
