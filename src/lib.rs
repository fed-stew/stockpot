//! Spot - Precision computer control system
//!
//! This crate re-exports the core library and provides the main `spot` binary.

pub use spot_core as core;

#[cfg(feature = "tui")]
pub use spot_tui as tui;

#[cfg(feature = "gui")]
pub use spot_gui as gui;
