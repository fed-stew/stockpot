//! Claude Code OAuth authentication.

use super::storage::{TokenStorage, TokenStorageError};
use crate::db::Database;
use crate::models::{ModelConfig, ModelType};
use serde::Deserialize;
use serdes_ai_models::claude_code_oauth::ClaudeCodeOAuthModel;
use serdes_ai_providers::oauth::{
    config::claude_code_oauth_config, refresh_token as oauth_refresh_token, run_pkce_flow,
    OAuthError, TokenResponse,
};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error, info, warn};

const PROVIDER: &str = "claude-code";

#[derive(Debug, Error)]
pub enum ClaudeCodeAuthError {
    #[error("OAuth error: {0}")]
    OAuth(#[from] OAuthError),
    #[error("Storage error: {0}")]
    Storage(#[from] TokenStorageError),
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Browser error: {0}")]
    Browser(String),
}

/// Claude Code authentication manager.
pub struct ClaudeCodeAuth<'a> {
    storage: TokenStorage<'a>,
}

impl<'a> ClaudeCodeAuth<'a> {
    /// Create a new Claude Code auth manager.
    pub fn new(db: &'a Database) -> Self {
        Self {
            storage: TokenStorage::new(db),
        }
    }

    /// Save tokens from OAuth response.
    pub fn save_tokens(&self, tokens: &TokenResponse) -> Result<(), ClaudeCodeAuthError> {
        self.storage.save(
            PROVIDER,
            &tokens.access_token,
            tokens.refresh_token.as_deref(),
            tokens.expires_in,
            None,
            None,
        )?;
        Ok(())
    }

