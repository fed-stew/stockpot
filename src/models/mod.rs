//! Model configuration and registry.
//!
//! This module handles:
//! - Loading model configurations from JSON files
//! - Per-model settings (temperature, reasoning_effort, etc.)
//! - Model type definitions
//! - Default model configurations

pub mod defaults;
pub mod model_config;
pub mod registry;
pub mod settings;
pub mod types;
pub mod utils;

// Re-export main types for convenience
pub use model_config::ModelConfig;
pub use registry::ModelRegistry;
pub use types::{CustomEndpoint, ModelConfigError, ModelType};
pub use utils::{has_api_key, has_oauth_tokens, resolve_api_key, resolve_env_var};

// Re-exports from other submodules
pub use defaults::{featured_models, is_featured};
pub use settings::ModelSettings;
