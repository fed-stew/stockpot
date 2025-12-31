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

pub use config::{CustomEndpoint, ModelConfig, ModelRegistry, ModelType, resolve_env_var};
pub use defaults::{default_models, default_models_json};
pub use settings::ModelSettings;
