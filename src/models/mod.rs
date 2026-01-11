//! Model configuration and registry.
//!
//! This module handles:
//! - Loading model configurations from JSON files
//! - Per-model settings (temperature, reasoning_effort, etc.)
//! - Model type definitions
//! - Default model configurations
//! - Model catalog from models.dev API

pub mod catalog;
pub mod defaults;
pub mod model_config;
pub mod registry;
pub mod settings;
pub mod types;
pub mod utils;

// Re-export main types for convenience
pub use model_config::ModelConfig;
pub use registry::ModelRegistry;
pub use types::{CustomEndpoint, ModelType};
pub use utils::resolve_api_key;

// Re-exports from other submodules
