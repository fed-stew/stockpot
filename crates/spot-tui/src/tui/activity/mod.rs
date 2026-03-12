//! Activity system for the TUI feed
//!
//! This module provides the core activity types that power the activity feed,
//! matching spot's visual model with spot-specific additions.

mod converter;
mod types;

pub use converter::*;
pub use types::*;
