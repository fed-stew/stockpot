//! Model catalog from models.dev API.
//!
//! This module provides types and functions for loading the build-time
//! bundled model catalog from models.dev/api.json.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// Models catalog downloaded at build time from https://models.dev/api.json
/// See build.rs for the download logic, caching, and fallback behavior.
const BUNDLED_MODELS_CATALOG_JSON: &str =
    include_str!(concat!(env!("OUT_DIR"), "/models_catalog.json"));

/// Provider information from the bundled catalog (models.dev/api.json).
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct ProviderInfo {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, ModelInfo>,
}

/// Model information from the bundled catalog (models.dev/api.json).
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub context_length: Option<u64>,
    #[serde(default)]
    pub input_price: Option<f64>,
    #[serde(default)]
    pub output_price: Option<f64>,
}

/// Load all providers from the build-time bundled catalog.
///
/// The catalog is downloaded from https://models.dev/api.json at build time.
/// To force a refresh, run: FORCE_CATALOG_REFRESH=1 cargo build
pub async fn fetch_providers() -> Result<HashMap<String, ProviderInfo>> {
    tracing::debug!("Loading providers from bundled catalog...");

    let providers: HashMap<String, ProviderInfo> =
        serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON)
            .map_err(|e| anyhow!("Failed to parse bundled catalog: {}", e))?;

    Ok(providers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_catalog_parses() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();
        assert!(!providers.is_empty(), "Catalog should have providers");
    }

    #[test]
    fn test_provider_info_deserialize() {
        let json = r#"{"id": "test", "name": "Test Provider", "env": ["API_KEY"]}"#;
        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "test");
        assert_eq!(provider.name, "Test Provider");
        assert_eq!(provider.env, vec!["API_KEY"]);
    }

    #[test]
    fn test_model_info_deserialize() {
        let json = r#"{"id": "gpt-4", "name": "GPT-4", "context_length": 128000}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "gpt-4");
        assert_eq!(model.name, Some("GPT-4".to_string()));
        assert_eq!(model.context_length, Some(128000));
    }

    #[test]
    fn test_model_info_optional_fields() {
        let json = r#"{"id": "minimal"}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "minimal");
        assert!(model.name.is_none());
        assert!(model.context_length.is_none());
        assert!(model.input_price.is_none());
        assert!(model.output_price.is_none());
    }
}
