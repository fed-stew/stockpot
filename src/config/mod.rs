//! Configuration management.

mod settings;
mod xdg;

pub use settings::{Settings, SettingsError};
pub use xdg::XdgDirs;
