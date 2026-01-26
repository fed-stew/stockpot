//! Stockpot Core Library
//!
//! Core functionality for the Stockpot AI coding assistant.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug flag for stream event logging
pub static DEBUG_STREAM_EVENTS: AtomicBool = AtomicBool::new(false);

/// Enable debug stream event logging
pub fn enable_debug_stream_events() {
    DEBUG_STREAM_EVENTS.store(true, Ordering::SeqCst);
}

pub mod agents;
pub mod auth;
pub mod config;
pub mod db;
pub mod display_detect;
pub mod mcp;
pub mod messaging;
pub mod models;
pub mod runner;
pub mod session;
pub mod terminal;
pub mod tokens;
pub mod tools;
pub mod version_check;
