//! GUI module for Stockpot
//!
//! Provides a GPUI-based graphical interface for the stockpot agent framework.

mod app;
pub mod components;
pub mod state;
mod theme;
mod zed_globals;

pub use app::{register_keybindings, ChatApp};
pub use theme::Theme;
pub use zed_globals::GlobalLanguageRegistry;
