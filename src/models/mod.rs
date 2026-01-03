//! Model configuration and registry.
//!
//! This module handles:
//! - Loading model configurations from JSON files
//! - Per-model settings (temperature, reasoning_effort, etc.)
//! - Model type definitions
//! - Default model configurations

pub mod config;
pub mod defaults;
pub mod settings;

pub use config::{
    has_api_key, resolve_api_key, resolve_env_var, CustomEndpoint, ModelConfig, ModelRegistry,
    ModelType,
};
pub use defaults::{featured_models, is_featured};
pub use settings::ModelSettings;
