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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
