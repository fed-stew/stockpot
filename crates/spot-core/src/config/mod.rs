//! Configuration management.

mod compression;
pub mod keys;
mod settings;
mod vdi;

pub use settings::{PdfMode, Settings};
pub use vdi::{detect_vdi_environment, is_vdi_mode_active};
