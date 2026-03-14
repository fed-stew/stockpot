//! Configuration management.

mod compression;
pub mod keys;
pub mod logging;
mod settings;
pub mod typed_config;
mod vdi;

pub use settings::{PdfMode, Settings};
pub use typed_config::{project_config_path, user_config_path};
pub use typed_config::{CompressionConfig, ConfigValidationError, SpotConfig, VdiConfig};
pub use vdi::{detect_vdi_environment, is_vdi_mode_active};