    /// Refresh tokens if needed.
    pub async fn refresh_if_needed(&self) -> Result<String, ClaudeCodeAuthError> {
        debug!("Checking Claude Code token status");

        let tokens = self.storage.load(PROVIDER)?.ok_or_else(|| {
            warn!("No Claude Code tokens found in storage");
            ClaudeCodeAuthError::NotAuthenticated
        })?;

        debug!(
            has_refresh_token = tokens.refresh_token.is_some(),
            is_expired = tokens.is_expired(),
            expires_within_5min = tokens.expires_within(300),
            "Token status"
        );

        // Refresh if expired or expiring within 5 minutes
        if tokens.expires_within(300) {
            if let Some(refresh_token) = &tokens.refresh_token {
                info!("Token expiring soon, refreshing...");
                let config = claude_code_oauth_config();
                match oauth_refresh_token(&config, refresh_token).await {
                    Ok(new_tokens) => {
                        info!("Token refreshed successfully");
                        self.save_tokens(&new_tokens)?;
                        return Ok(new_tokens.access_token);
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to refresh token");
                        return Err(e.into());
                    }
                }
            }
            // No refresh token and expired
            if tokens.is_expired() {
                warn!("Token expired and no refresh token available");
                return Err(ClaudeCodeAuthError::NotAuthenticated);
            }
        }

        debug!("Using existing valid token");
        Ok(tokens.access_token)
    }
}

// ============================================================================
// Model fetching types and functions
// ============================================================================

/// Model info from Anthropic API
#[derive(Debug, Deserialize)]
struct ModelInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

/// Fetch available models from Claude API
async fn fetch_claude_models(access_token: &str) -> Result<Vec<String>, ClaudeCodeAuthError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.anthropic.com/v1/models")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| ClaudeCodeAuthError::OAuth(OAuthError::TokenExchange(e.to_string())))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        error!("Failed to fetch models: {} - {}", status, text);
        return Err(ClaudeCodeAuthError::OAuth(OAuthError::TokenExchange(
            format!("Failed to fetch models: {}", status),
        )));
    }

    let models_response: ModelsResponse = response
        .json()
        .await
        .map_err(|e| ClaudeCodeAuthError::OAuth(OAuthError::TokenExchange(e.to_string())))?;

    let model_names: Vec<String> = models_response
        .data
        .into_iter()
        .filter_map(|m| {
            // Use id as the model name
            let name = m.id;
            // Only include claude models
            if name.starts_with("claude-") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    info!("Fetched {} Claude models from API", model_names.len());
    Ok(model_names)
}

/// Filter to only latest versions of haiku, sonnet, opus
fn filter_latest_models(models: Vec<String>) -> Vec<String> {
    // Log all models received
    info!("Received {} models from API: {:?}", models.len(), models);

    // Map: family -> (model_name, version_tuple)
    // version_tuple = (major, minor, date)
    let mut latest: HashMap<String, (String, (u32, u32, u32))> = HashMap::new();

    for model in &models {
        // Determine family
        let family = if model.contains("haiku") {
            "haiku"
        } else if model.contains("sonnet") {
            "sonnet"
        } else if model.contains("opus") {
            "opus"
        } else {
            continue; // Skip non-Claude models
        };

        // Extract all numbers from the model name
        let numbers: Vec<u32> = model
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();

        // Parse version: expect [major, minor?, date]
        // Examples:
        //   claude-opus-4-5-20251101 -> [4, 5, 20251101]
        //   claude-3-5-sonnet-20241022 -> [3, 5, 20241022]
        //   claude-3-haiku-20240307 -> [3, 20240307]

        let (major, minor, date) = match numbers.as_slice() {
            [m, n, d] if *d > 20000000 => (*m, *n, *d), // major, minor, date
            [m, d] if *d > 20000000 => (*m, 0, *d),     // major, date (no minor)
            [m, n, d, ..] => (*m, *n, *d),              // Take first 3
            _ => continue,
        };

        debug!(
            "  Parsed {}: family={}, version=({}, {}, {})",
            model, family, major, minor, date
        );

        // Check if this is better than current best
        let dominated = latest
            .get(family)
            .is_some_and(|(_, (cur_m, cur_n, cur_d))| {
                // Compare by: major, then minor, then date
                (major, minor, date) <= (*cur_m, *cur_n, *cur_d)
            });

        if !dominated {
            latest.insert(family.to_string(), (model.clone(), (major, minor, date)));
        }
    }

    let filtered: Vec<String> = latest.into_values().map(|(name, _)| name).collect();
    info!(
        "Filtered to {} latest models: {:?}",
        filtered.len(),
        filtered
    );
    filtered
}

/// Save Claude models to database
fn save_claude_models_to_db(db: &Database, models: &[String]) -> Result<(), std::io::Error> {
    use crate::models::ModelRegistry;

    for model_name in models {
        // Create prefixed name like "claude-code-claude-sonnet-4-20250514"
        let prefixed = format!("claude-code-{}", model_name);

        // Determine if it supports thinking (opus and sonnet 4+ do)
        let supports_thinking = model_name.contains("opus")
            || (model_name.contains("sonnet")
                && (model_name.contains("-4") || model_name.contains("4-")));

        let config = ModelConfig {
            name: prefixed,
            model_type: ModelType::ClaudeCode,
            model_id: Some(model_name.clone()),
            context_length: 200_000,
            supports_thinking,
            supports_vision: true,
            supports_tools: true,
            description: None,
            custom_endpoint: None,
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        ModelRegistry::add_model_to_db(db, &config)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
    }

    info!("Saved {} Claude Code models to database", models.len());
    Ok(())
}

/// Run the Claude Code OAuth flow (prints to stdout).
pub async fn run_claude_code_auth(db: &Database) -> Result<(), ClaudeCodeAuthError> {
    run_claude_code_auth_with_progress(db, &super::StdoutProgress).await
}

/// Run the Claude Code OAuth flow with custom progress reporting.
pub async fn run_claude_code_auth_with_progress(
    db: &Database,
    progress: &impl super::AuthProgress,
) -> Result<(), ClaudeCodeAuthError> {
    progress.info("🔐 Starting Claude Code OAuth authentication...");

    let config = claude_code_oauth_config();
    let (auth_url, handle) = run_pkce_flow(&config).await?;

    progress.info("📋 Open this URL in your browser:");
    progress.info(&format!("   {}", auth_url));
    progress.info("");
    progress.info(&format!(
        "⏳ Waiting for authentication callback on port {}...",
        handle.port()
    ));

    // Try to open browser
    if let Err(e) = webbrowser::open(&auth_url) {
        progress.warning(&format!("⚠️  Could not open browser automatically: {}", e));
        progress.info("   Please open the URL manually.");
    }

    let tokens = handle.wait_for_tokens().await?;

    let auth = ClaudeCodeAuth::new(db);
    auth.save_tokens(&tokens)?;

    progress.success("✅ Authentication successful!");

    // Fetch and save available models
    progress.info("📥 Fetching available Claude models...");
    match fetch_claude_models(&tokens.access_token).await {
        Ok(models) => {
            let filtered = filter_latest_models(models);
            if filtered.is_empty() {
                progress.warning("⚠️  No Claude models found. You may need to check your subscription.");
            } else {
                match save_claude_models_to_db(db, &filtered) {
                    Ok(()) => {
                        progress.success(&format!("✅ Saved {} Claude Code models:", filtered.len()));
                        for model in &filtered {
                            progress.info(&format!("   • claude-code-{}", model));
                        }
                    }
                    Err(e) => {
                        progress.warning(&format!("⚠️  Failed to save models: {}", e));
                    }
                }
            }
        }
        Err(e) => {
            progress.warning(&format!("⚠️  Failed to fetch models: {}", e));
            progress.info("   You can try /claude-code-auth again later.");
        }
    }

    progress.info("");
    progress.success("🎉 Claude Code authentication complete!");
    progress.info("   Use /model to select a claude-code-* model.");

    Ok(())
}

/// Known valid Anthropic model name patterns.
/// These are the base patterns without dates.
const KNOWN_MODEL_PATTERNS: &[&str] = &[
    "claude-3-opus",
    "claude-3-sonnet",
    "claude-3-haiku",
    "claude-3-5-sonnet",
    "claude-3-5-haiku",
    "claude-sonnet-4",
    "claude-opus-4",
    "claude-haiku-4",
    // Shorthand aliases
    "claude-sonnet",
    "claude-opus",
    "claude-haiku",
];

/// Validate that a model name looks like a valid Anthropic model.
fn validate_model_name(model_name: &str) -> bool {
    // Check if it matches any known pattern
    for pattern in KNOWN_MODEL_PATTERNS {
        if model_name.starts_with(pattern) {
            return true;
        }
    }
    false
}

/// Get a Claude Code OAuth model, refreshing tokens if needed.
pub async fn get_claude_code_model(
    db: &Database,
    model_name: &str,
    thinking_budget: Option<u64>,
) -> Result<ClaudeCodeOAuthModel, ClaudeCodeAuthError> {
    debug!(model_name = %model_name, ?thinking_budget, "get_claude_code_model called");

    let auth = ClaudeCodeAuth::new(db);
    let access_token = auth.refresh_if_needed().await?;

    // Strip the claude-code- prefix if present
    let actual_model_name = model_name
        .strip_prefix("claude-code-")
        .or_else(|| model_name.strip_prefix("claude_code_"))
        .unwrap_or(model_name);

    // Validate the model name
    if !validate_model_name(actual_model_name) {
        warn!(
            model_name = %actual_model_name,
            "Model name doesn't match known Anthropic patterns! This may cause API errors."
        );
        warn!("Known patterns: {:?}", KNOWN_MODEL_PATTERNS);
        warn!("Example valid names: claude-sonnet-4-20250514, claude-3-5-sonnet-20241022");
    }

    debug!(
        requested_model = %model_name,
        actual_model = %actual_model_name,
        ?thinking_budget,
        "Creating Claude Code OAuth model"
    );

    let mut model = ClaudeCodeOAuthModel::new(actual_model_name, access_token);
    if let Some(budget) = thinking_budget {
        model = model.with_thinking(Some(budget));
    }
    Ok(model)
}
