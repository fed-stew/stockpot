//! GUI module for Stockpot
//!
//! Provides a GPUI-based graphical interface for the stockpot agent framework.

mod app;
pub mod components;
pub mod image_processing;
pub mod pdf_processing;
pub mod render_test;
pub mod render_test_app;
pub mod state;
mod theme;

pub use app::{register_keybindings, ChatApp};
pub use theme::Theme;
