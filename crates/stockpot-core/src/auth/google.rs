//! Google OAuth authentication.

use super::storage::{StoredTokens, TokenStorage, TokenStorageError};
use crate::db::Database;
use crate::models::{ModelConfig, ModelType};
use serdes_ai_models::antigravity::AntigravityModel;
use serdes_ai_providers::oauth::{
    config::google_oauth_config, refresh_token as oauth_refresh_token, run_pkce_flow, OAuthError,
    TokenResponse,
};
use thiserror::Error;
use tracing::{debug, error, info, warn};

const PROVIDER: &str = "google";

#[derive(Debug, Error)]
pub enum GoogleAuthError {
    #[error("OAuth error: {0}")]
    OAuth(#[from] OAuthError),
    #[error("Storage error: {0}")]
    Storage(#[from] TokenStorageError),
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Browser error: {0}")]
    Browser(String),
    #[error("Onboarding error: {0}")]
    Onboarding(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Google authentication manager.
pub struct GoogleAuth<'a> {
    storage: TokenStorage<'a>,
}

impl<'a> GoogleAuth<'a> {
    /// Create a new Google auth manager.
    pub fn new(db: &'a Database) -> Self {
        Self {
            storage: TokenStorage::new(db),
        }
    }

    /// Get stored tokens.
    pub fn get_tokens(&self) -> Result<Option<StoredTokens>, GoogleAuthError> {
        Ok(self.storage.load(PROVIDER)?)
    }

    /// Save tokens from OAuth response.
    pub fn save_tokens(
        &self,
        tokens: &TokenResponse,
        project_id: Option<&str>,
    ) -> Result<(), GoogleAuthError> {
        self.storage.save(
            PROVIDER,
            &tokens.access_token,
            tokens.refresh_token.as_deref(),
            tokens.expires_in,
            project_id, // Store project_id in account_id field
            None,
        )?;
        Ok(())
    }

    /// Refresh tokens if needed.
    pub async fn refresh_if_needed(&self) -> Result<String, GoogleAuthError> {
        let tokens = self
            .storage
            .load(PROVIDER)?
            .ok_or(GoogleAuthError::NotAuthenticated)?;

        // Refresh if expired or expiring within 5 minutes
        if tokens.expires_within(300) {
            if let Some(refresh_token) = &tokens.refresh_token {
                let config = google_oauth_config();
                let new_tokens = oauth_refresh_token(&config, refresh_token).await?;
                // Preserve the project_id (stored in account_id)
                self.save_tokens(&new_tokens, tokens.account_id.as_deref())?;
                return Ok(new_tokens.access_token);
            }
            // No refresh token and expired
            if tokens.is_expired() {
                return Err(GoogleAuthError::NotAuthenticated);
            }
        }

        Ok(tokens.access_token)
    }
}

// ============================================================================
// Onboarding Logic
// ============================================================================

/// Default fallback project ID when Antigravity doesn't return one
/// (e.g., for business/workspace accounts)
const ANTIGRAVITY_DEFAULT_PROJECT_ID: &str = "rising-fact-p41fc";

/// Fetch the Antigravity Project ID via loadCodeAssist API.
///
/// Uses the same approach as opencode-antigravity-auth:
/// 1. Try loadCodeAssist on prod, daily, and autopush endpoints
/// 2. Fall back to default project ID if all fail
async fn fetch_project_id(access_token: &str) -> Result<String, GoogleAuthError> {
    info!("Fetching Antigravity Project ID via loadCodeAssist...");

    let client = reqwest::Client::new();

    // Endpoints in order: prod first (best for managed project resolution), then fallbacks
    let endpoints = [
        "https://cloudcode-pa.googleapis.com",
        "https://daily-cloudcode-pa.sandbox.googleapis.com",
        "https://autopush-cloudcode-pa.sandbox.googleapis.com",
    ];

    // Required headers to match CLIProxy/Vibeproxy behavior
    let client_metadata =
        r#"{"ideType":"IDE_UNSPECIFIED","platform":"PLATFORM_UNSPECIFIED","pluginType":"GEMINI"}"#;

    // Request body for loadCodeAssist
    let payload = serde_json::json!({
        "metadata": {
            "ideType": "IDE_UNSPECIFIED",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI"
        }
    });

    let mut errors = Vec::new();

    for base_endpoint in endpoints {
        let url = format!("{}/v1internal:loadCodeAssist", base_endpoint);
        debug!("Trying loadCodeAssist at: {}", url);

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .header("User-Agent", "google-api-nodejs-client/9.15.1")
            .header(
                "X-Goog-Api-Client",
                "google-cloud-sdk vscode_cloudshelleditor/0.1",
            )
            .header("Client-Metadata", client_metadata)
            .body(payload.to_string())
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        debug!("loadCodeAssist response: {:?}", json);

                        // Look for cloudaicompanionProject field
                        if let Some(project_id) = extract_project_id(&json) {
                            info!("Successfully fetched Project ID: {}", project_id);
                            return Ok(project_id);
                        }
                        errors.push(format!(
                            "loadCodeAssist missing project id at {}",
                            base_endpoint
                        ));
                    }
                    Err(e) => {
                        errors.push(format!(
                            "loadCodeAssist parse error at {}: {}",
                            base_endpoint, e
                        ));
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                errors.push(format!(
                    "loadCodeAssist {} at {}: {}",
                    status, base_endpoint, text
                ));
            }
            Err(e) => {
                errors.push(format!("loadCodeAssist error at {}: {}", base_endpoint, e));
            }
        }
    }

    // Log all errors for debugging
    if !errors.is_empty() {
        warn!(
            "Failed to resolve Antigravity project via loadCodeAssist: {}",
            errors.join("; ")
        );
    }

    // Use default fallback project ID
    info!(
        "Using default Antigravity project ID: {}",
        ANTIGRAVITY_DEFAULT_PROJECT_ID
    );
    Ok(ANTIGRAVITY_DEFAULT_PROJECT_ID.to_string())
}

/// Extract project ID from loadCodeAssist response
fn extract_project_id(json: &serde_json::Value) -> Option<String> {
    // Primary: cloudaicompanionProject as string
    if let Some(id) = json.get("cloudaicompanionProject").and_then(|v| v.as_str()) {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    // Alternative: cloudaicompanionProject.id as nested object
    if let Some(project) = json.get("cloudaicompanionProject") {
        if let Some(id) = project.get("id").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }

    // Fallback: project_id or projectId at root
    if let Some(id) = json
        .get("project_id")
        .or_else(|| json.get("projectId"))
        .and_then(|v| v.as_str())
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    None
}

// ============================================================================
// Model Registration
// ============================================================================

fn save_google_models_to_db(db: &Database, project_id: &str) -> Result<(), std::io::Error> {
    use crate::models::ModelRegistry;

    // List of models to register
    let models = vec![
        "gemini-3-pro-low",
        "gemini-3-pro-high",
        "gemini-3-flash",
        "claude-sonnet-4-5",
        "claude-sonnet-4-5-thinking-low",
        "claude-sonnet-4-5-thinking-medium",
        "claude-sonnet-4-5-thinking-high",
        "claude-opus-4-5-thinking-low",
        "claude-opus-4-5-thinking-medium",
        "claude-opus-4-5-thinking-high",
    ];

    println!("💾 Saving {} Google models to database...", models.len());

    for model_name in models {
        // Create prefixed name
        let prefixed = format!("google-{}", model_name);

        // Determine capabilities
        let supports_thinking = model_name.contains("thinking");
        let supports_vision = true; // Most Gemini/Claude models via Google support vision

        let config = ModelConfig {
            name: prefixed.clone(),
            model_type: ModelType::GoogleVertex, // Assuming we use Vertex type since we have project_id
            model_id: Some(model_name.to_string()),
            context_length: 1_000_000, // Large context for Gemini 1.5/2.0+
            supports_thinking,
            supports_vision,
            supports_tools: true,
            description: Some(format!("Google Model (Project: {})", project_id)),
            custom_endpoint: None,
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        match ModelRegistry::add_model_to_db(db, &config) {
            Ok(()) => println!("   ✓ Saved: {}", prefixed),
            Err(e) => {
                println!("   ✗ FAILED to save {}: {}", prefixed, e);
                error!(model = %prefixed, error = %e, "Failed to save model");
            }
        }
    }

    Ok(())
}

// ============================================================================
// Auth Flow
// ============================================================================

/// Run the Google OAuth flow (prints to stdout).
pub async fn run_google_auth(db: &Database) -> Result<(), GoogleAuthError> {
    run_google_auth_with_progress(db, &super::StdoutProgress).await
}

/// Run the Google OAuth flow with custom progress reporting.
pub async fn run_google_auth_with_progress(
    db: &Database,
    progress: &impl super::AuthProgress,
) -> Result<(), GoogleAuthError> {
    progress.info("🔐 Starting Google OAuth authentication...");

    let config = google_oauth_config();
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
    progress.success("✅ OAuth tokens received.");

    // Fetch Project ID via loadCodeAssist API
    progress.info("🚀 Fetching Antigravity Project ID...");
    let project_id = match fetch_project_id(&tokens.access_token).await {
        Ok(id) => {
            if id == ANTIGRAVITY_DEFAULT_PROJECT_ID {
                progress.info(&format!("📋 Using default Antigravity project: {}", id));
            } else {
                progress.success(&format!("✅ Got Project ID: {}", id));
            }
            id
        }
        Err(e) => {
            // This shouldn't happen since fetch_project_id now has fallback
            progress.warning(&format!("⚠️  Failed to fetch Project ID: {}", e));
            progress.info("   Using default project ID.");
            ANTIGRAVITY_DEFAULT_PROJECT_ID.to_string()
        }
    };

    let auth = GoogleAuth::new(db);
    auth.save_tokens(&tokens, Some(&project_id))?;

    progress.success("✅ Authentication successful!");

    // Register models with the project ID
    if let Err(e) = save_google_models_to_db(db, &project_id) {
        progress.warning(&format!("⚠️  Failed to save models: {}", e));
    }

    progress.info("");
    progress.success("🎉 Google authentication complete!");
    progress.info("   Use /model to select a google-* model.");

    Ok(())
}

// ============================================================================
// Model Retrieval
// ============================================================================

/// Transform model name to API format for Antigravity.
///
/// For Antigravity API:
/// - gemini-3-pro-{low,high}: Keep the tier suffix (API requires it)
/// - gemini-3-flash-{tier}: Strip the tier, use thinkingLevel param
/// - claude-*-thinking-{tier}: Strip the tier, use thinking_budget param
///
/// Returns (api_model_name, thinking_budget, thinking_level)
fn transform_model_name(model_name: &str) -> (String, Option<u64>, Option<String>) {
    let lower = model_name.to_lowercase();
    let tiers = [
        ("-high", 32768u64, "high"),
        ("-medium", 16384u64, "medium"),
        ("-low", 8192u64, "low"),
    ];

    // Gemini 3 Pro: KEEP the tier suffix (API requires it)
    if lower.starts_with("gemini-3-pro") {
        for (suffix, _budget, level) in tiers {
            if model_name.ends_with(suffix) {
                // Keep full name like gemini-3-pro-low
                return (model_name.to_string(), None, Some(level.to_string()));
            }
        }
        // No tier? Default to -low
        return (format!("{}-low", model_name), None, Some("low".to_string()));
    }

    // Gemini 3 Flash: Strip tier, use thinkingLevel
    if lower.starts_with("gemini-3-flash") {
        for (suffix, _, level) in tiers {
            if model_name.ends_with(suffix) {
                let base = model_name.strip_suffix(suffix).unwrap();
                return (base.to_string(), None, Some(level.to_string()));
            }
        }
        // No tier, use default low
        return (model_name.to_string(), None, Some("low".to_string()));
    }

    // Claude thinking models: Strip tier, use thinking_budget
    if lower.contains("claude") && lower.contains("thinking") {
        for (suffix, budget, _) in tiers {
            if model_name.ends_with(suffix) {
                let base = model_name.strip_suffix(suffix).unwrap();
                return (base.to_string(), Some(budget), None);
            }
        }
        // No tier, default to high budget for Claude
        return (model_name.to_string(), Some(32768), None);
    }

    // Non-thinking model
    (model_name.to_string(), None, None)
}

/// Get a Google/Antigravity model, refreshing tokens if needed.
pub async fn get_google_model(
    db: &Database,
    model_name: &str,
) -> Result<AntigravityModel, GoogleAuthError> {
    let auth = GoogleAuth::new(db);
    let access_token = auth.refresh_if_needed().await?;

    let tokens = auth
        .get_tokens()?
        .ok_or(GoogleAuthError::NotAuthenticated)?;

    let project_id = tokens
        .account_id
        .ok_or_else(|| GoogleAuthError::Onboarding("No Project ID found in storage".to_string()))?;

    // Strip google- prefix if present
    let model_without_prefix = model_name.strip_prefix("google-").unwrap_or(model_name);

    // Transform model name (get API model name, thinking budget, thinking level)
    let (api_model_name, thinking_budget, thinking_level) =
        transform_model_name(model_without_prefix);

    debug!(
        original = %model_name,
        api_model = %api_model_name,
        thinking_budget = ?thinking_budget,
        thinking_level = ?thinking_level,
        "Transformed model name for Antigravity API"
    );

    // Use Antigravity model which supports Google's Cloud Code API
    let model = AntigravityModel::new(&api_model_name, access_token, project_id);

    // Enable thinking with appropriate config
    let model = if let Some(budget) = thinking_budget {
        // Claude uses thinking_budget
        model.with_thinking(Some(budget))
    } else if let Some(level) = thinking_level {
        // Gemini 3 uses thinkingLevel
        model.with_thinking_level(level)
    } else {
        model
    };

    Ok(model)
}
