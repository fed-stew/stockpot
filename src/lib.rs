//! Stockpot - AI-powered coding assistant
//!
//! This crate re-exports the core library and provides the main `spot` binary.

pub use stockpot_core as core;

#[cfg(feature = "tui")]
pub use stockpot_tui as tui;

#[cfg(feature = "gui")]
pub use stockpot_gui as gui;
