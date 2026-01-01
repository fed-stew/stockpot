//! GUI module for Stockpot
//!
//! Provides a GPUI-based graphical interface for the stockpot agent framework.

mod app;
mod theme;
mod zed_globals;
pub mod components;
pub mod state;

pub use app::{ChatApp, register_keybindings};
pub use theme::Theme;
pub use zed_globals::GlobalLanguageRegistry;
