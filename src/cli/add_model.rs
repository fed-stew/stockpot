//! Interactive model discovery and configuration.
//!
//! This module provides functionality to browse available AI providers
//! and models from models.dev, and configure them for use.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

const MODELS_API_URL: &str = "https://models.dev/api.json";

/// Provider information from models.dev
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

/// Model information from models.dev
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

/// Saved model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedModel {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub api_endpoint: Option<String>,
    pub env_var: String,
}

/// Extra models configuration file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtraModelsConfig {
    pub models: Vec<SavedModel>,
}

impl ExtraModelsConfig {
    /// Load from file or create empty
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Get config file path
    fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".stockpot").join("extra_models.json"))
    }

    /// Add a model
    pub fn add_model(&mut self, model: SavedModel) {
        // Remove existing with same id
        self.models.retain(|m| {
            !(m.provider_id == model.provider_id && m.model_id == model.model_id)
        });
        self.models.push(model);
    }
}

/// Fetch all providers from models.dev
pub async fn fetch_providers() -> Result<HashMap<String, ProviderInfo>> {
    println!("\x1b[2mFetching providers from models.dev...\x1b[0m");
    
    let client = reqwest::Client::new();
    let response = client
        .get(MODELS_API_URL)
        .header("User-Agent", "stockpot/0.1")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(anyhow!("Failed to fetch providers: HTTP {}", response.status()));
    }
    
    let providers: HashMap<String, ProviderInfo> = response.json().await?;
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
            if provider.name.is_empty() { &provider.id } else { &provider.name },
            model_count
        );
    }
    
    println!("\n  \x1b[2m  0. Cancel\x1b[0m");
    print!("\n\x1b[1mSelect provider (1-{}):\x1b[0m ", provider_list.len());
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
    
    println!("\n\x1b[1m🤖 Models for {}:\x1b[0m\n", 
        if provider.name.is_empty() { &provider.id } else { &provider.name });
    
    for (i, model) in model_list.iter().enumerate() {
        let name = model.name.as_deref().unwrap_or(&model.id);
        let ctx = model.context_length
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
            if !price.is_empty() { format!(" | {}", price) } else { String::new() }
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

/// Prompt for API key
pub fn prompt_api_key(provider: &ProviderInfo) -> Result<Option<String>> {
    let env_var = provider.env.first()
        .map(|s| s.as_str())
        .unwrap_or("API_KEY");
    
    println!("\n\x1b[1m🔑 API Key Configuration\x1b[0m");
    
    // Check if already set in environment
    if let Ok(existing) = std::env::var(env_var) {
        if !existing.is_empty() {
            println!("\x1b[32m✓ {} is already set in your environment\x1b[0m", env_var);
            print!("\nUse existing key? [Y/n]: ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();
            
            if input.is_empty() || input == "y" || input == "yes" {
                return Ok(Some(env_var.to_string()));
            }
        }
    }
    
    println!("\nTo use this provider, you need to set the {} environment variable.", env_var);
    
    if let Some(doc) = &provider.doc {
        println!("\x1b[2mDocumentation: {}\x1b[0m", doc);
    }
    
    println!("\n\x1b[1mOptions:\x1b[0m");
    println!("  1. I'll set it manually (export {}=...)", env_var);
    println!("  2. Enter API key now (will be shown in terminal!)");
    println!("  0. Cancel");
    
    print!("\nChoice: ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();
    
    match choice {
        "1" => {
            println!("\n\x1b[33mRemember to set: export {}=your_api_key\x1b[0m", env_var);
            Ok(Some(env_var.to_string()))
        }
        "2" => {
            print!("\nEnter API key: ");
            io::stdout().flush()?;
            
            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();
            
            if key.is_empty() {
                return Ok(None);
            }
            
            // Set in current process
            std::env::set_var(env_var, key);
            println!("\x1b[32m✓ API key set for this session\x1b[0m");
            println!("\x1b[2mNote: Add to your shell profile to persist\x1b[0m");
            
            Ok(Some(env_var.to_string()))
        }
        _ => Ok(None),
    }
}

/// Run the interactive add model flow
pub async fn run_add_model() -> Result<()> {
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
    let env_var = match prompt_api_key(provider)? {
        Some(e) => e,
        None => {
            println!("\nCancelled.");
            return Ok(());
        }
    };
    
    // Create the model config
    let saved_model = SavedModel {
        provider_id: provider.id.clone(),
        model_id: model.id.clone(),
        display_name: model.name.clone().unwrap_or_else(|| model.id.clone()),
        api_endpoint: provider.api.clone(),
        env_var,
    };
    
    // Save to config
    let mut config = ExtraModelsConfig::load()?;
    config.add_model(saved_model.clone());
    config.save()?;
    
    // Generate the model name to use
    let model_name = format!("{}:{}", provider.id, model.id);
    
    println!("\n\x1b[1;32m✅ Model added successfully!\x1b[0m");
    println!("\nTo use this model:");
    println!("  \x1b[1;36m/model {}\x1b[0m", model_name);
    println!("\nOr pin it to an agent:");
    println!("  \x1b[1;36m/pin {}\x1b[0m", model_name);
    
    Ok(())
}

/// List configured extra models
pub fn list_extra_models() -> Result<()> {
    let config = ExtraModelsConfig::load()?;
    
    if config.models.is_empty() {
        println!("\x1b[2mNo extra models configured.\x1b[0m");
        println!("\x1b[2mUse /add_model to add models from models.dev\x1b[0m");
        return Ok(());
    }
    
    println!("\n\x1b[1m📋 Configured Extra Models:\x1b[0m\n");
    
    for model in &config.models {
        let model_name = format!("{}:{}", model.provider_id, model.model_id);
        println!("  • \x1b[1;36m{}\x1b[0m", model_name);
        println!("    \x1b[2mDisplay: {} | Env: {}\x1b[0m", model.display_name, model.env_var);
    }
    
    println!();
    Ok(())
}
