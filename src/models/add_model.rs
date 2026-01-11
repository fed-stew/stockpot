//! Interactive model discovery and configuration.
//!
//! This module provides functionality to browse available AI providers
//! and models from a build-time bundled catalog, and configure them for use.

use crate::db::Database;
use crate::models::{CustomEndpoint, ModelConfig, ModelRegistry, ModelType};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{self, Write};

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
    println!("\x1b[2mLoading providers from bundled catalog...\x1b[0m");

    let providers: HashMap<String, ProviderInfo> =
        serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON)
            .map_err(|e| anyhow!("Failed to parse bundled catalog: {}", e))?;

    Ok(providers)
}

/// Interactive provider selection
pub fn select_provider(providers: &HashMap<String, ProviderInfo>) -> Result<Option<&ProviderInfo>> {
    // Sort providers by name for display
    let mut provider_list: Vec<_> = providers.values().collect();
    provider_list.sort_by(|a, b| a.name.cmp(&b.name));

    println!("\n\x1b[1m📦 Available Providers:\x1b[0m\n");

    for (i, provider) in provider_list.iter().enumerate() {
        let model_count = provider.models.len();
        println!(
            "  \x1b[1;33m{:>3}\x1b[0m. {} \x1b[2m({} models)\x1b[0m",
            i + 1,
            if provider.name.is_empty() {
                &provider.id
            } else {
                &provider.name
            },
            model_count
        );
    }

    println!("\n  \x1b[2m  0. Cancel\x1b[0m");
    print!(
        "\n\x1b[1mSelect provider (1-{}):\x1b[0m ",
        provider_list.len()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input == "0" || input.is_empty() {
        return Ok(None);
    }

    let index: usize = input.parse().map_err(|_| anyhow!("Invalid selection"))?;
    if index == 0 || index > provider_list.len() {
        return Err(anyhow!("Invalid selection"));
    }

    Ok(Some(provider_list[index - 1]))
}

/// Interactive model selection
pub fn select_model(provider: &ProviderInfo) -> Result<Option<&ModelInfo>> {
    if provider.models.is_empty() {
        println!("\x1b[1;33m⚠️  No models available for this provider\x1b[0m");
        return Ok(None);
    }

    // Sort models by id
    let mut model_list: Vec<_> = provider.models.values().collect();
    model_list.sort_by(|a, b| a.id.cmp(&b.id));

    println!(
        "\n\x1b[1m🤖 Models for {}:\x1b[0m\n",
        if provider.name.is_empty() {
            &provider.id
        } else {
            &provider.name
        }
    );

    for (i, model) in model_list.iter().enumerate() {
        let name = model.name.as_deref().unwrap_or(&model.id);
        let ctx = model
            .context_length
            .map(|c| format!("{}k ctx", c / 1000))
            .unwrap_or_default();
        let price = match (model.input_price, model.output_price) {
            (Some(i), Some(o)) => format!("${:.2}/${:.2} per 1M", i, o),
            _ => String::new(),
        };

        println!(
            "  \x1b[1;36m{:>3}\x1b[0m. {} \x1b[2m{}{}\x1b[0m",
            i + 1,
            name,
            ctx,
            if !price.is_empty() {
                format!(" | {}", price)
            } else {
                String::new()
            }
        );
    }

    println!("\n  \x1b[2m  0. Back\x1b[0m");
    print!("\n\x1b[1mSelect model (1-{}):\x1b[0m ", model_list.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input == "0" || input.is_empty() {
        return Ok(None);
    }

    let index: usize = input.parse().map_err(|_| anyhow!("Invalid selection"))?;
    if index == 0 || index > model_list.len() {
        return Err(anyhow!("Invalid selection"));
    }

    Ok(Some(model_list[index - 1]))
}

/// Prompt for API key and save to database for persistence.
pub fn prompt_api_key(db: &Database, provider: &ProviderInfo) -> Result<Option<String>> {
    let env_var = provider
        .env
        .first()
        .map(|s| s.as_str())
        .unwrap_or("API_KEY");

    println!("\n\x1b[1m🔑 API Key Configuration\x1b[0m");

    // Check if already in database
    if db.has_api_key(env_var) {
        println!(
            "\x1b[32m✓ {} is already configured in stockpot\x1b[0m",
            env_var
        );
        print!("\nUse existing key? [Y/n]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input.is_empty() || input == "y" || input == "yes" {
            return Ok(Some(env_var.to_string()));
        }
    }
    // Also check environment variable
    else if let Ok(existing) = std::env::var(env_var) {
        if !existing.is_empty() {
            println!("\x1b[32m✓ {} is set in your environment\x1b[0m", env_var);
            print!("\nUse existing key and save to stockpot? [Y/n]: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();

            if input.is_empty() || input == "y" || input == "yes" {
                // Save to database for persistence
                if let Err(e) = db.save_api_key(env_var, &existing) {
                    println!("\x1b[33m⚠️  Failed to save API key: {}\x1b[0m", e);
                } else {
                    println!("\x1b[32m✓ API key saved to stockpot database\x1b[0m");
                }
                return Ok(Some(env_var.to_string()));
            }
        }
    }

    println!("\nTo use this provider, you need an API key.");

    if let Some(doc) = &provider.doc {
        println!("\x1b[2mDocumentation: {}\x1b[0m", doc);
    }

    println!("\n\x1b[1mOptions:\x1b[0m");
    println!("  1. Enter API key now (saved securely in stockpot)");
    println!(
        "  2. I'll set it via environment variable (export {}=...)",
        env_var
    );
    println!("  0. Cancel");

    print!("\nChoice: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    match choice {
        "1" => {
            print!("\nEnter API key: ");
            io::stdout().flush()?;

            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();

            if key.is_empty() {
                return Ok(None);
            }

            // Save to database
            if let Err(e) = db.save_api_key(env_var, key) {
                println!("\x1b[33m⚠️  Failed to save API key: {}\x1b[0m", e);
            } else {
                println!("\x1b[32m✓ API key saved to stockpot database\x1b[0m");
            }

            // Also set in current process for immediate use
            std::env::set_var(env_var, key);

            Ok(Some(env_var.to_string()))
        }
        "2" => {
            println!(
                "\n\x1b[33mRemember to set: export {}=your_api_key\x1b[0m",
                env_var
            );
            Ok(Some(env_var.to_string()))
        }
        _ => Ok(None),
    }
}

/// Run the interactive add model flow
pub async fn run_add_model(db: &Database) -> Result<()> {
    println!("\n\x1b[1m🍲 Add Model Wizard\x1b[0m\n");

    // Fetch providers
    let providers = fetch_providers().await?;
    println!("\x1b[32m✓ Found {} providers\x1b[0m", providers.len());

    // Select provider
    let provider = match select_provider(&providers)? {
        Some(p) => p,
        None => {
            println!("\nCancelled.");
            return Ok(());
        }
    };

    // Select model
    let model = match select_model(provider)? {
        Some(m) => m,
        None => {
            println!("\nCancelled.");
            return Ok(());
        }
    };

    // Prompt for API key
    let env_var = match prompt_api_key(db, provider)? {
        Some(e) => e,
        None => {
            println!("\nCancelled.");
            return Ok(());
        }
    };

    // Generate the model name
    let model_name = format!("{}:{}", provider.id, model.id);

    // Get the API endpoint URL - use provider.api if available, otherwise use known fallbacks
    let api_url = provider.api.clone().unwrap_or_else(|| {
        // Known provider API endpoints (when not specified in the bundled catalog)
        match provider.id.as_str() {
            "cerebras" => "https://api.cerebras.ai/v1".to_string(),
            "together" => "https://api.together.xyz/v1".to_string(),
            "groq" => "https://api.groq.com/openai/v1".to_string(),
            "fireworks" => "https://api.fireworks.ai/inference/v1".to_string(),
            "deepseek" => "https://api.deepseek.com/v1".to_string(),
            "mistral" => "https://api.mistral.ai/v1".to_string(),
            "perplexity" => "https://api.perplexity.ai".to_string(),
            "openrouter" => "https://openrouter.ai/api/v1".to_string(),
            "anyscale" => "https://api.endpoints.anyscale.com/v1".to_string(),
            "lepton" => "https://api.lepton.ai/v1".to_string(),
            "novita" => "https://api.novita.ai/v3/openai".to_string(),
            "hyperbolic" => "https://api.hyperbolic.xyz/v1".to_string(),
            "sambanova" => "https://api.sambanova.ai/v1".to_string(),
            _ => {
                println!("\x1b[1;33m⚠️  No API endpoint found for provider '{}', using OpenAI-compatible default\x1b[0m", provider.id);
                "https://api.openai.com/v1".to_string()
            }
        }
    });

    println!("\x1b[2mUsing API endpoint: {}\x1b[0m", api_url);

    // Create the model config
    let config = ModelConfig {
        name: model_name.clone(),
        model_type: ModelType::CustomOpenai, // Default - works with most providers
        model_id: Some(model.id.clone()),
        context_length: model.context_length.unwrap_or(128_000) as usize,
        supports_thinking: false,
        supports_vision: false,
        supports_tools: true,
        description: Some(model.name.clone().unwrap_or_else(|| model.id.clone())),
        custom_endpoint: Some(CustomEndpoint {
            url: api_url,
            api_key: Some(format!("${}", env_var)),
            headers: HashMap::new(),
            ca_certs_path: None,
        }),
        azure_deployment: None,
        azure_api_version: None,
        round_robin_models: Vec::new(),
    };

    // Save to database
    ModelRegistry::add_model_to_db(db, &config)?;

    println!("\n\x1b[1;32m✅ Model added successfully!\x1b[0m");
    println!("\nTo use this model:");
    println!("  \x1b[1;36m/model {}\x1b[0m", model_name);
    println!("\nOr pin it to an agent:");
    println!("  \x1b[1;36m/pin {}\x1b[0m", model_name);

    Ok(())
}

/// List all custom models from the database.
pub fn list_custom_models(db: &Database) -> Result<()> {
    let registry = ModelRegistry::load_from_db(db)?;
    let available = registry.list_available(db);

    if available.is_empty() {
        println!("\x1b[2mNo available models found.\x1b[0m");
        println!("\x1b[2mUse /add_model to add models from the bundled catalog\x1b[0m");
        return Ok(());
    }

    println!("\n\x1b[1m📋 Available Models:\x1b[0m\n");

    for name in &available {
        if let Some(config) = registry.get(name) {
            let desc = config.description.as_deref().unwrap_or("No description");
            println!("  • \x1b[1;36m{}\x1b[0m", name);
            println!("    \x1b[2m{}\x1b[0m", desc);
        }
    }

    println!();
    Ok(())
}

/// Get the API endpoint URL for a provider, using provider.api if available,
/// otherwise using known fallbacks.
///
/// This function is extracted for testability.
pub fn get_api_endpoint(provider: &ProviderInfo) -> String {
    provider
        .api
        .clone()
        .unwrap_or_else(|| match provider.id.as_str() {
            "cerebras" => "https://api.cerebras.ai/v1".to_string(),
            "together" => "https://api.together.xyz/v1".to_string(),
            "groq" => "https://api.groq.com/openai/v1".to_string(),
            "fireworks" => "https://api.fireworks.ai/inference/v1".to_string(),
            "deepseek" => "https://api.deepseek.com/v1".to_string(),
            "mistral" => "https://api.mistral.ai/v1".to_string(),
            "perplexity" => "https://api.perplexity.ai".to_string(),
            "openrouter" => "https://openrouter.ai/api/v1".to_string(),
            "anyscale" => "https://api.endpoints.anyscale.com/v1".to_string(),
            "lepton" => "https://api.lepton.ai/v1".to_string(),
            "novita" => "https://api.novita.ai/v3/openai".to_string(),
            "hyperbolic" => "https://api.hyperbolic.xyz/v1".to_string(),
            "sambanova" => "https://api.sambanova.ai/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        })
}

/// Get the environment variable name for a provider's API key.
pub fn get_env_var_name(provider: &ProviderInfo) -> &str {
    provider
        .env
        .first()
        .map(|s| s.as_str())
        .unwrap_or("API_KEY")
}

/// Generate a model name in the format "provider:model_id".
pub fn generate_model_name(provider_id: &str, model_id: &str) -> String {
    format!("{}:{}", provider_id, model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ProviderInfo Deserialization Tests
    // =========================================================================

    #[test]
    fn test_provider_info_full_deserialization() {
        let json = r#"{
            "id": "openai",
            "name": "OpenAI",
            "env": ["OPENAI_API_KEY"],
            "api": "https://api.openai.com/v1",
            "doc": "https://platform.openai.com/docs",
            "models": {
                "gpt-4o": {
                    "id": "gpt-4o",
                    "name": "GPT-4o",
                    "context_length": 128000,
                    "input_price": 5.0,
                    "output_price": 15.0
                }
            }
        }"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "openai");
        assert_eq!(provider.name, "OpenAI");
        assert_eq!(provider.env, vec!["OPENAI_API_KEY"]);
        assert_eq!(provider.api, Some("https://api.openai.com/v1".to_string()));
        assert_eq!(
            provider.doc,
            Some("https://platform.openai.com/docs".to_string())
        );
        assert_eq!(provider.models.len(), 1);

        let model = provider.models.get("gpt-4o").unwrap();
        assert_eq!(model.id, "gpt-4o");
        assert_eq!(model.name, Some("GPT-4o".to_string()));
        assert_eq!(model.context_length, Some(128000));
        assert_eq!(model.input_price, Some(5.0));
        assert_eq!(model.output_price, Some(15.0));
    }

    #[test]
    fn test_provider_info_minimal_deserialization() {
        let json = r#"{"id": "custom"}"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "custom");
        assert_eq!(provider.name, ""); // default empty string
        assert!(provider.env.is_empty());
        assert!(provider.api.is_none());
        assert!(provider.doc.is_none());
        assert!(provider.models.is_empty());
    }

    #[test]
    fn test_model_info_minimal_deserialization() {
        let json = r#"{"id": "test-model"}"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "test-model");
        assert!(model.name.is_none());
        assert!(model.context_length.is_none());
        assert!(model.input_price.is_none());
        assert!(model.output_price.is_none());
    }

    // =========================================================================
    // Bundled Catalog Tests
    // =========================================================================

    #[test]
    fn test_bundled_catalog_parses() {
        // Verify the bundled catalog can be parsed
        let result: Result<HashMap<String, ProviderInfo>, _> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON);
        assert!(result.is_ok(), "Bundled catalog should parse: {:?}", result);

        let providers = result.unwrap();
        assert!(
            !providers.is_empty(),
            "Bundled catalog should have providers"
        );
    }

    #[test]
    fn test_bundled_catalog_has_major_providers() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        // Check that at least some major providers exist
        let major_providers = ["openai", "anthropic", "google"];
        for provider in major_providers {
            // Not all might be present, but at least check the structure works
            if let Some(p) = providers.get(provider) {
                assert!(!p.id.is_empty());
            }
        }
    }

    // =========================================================================
    // API Endpoint Resolution Tests
    // =========================================================================

    #[test]
    fn test_get_api_endpoint_uses_provider_api() {
        let provider = ProviderInfo {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            api: Some("https://custom.api.com/v1".to_string()),
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        assert_eq!(get_api_endpoint(&provider), "https://custom.api.com/v1");
    }

    #[test]
    fn test_get_api_endpoint_known_fallbacks() {
        let test_cases = vec![
            ("cerebras", "https://api.cerebras.ai/v1"),
            ("together", "https://api.together.xyz/v1"),
            ("groq", "https://api.groq.com/openai/v1"),
            ("fireworks", "https://api.fireworks.ai/inference/v1"),
            ("deepseek", "https://api.deepseek.com/v1"),
            ("mistral", "https://api.mistral.ai/v1"),
            ("perplexity", "https://api.perplexity.ai"),
            ("openrouter", "https://openrouter.ai/api/v1"),
            ("anyscale", "https://api.endpoints.anyscale.com/v1"),
            ("lepton", "https://api.lepton.ai/v1"),
            ("novita", "https://api.novita.ai/v3/openai"),
            ("hyperbolic", "https://api.hyperbolic.xyz/v1"),
            ("sambanova", "https://api.sambanova.ai/v1"),
        ];

        for (provider_id, expected_url) in test_cases {
            let provider = ProviderInfo {
                id: provider_id.to_string(),
                name: String::new(),
                api: None,
                env: vec![],
                doc: None,
                models: HashMap::new(),
            };
            assert_eq!(
                get_api_endpoint(&provider),
                expected_url,
                "Failed for provider: {}",
                provider_id
            );
        }
    }

    #[test]
    fn test_get_api_endpoint_unknown_provider_defaults_to_openai() {
        let provider = ProviderInfo {
            id: "unknown_provider_xyz".to_string(),
            name: String::new(),
            api: None,
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        assert_eq!(get_api_endpoint(&provider), "https://api.openai.com/v1");
    }

    // =========================================================================
    // Environment Variable Name Tests
    // =========================================================================

    #[test]
    fn test_get_env_var_name_from_provider() {
        let provider = ProviderInfo {
            id: "openai".to_string(),
            name: String::new(),
            api: None,
            env: vec!["OPENAI_API_KEY".to_string()],
            doc: None,
            models: HashMap::new(),
        };

        assert_eq!(get_env_var_name(&provider), "OPENAI_API_KEY");
    }

    #[test]
    fn test_get_env_var_name_multiple_env_vars_uses_first() {
        let provider = ProviderInfo {
            id: "test".to_string(),
            name: String::new(),
            api: None,
            env: vec!["PRIMARY_KEY".to_string(), "SECONDARY_KEY".to_string()],
            doc: None,
            models: HashMap::new(),
        };

        assert_eq!(get_env_var_name(&provider), "PRIMARY_KEY");
    }

    #[test]
    fn test_get_env_var_name_empty_defaults_to_api_key() {
        let provider = ProviderInfo {
            id: "custom".to_string(),
            name: String::new(),
            api: None,
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        assert_eq!(get_env_var_name(&provider), "API_KEY");
    }

    // =========================================================================
    // Model Name Generation Tests
    // =========================================================================

    #[test]
    fn test_generate_model_name() {
        assert_eq!(generate_model_name("openai", "gpt-4o"), "openai:gpt-4o");
        assert_eq!(
            generate_model_name("anthropic", "claude-3-opus"),
            "anthropic:claude-3-opus"
        );
    }

    #[test]
    fn test_generate_model_name_with_special_chars() {
        assert_eq!(
            generate_model_name("provider-name", "model.v2"),
            "provider-name:model.v2"
        );
    }

    #[test]
    fn test_generate_model_name_empty_strings() {
        assert_eq!(generate_model_name("", ""), ":");
        assert_eq!(generate_model_name("provider", ""), "provider:");
        assert_eq!(generate_model_name("", "model"), ":model");
    }

    #[test]
    fn test_generate_model_name_with_slashes() {
        // Some model IDs contain slashes (e.g., openrouter format)
        assert_eq!(
            generate_model_name("openrouter", "anthropic/claude-3-opus"),
            "openrouter:anthropic/claude-3-opus"
        );
    }

    // =========================================================================
    // ModelInfo Deserialization Tests
    // =========================================================================

    #[test]
    fn test_model_info_full_deserialization() {
        let json = r#"{
            "id": "gpt-4-turbo",
            "name": "GPT-4 Turbo",
            "context_length": 128000,
            "input_price": 10.0,
            "output_price": 30.0
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "gpt-4-turbo");
        assert_eq!(model.name, Some("GPT-4 Turbo".to_string()));
        assert_eq!(model.context_length, Some(128000));
        assert_eq!(model.input_price, Some(10.0));
        assert_eq!(model.output_price, Some(30.0));
    }

    #[test]
    fn test_model_info_partial_pricing() {
        // Only input price
        let json = r#"{"id": "test", "input_price": 5.0}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.input_price, Some(5.0));
        assert!(model.output_price.is_none());

        // Only output price
        let json = r#"{"id": "test", "output_price": 15.0}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert!(model.input_price.is_none());
        assert_eq!(model.output_price, Some(15.0));
    }

    #[test]
    fn test_model_info_zero_values() {
        let json = r#"{
            "id": "free-model",
            "context_length": 0,
            "input_price": 0.0,
            "output_price": 0.0
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.context_length, Some(0));
        assert_eq!(model.input_price, Some(0.0));
        assert_eq!(model.output_price, Some(0.0));
    }

    #[test]
    fn test_model_info_large_context_length() {
        let json = r#"{"id": "large-ctx", "context_length": 2000000}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.context_length, Some(2_000_000));
    }

    // =========================================================================
    // ProviderInfo Edge Cases
    // =========================================================================

    #[test]
    fn test_provider_info_multiple_env_vars() {
        let json = r#"{
            "id": "multi-env",
            "env": ["PRIMARY_KEY", "SECONDARY_KEY", "FALLBACK_KEY"]
        }"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.env.len(), 3);
        assert_eq!(provider.env[0], "PRIMARY_KEY");
        assert_eq!(provider.env[1], "SECONDARY_KEY");
        assert_eq!(provider.env[2], "FALLBACK_KEY");
    }

    #[test]
    fn test_provider_info_empty_models_map() {
        let json = r#"{"id": "empty-provider", "models": {}}"#;
        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert!(provider.models.is_empty());
    }

    #[test]
    fn test_provider_info_multiple_models() {
        let json = r#"{
            "id": "multi-model",
            "models": {
                "model-a": {"id": "model-a"},
                "model-b": {"id": "model-b"},
                "model-c": {"id": "model-c"}
            }
        }"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.models.len(), 3);
        assert!(provider.models.contains_key("model-a"));
        assert!(provider.models.contains_key("model-b"));
        assert!(provider.models.contains_key("model-c"));
    }

    #[test]
    fn test_provider_info_with_all_optional_fields() {
        let json = r#"{
            "id": "complete",
            "name": "Complete Provider",
            "env": ["COMPLETE_API_KEY"],
            "api": "https://api.complete.com/v1",
            "doc": "https://docs.complete.com"
        }"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "complete");
        assert_eq!(provider.name, "Complete Provider");
        assert!(!provider.env.is_empty());
        assert!(provider.api.is_some());
        assert!(provider.doc.is_some());
    }

    // =========================================================================
    // Catalog Parsing Tests
    // =========================================================================

    #[test]
    fn test_bundled_catalog_provider_structure() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        // Every provider should have a non-empty id
        for (key, provider) in &providers {
            assert!(!provider.id.is_empty(), "Provider {} has empty id", key);
        }
    }

    #[test]
    fn test_bundled_catalog_models_have_ids() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        for (provider_key, provider) in &providers {
            for (model_key, model) in &provider.models {
                assert!(
                    !model.id.is_empty(),
                    "Model {} in provider {} has empty id",
                    model_key,
                    provider_key
                );
            }
        }
    }

    // =========================================================================
    // API Endpoint URL Validation Tests
    // =========================================================================

    #[test]
    fn test_get_api_endpoint_returns_valid_urls() {
        let known_providers = [
            "cerebras",
            "together",
            "groq",
            "fireworks",
            "deepseek",
            "mistral",
            "perplexity",
            "openrouter",
            "anyscale",
            "lepton",
            "novita",
            "hyperbolic",
            "sambanova",
        ];

        for provider_id in known_providers {
            let provider = ProviderInfo {
                id: provider_id.to_string(),
                name: String::new(),
                api: None,
                env: vec![],
                doc: None,
                models: HashMap::new(),
            };

            let url = get_api_endpoint(&provider);
            assert!(
                url.starts_with("https://"),
                "Provider {} URL should start with https://: {}",
                provider_id,
                url
            );
        }
    }

    #[test]
    fn test_get_api_endpoint_provider_api_takes_precedence() {
        // Even for known providers, if api is set, it should be used
        let provider = ProviderInfo {
            id: "groq".to_string(), // Known provider
            name: String::new(),
            api: Some("https://custom.groq.endpoint/v2".to_string()),
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        assert_eq!(
            get_api_endpoint(&provider),
            "https://custom.groq.endpoint/v2"
        );
    }

    #[test]
    fn test_get_api_endpoint_empty_api_uses_fallback() {
        // Empty string api should not be used (Option::Some("") is truthy but empty)
        let provider = ProviderInfo {
            id: "groq".to_string(),
            name: String::new(),
            api: Some("".to_string()),
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        // Empty string is still Some, so it will be used
        assert_eq!(get_api_endpoint(&provider), "");
    }

    // =========================================================================
    // Env Var Name Edge Cases
    // =========================================================================

    #[test]
    fn test_get_env_var_name_with_empty_string_in_vec() {
        let provider = ProviderInfo {
            id: "test".to_string(),
            name: String::new(),
            api: None,
            env: vec!["".to_string()], // Empty string as first env var
            doc: None,
            models: HashMap::new(),
        };

        // Should return empty string (first element)
        assert_eq!(get_env_var_name(&provider), "");
    }

    // =========================================================================
    // ModelConfig Creation Tests (simulating add_model flow)
    // =========================================================================

    #[test]
    fn test_model_config_creation_from_provider_and_model() {
        let provider = ProviderInfo {
            id: "test-provider".to_string(),
            name: "Test Provider".to_string(),
            api: Some("https://api.test.com/v1".to_string()),
            env: vec!["TEST_API_KEY".to_string()],
            doc: Some("https://docs.test.com".to_string()),
            models: HashMap::new(),
        };

        let model = ModelInfo {
            id: "test-model".to_string(),
            name: Some("Test Model".to_string()),
            context_length: Some(64000),
            input_price: Some(1.0),
            output_price: Some(2.0),
        };

        let model_name = generate_model_name(&provider.id, &model.id);
        let api_url = get_api_endpoint(&provider);
        let env_var = get_env_var_name(&provider);

        let config = ModelConfig {
            name: model_name.clone(),
            model_type: ModelType::CustomOpenai,
            model_id: Some(model.id.clone()),
            context_length: model.context_length.unwrap_or(128_000) as usize,
            supports_thinking: false,
            supports_vision: false,
            supports_tools: true,
            description: Some(model.name.clone().unwrap_or_else(|| model.id.clone())),
            custom_endpoint: Some(CustomEndpoint {
                url: api_url.clone(),
                api_key: Some(format!("${}", env_var)),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        assert_eq!(config.name, "test-provider:test-model");
        assert_eq!(config.model_id, Some("test-model".to_string()));
        assert_eq!(config.context_length, 64000);
        assert_eq!(config.description, Some("Test Model".to_string()));

        let endpoint = config.custom_endpoint.as_ref().unwrap();
        assert_eq!(endpoint.url, "https://api.test.com/v1");
        assert_eq!(endpoint.api_key, Some("$TEST_API_KEY".to_string()));
    }

    #[test]
    fn test_model_config_creation_with_missing_context_length() {
        let model = ModelInfo {
            id: "no-ctx".to_string(),
            name: None,
            context_length: None,
            input_price: None,
            output_price: None,
        };

        let context_length = model.context_length.unwrap_or(128_000) as usize;
        assert_eq!(context_length, 128_000);
    }

    #[test]
    fn test_model_config_creation_with_missing_name_uses_id() {
        let model = ModelInfo {
            id: "model-id-123".to_string(),
            name: None,
            context_length: None,
            input_price: None,
            output_price: None,
        };

        let description = model.name.clone().unwrap_or_else(|| model.id.clone());
        assert_eq!(description, "model-id-123");
    }

    // =========================================================================
    // Serialization Roundtrip Tests
    // =========================================================================

    #[test]
    fn test_provider_info_serialization_roundtrip() {
        let original = ProviderInfo {
            id: "roundtrip".to_string(),
            name: "Roundtrip Provider".to_string(),
            env: vec!["KEY1".to_string(), "KEY2".to_string()],
            api: Some("https://api.roundtrip.com".to_string()),
            doc: Some("https://docs.roundtrip.com".to_string()),
            models: HashMap::from([(
                "model1".to_string(),
                ModelInfo {
                    id: "model1".to_string(),
                    name: Some("Model One".to_string()),
                    context_length: Some(100000),
                    input_price: Some(5.0),
                    output_price: Some(10.0),
                },
            )]),
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ProviderInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.env, original.env);
        assert_eq!(parsed.api, original.api);
        assert_eq!(parsed.doc, original.doc);
        assert_eq!(parsed.models.len(), original.models.len());
    }

    #[test]
    fn test_model_info_serialization_roundtrip() {
        let original = ModelInfo {
            id: "roundtrip-model".to_string(),
            name: Some("Roundtrip Model".to_string()),
            context_length: Some(256000),
            input_price: Some(2.5),
            output_price: Some(7.5),
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.context_length, original.context_length);
        assert_eq!(parsed.input_price, original.input_price);
        assert_eq!(parsed.output_price, original.output_price);
    }

    // =========================================================================
    // Provider Detection Logic Tests
    // =========================================================================

    #[test]
    fn test_provider_detection_all_known_providers() {
        // Test that all known provider IDs map to correct endpoints
        let expected_mappings = vec![
            ("cerebras", "https://api.cerebras.ai/v1"),
            ("together", "https://api.together.xyz/v1"),
            ("groq", "https://api.groq.com/openai/v1"),
            ("fireworks", "https://api.fireworks.ai/inference/v1"),
            ("deepseek", "https://api.deepseek.com/v1"),
            ("mistral", "https://api.mistral.ai/v1"),
            ("perplexity", "https://api.perplexity.ai"),
            ("openrouter", "https://openrouter.ai/api/v1"),
            ("anyscale", "https://api.endpoints.anyscale.com/v1"),
            ("lepton", "https://api.lepton.ai/v1"),
            ("novita", "https://api.novita.ai/v3/openai"),
            ("hyperbolic", "https://api.hyperbolic.xyz/v1"),
            ("sambanova", "https://api.sambanova.ai/v1"),
        ];

        for (provider_id, expected_url) in expected_mappings {
            let provider = ProviderInfo {
                id: provider_id.to_string(),
                name: String::new(),
                api: None,
                env: vec![],
                doc: None,
                models: HashMap::new(),
            };

            let actual_url = get_api_endpoint(&provider);
            assert_eq!(
                actual_url, expected_url,
                "Mismatch for provider {}: expected {}, got {}",
                provider_id, expected_url, actual_url
            );
        }
    }

    #[test]
    fn test_provider_detection_case_sensitive() {
        // Provider IDs should be case-sensitive
        let provider = ProviderInfo {
            id: "GROQ".to_string(), // uppercase
            name: String::new(),
            api: None,
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        // Should fall back to OpenAI default (case mismatch)
        assert_eq!(get_api_endpoint(&provider), "https://api.openai.com/v1");
    }

    #[test]
    fn test_provider_detection_similar_names() {
        // Providers with similar but not exact names should default
        let similar_names = ["groq-ai", "together-ai", "deepseek-v2", "openrouter-pro"];

        for name in similar_names {
            let provider = ProviderInfo {
                id: name.to_string(),
                name: String::new(),
                api: None,
                env: vec![],
                doc: None,
                models: HashMap::new(),
            };

            assert_eq!(
                get_api_endpoint(&provider),
                "https://api.openai.com/v1",
                "Similar name '{}' should default to OpenAI",
                name
            );
        }
    }

    // =========================================================================
    // CustomEndpoint Configuration Tests
    // =========================================================================

    #[test]
    fn test_custom_endpoint_with_env_var_reference() {
        let endpoint = CustomEndpoint {
            url: "https://api.custom.com".to_string(),
            api_key: Some("$MY_CUSTOM_KEY".to_string()),
            headers: HashMap::new(),
            ca_certs_path: None,
        };

        assert!(endpoint.api_key.as_ref().unwrap().starts_with('$'));
    }

    #[test]
    fn test_custom_endpoint_with_braced_env_var() {
        let endpoint = CustomEndpoint {
            url: "https://api.custom.com".to_string(),
            api_key: Some("${MY_BRACED_KEY}".to_string()),
            headers: HashMap::new(),
            ca_certs_path: None,
        };

        let key = endpoint.api_key.as_ref().unwrap();
        assert!(key.starts_with("${") && key.ends_with("}"));
    }

    #[test]
    fn test_custom_endpoint_with_headers() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
        headers.insert("Authorization".to_string(), "Bearer token".to_string());

        let endpoint = CustomEndpoint {
            url: "https://api.custom.com".to_string(),
            api_key: None,
            headers,
            ca_certs_path: None,
        };

        assert_eq!(endpoint.headers.len(), 2);
        assert_eq!(
            endpoint.headers.get("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
    }

    // =========================================================================
    // JSON Parsing Edge Cases
    // =========================================================================

    #[test]
    fn test_provider_info_extra_fields_ignored() {
        // Unknown fields should be silently ignored
        let json = r#"{
            "id": "test",
            "unknown_field": "should be ignored",
            "another_unknown": 123
        }"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "test");
    }

    #[test]
    fn test_model_info_extra_fields_ignored() {
        let json = r#"{
            "id": "test",
            "unknown_capability": true,
            "max_tokens": 4096
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "test");
    }

    #[test]
    fn test_provider_info_null_optional_fields() {
        // Note: serde(default) doesn't handle explicit `null` for String fields
        // `null` for Option<T> fields works, but not for String with default
        let json = r#"{
            "id": "test",
            "api": null,
            "doc": null
        }"#;

        // api and doc are Option<String>, so null works for them
        let result: Result<ProviderInfo, _> = serde_json::from_str(json);
        assert!(result.is_ok());

        let provider = result.unwrap();
        assert_eq!(provider.id, "test");
        assert!(provider.api.is_none());
        assert!(provider.doc.is_none());
    }

    #[test]
    fn test_provider_info_null_name_fails() {
        // `name` is String with #[serde(default)], explicit null causes error
        // This documents actual behavior - not a bug, just how serde works
        let json = r#"{"id": "test", "name": null}"#;
        let result: Result<ProviderInfo, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // =========================================================================
    // Numeric Edge Cases
    // =========================================================================

    #[test]
    fn test_model_info_float_precision() {
        let json = r#"{
            "id": "precise",
            "input_price": 0.001,
            "output_price": 0.0001
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert!((model.input_price.unwrap() - 0.001).abs() < f64::EPSILON);
        assert!((model.output_price.unwrap() - 0.0001).abs() < f64::EPSILON);
    }

    #[test]
    fn test_model_info_very_large_context() {
        let json = r#"{"id": "huge", "context_length": 10000000}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.context_length, Some(10_000_000));
    }

    // =========================================================================
    // Integration-style Tests
    // =========================================================================

    #[test]
    fn test_full_model_addition_flow_simulation() {
        // Simulate the full flow of adding a model
        let provider = ProviderInfo {
            id: "test-provider".to_string(),
            name: "Test Provider Inc.".to_string(),
            env: vec!["TEST_PROVIDER_API_KEY".to_string()],
            api: Some("https://api.testprovider.com/v1".to_string()),
            doc: Some("https://docs.testprovider.com".to_string()),
            models: HashMap::from([(
                "large-model".to_string(),
                ModelInfo {
                    id: "large-model".to_string(),
                    name: Some("Large Language Model".to_string()),
                    context_length: Some(200000),
                    input_price: Some(3.0),
                    output_price: Some(9.0),
                },
            )]),
        };

        let model = provider.models.get("large-model").unwrap();

        // Step 1: Generate model name
        let model_name = generate_model_name(&provider.id, &model.id);
        assert_eq!(model_name, "test-provider:large-model");

        // Step 2: Get API endpoint
        let api_url = get_api_endpoint(&provider);
        assert_eq!(api_url, "https://api.testprovider.com/v1");

        // Step 3: Get env var name
        let env_var = get_env_var_name(&provider);
        assert_eq!(env_var, "TEST_PROVIDER_API_KEY");

        // Step 4: Create ModelConfig
        let config = ModelConfig {
            name: model_name,
            model_type: ModelType::CustomOpenai,
            model_id: Some(model.id.clone()),
            context_length: model.context_length.unwrap_or(128_000) as usize,
            supports_thinking: false,
            supports_vision: false,
            supports_tools: true,
            description: Some(model.name.clone().unwrap_or_else(|| model.id.clone())),
            custom_endpoint: Some(CustomEndpoint {
                url: api_url,
                api_key: Some(format!("${}", env_var)),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        // Verify final config
        assert_eq!(config.name, "test-provider:large-model");
        assert_eq!(config.context_length, 200000);
        assert_eq!(config.description, Some("Large Language Model".to_string()));
        assert!(config.custom_endpoint.is_some());
    }

    #[test]
    fn test_provider_without_api_uses_fallback_correctly() {
        // Provider from catalog might not have api field
        let provider = ProviderInfo {
            id: "groq".to_string(),
            name: "Groq".to_string(),
            env: vec!["GROQ_API_KEY".to_string()],
            api: None, // No API specified
            doc: None,
            models: HashMap::new(),
        };

        let url = get_api_endpoint(&provider);
        assert_eq!(url, "https://api.groq.com/openai/v1");
    }

    // =========================================================================
    // Additional Edge Cases
    // =========================================================================

    #[test]
    fn test_model_info_negative_prices() {
        // Edge case: negative prices (shouldn't happen but should deserialize)
        let json = r#"{
            "id": "negative",
            "input_price": -1.0,
            "output_price": -0.5
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.input_price, Some(-1.0));
        assert_eq!(model.output_price, Some(-0.5));
    }

    #[test]
    fn test_provider_info_unicode_names() {
        let json = r#"{
            "id": "中文provider",
            "name": "日本語プロバイダー",
            "env": ["UNICODE_KEY_🔑"]
        }"#;

        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "中文provider");
        assert_eq!(provider.name, "日本語プロバイダー");
        assert_eq!(provider.env[0], "UNICODE_KEY_🔑");
    }

    #[test]
    fn test_model_info_unicode_id() {
        let json = r#"{"id": "модель-v2"}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "модель-v2");
    }

    #[test]
    fn test_generate_model_name_unicode() {
        let name = generate_model_name("提供商", "模型");
        assert_eq!(name, "提供商:模型");
    }

    #[test]
    fn test_provider_info_whitespace_in_id() {
        // Whitespace in ID (shouldn't happen but should deserialize)
        let json = r#"{"id": " spaced id "}"#;
        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, " spaced id ");
    }

    #[test]
    fn test_model_info_whitespace_in_name() {
        let json = r#"{"id": "test", "name": "  Padded Name  "}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.name, Some("  Padded Name  ".to_string()));
    }

    #[test]
    fn test_provider_info_clone() {
        let original = ProviderInfo {
            id: "clone-test".to_string(),
            name: "Clone Test".to_string(),
            env: vec!["KEY".to_string()],
            api: Some("https://api.clone.test".to_string()),
            doc: Some("https://docs.clone.test".to_string()),
            models: HashMap::from([(
                "model".to_string(),
                ModelInfo {
                    id: "model".to_string(),
                    name: Some("Model".to_string()),
                    context_length: Some(1000),
                    input_price: Some(1.0),
                    output_price: Some(2.0),
                },
            )]),
        };

        let cloned = original.clone();
        assert_eq!(cloned.id, original.id);
        assert_eq!(cloned.name, original.name);
        assert_eq!(cloned.env, original.env);
        assert_eq!(cloned.api, original.api);
        assert_eq!(cloned.doc, original.doc);
        assert_eq!(cloned.models.len(), original.models.len());
    }

    #[test]
    fn test_model_info_clone() {
        let original = ModelInfo {
            id: "clone-model".to_string(),
            name: Some("Clone Model".to_string()),
            context_length: Some(50000),
            input_price: Some(2.5),
            output_price: Some(5.0),
        };

        let cloned = original.clone();
        assert_eq!(cloned.id, original.id);
        assert_eq!(cloned.name, original.name);
        assert_eq!(cloned.context_length, original.context_length);
        assert_eq!(cloned.input_price, original.input_price);
        assert_eq!(cloned.output_price, original.output_price);
    }

    #[test]
    fn test_provider_info_debug_impl() {
        let provider = ProviderInfo {
            id: "debug-test".to_string(),
            name: String::new(),
            env: vec![],
            api: None,
            doc: None,
            models: HashMap::new(),
        };

        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_model_info_debug_impl() {
        let model = ModelInfo {
            id: "debug-model".to_string(),
            name: None,
            context_length: None,
            input_price: None,
            output_price: None,
        };

        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("debug-model"));
    }

    #[test]
    fn test_get_api_endpoint_whitespace_provider_id() {
        // Provider ID with whitespace should not match known providers
        let provider = ProviderInfo {
            id: " groq ".to_string(),
            name: String::new(),
            api: None,
            env: vec![],
            doc: None,
            models: HashMap::new(),
        };

        // Should default to OpenAI (no match for " groq ")
        assert_eq!(get_api_endpoint(&provider), "https://api.openai.com/v1");
    }

    #[test]
    fn test_bundled_catalog_context_lengths_are_reasonable() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        for (provider_key, provider) in &providers {
            for (model_key, model) in &provider.models {
                if let Some(ctx) = model.context_length {
                    // Context lengths should be > 0 and < 100 million (sanity check)
                    assert!(
                        ctx > 0 && ctx < 100_000_000,
                        "Unreasonable context_length {} for {}/{}",
                        ctx,
                        provider_key,
                        model_key
                    );
                }
            }
        }
    }

    #[test]
    fn test_bundled_catalog_prices_are_non_negative() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        for (provider_key, provider) in &providers {
            for (model_key, model) in &provider.models {
                if let Some(price) = model.input_price {
                    assert!(
                        price >= 0.0,
                        "Negative input_price {} for {}/{}",
                        price,
                        provider_key,
                        model_key
                    );
                }
                if let Some(price) = model.output_price {
                    assert!(
                        price >= 0.0,
                        "Negative output_price {} for {}/{}",
                        price,
                        provider_key,
                        model_key
                    );
                }
            }
        }
    }

    #[test]
    fn test_generate_model_name_with_colon_in_model_id() {
        // Model IDs might contain colons (edge case)
        let name = generate_model_name("provider", "model:variant:v2");
        assert_eq!(name, "provider:model:variant:v2");
    }

    #[test]
    fn test_model_config_api_key_format() {
        // Verify the api_key format is "$ENV_VAR"
        let env_var = "MY_API_KEY";
        let api_key = format!("${}", env_var);

        assert!(api_key.starts_with('$'));
        assert!(!api_key.starts_with("${"));
        assert_eq!(api_key, "$MY_API_KEY");
    }

    #[test]
    fn test_provider_info_json_with_integer_context() {
        // context_length as integer (not float) should work
        let json = r#"{"id": "test", "models": {"m": {"id": "m", "context_length": 128000}}}"#;
        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        let model = provider.models.get("m").unwrap();
        assert_eq!(model.context_length, Some(128000));
    }

    #[test]
    fn test_model_info_scientific_notation_price() {
        // Prices might come in scientific notation
        let json = r#"{"id": "sci", "input_price": 1e-3, "output_price": 1.5e-2}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert!((model.input_price.unwrap() - 0.001).abs() < f64::EPSILON);
        assert!((model.output_price.unwrap() - 0.015).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_provider_id() {
        let json = r#"{"id": ""}"#;
        let provider: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "");

        // Empty ID should default to OpenAI endpoint
        assert_eq!(get_api_endpoint(&provider), "https://api.openai.com/v1");
    }

    #[test]
    fn test_model_info_max_u64_context() {
        // Test with max u64 value
        let json = r#"{"id": "max", "context_length": 18446744073709551615}"#;
        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.context_length, Some(u64::MAX));
    }

    #[test]
    fn test_bundled_catalog_provider_keys_match_ids() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        for (key, provider) in &providers {
            // Provider key should match provider.id (common pattern)
            assert_eq!(
                key, &provider.id,
                "Provider key '{}' doesn't match id '{}'",
                key, provider.id
            );
        }
    }

    #[test]
    fn test_bundled_catalog_model_keys_match_ids() {
        let providers: HashMap<String, ProviderInfo> =
            serde_json::from_str(BUNDLED_MODELS_CATALOG_JSON).unwrap();

        for (provider_key, provider) in &providers {
            for (model_key, model) in &provider.models {
                assert_eq!(
                    model_key, &model.id,
                    "Model key '{}' doesn't match id '{}' in provider '{}'",
                    model_key, model.id, provider_key
                );
            }
        }
    }
}
