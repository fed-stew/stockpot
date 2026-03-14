//! Spot Core Library
//!
//! Core functionality for the Spot AI coding assistant.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug flag for stream event logging
pub static DEBUG_STREAM_EVENTS: AtomicBool = AtomicBool::new(false);

/// Enable debug stream event logging
pub fn enable_debug_stream_events() {
    DEBUG_STREAM_EVENTS.store(true, Ordering::SeqCst);
}

// Re-export extracted crates under their original module names
pub use spot_auth as auth;
pub use spot_models as models;
pub use spot_storage as db;
pub use spot_tools::terminal;

// Modules that remain in spot-core
pub mod agents;
pub mod config;
pub mod display_detect;
pub mod mcp;
pub mod messaging;
pub mod metrics;
pub mod plugins;
pub mod runner;
pub mod session;
pub mod tokens;
pub mod tools;
pub mod version_check;

#[cfg(test)]
mod test_utils;
