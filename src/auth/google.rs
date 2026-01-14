//! Google OAuth authentication.

use super::storage::{StoredTokens, TokenStorage, TokenStorageError};
use crate::db::Database;
use crate::models::{ModelConfig, ModelType};
use serdes_ai_models::google::GoogleModel;
use serdes_ai_providers::oauth::{
    config::google_oauth_config, refresh_token as oauth_refresh_token, run_pkce_flow, OAuthError,
    TokenResponse,
};
use thiserror::Error;
use tracing::{debug, error, info};

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

/// Fetch the Antigravity Project ID by onboarding the user.
async fn fetch_project_id(access_token: &str) -> Result<String, GoogleAuthError> {
    info!("Fetching Antigravity Project ID...");

    let client = reqwest::Client::new();
    // Use the sandbox endpoint as specified
    let url = "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:onboardUser";

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        error!("Onboarding failed: {} - {}", status, text);
        return Err(GoogleAuthError::Onboarding(format!(
            "Failed to onboard user: {}",
            status
        )));
    }

    // Try to parse as JSON first
    let json: serde_json::Value = response.json().await?;
    debug!("Onboarding response: {:?}", json);

    // Look for project_id or similar field
    // Based on typical Google APIs, it might be camelCase or snake_case
    if let Some(project_id) = json
        .get("project_id")
        .or_else(|| json.get("projectId"))
        .and_then(|v| v.as_str())
    {
        info!("Successfully fetched Project ID: {}", project_id);
        return Ok(project_id.to_string());
    }

    // If direct field not found, maybe check nested fields if we knew the structure
    // For now, fail if not found
    Err(GoogleAuthError::Onboarding(
        "Project ID not found in onboarding response".to_string(),
    ))
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

/// Run the Google OAuth flow.
pub async fn run_google_auth(db: &Database) -> Result<(), GoogleAuthError> {
    println!("🔐 Starting Google OAuth authentication...");

    let config = google_oauth_config();
    let (auth_url, handle) = run_pkce_flow(&config).await?;

    println!("📋 Open this URL in your browser:");
    println!("   {}", auth_url);
    println!();
    println!(
        "⏳ Waiting for authentication callback on port {}...",
        handle.port()
    );

    // Try to open browser
    if let Err(e) = webbrowser::open(&auth_url) {
        println!("⚠️  Could not open browser automatically: {}", e);
        println!("   Please open the URL manually.");
    }

    let tokens = handle.wait_for_tokens().await?;
    println!("✅ OAuth tokens received.");

    // Onboard user to get Project ID
    println!("🚀 Onboarding user to fetch Project ID...");
    let project_id = match fetch_project_id(&tokens.access_token).await {
        Ok(id) => id,
        Err(e) => {
            println!("⚠️  Failed to fetch Project ID: {}", e);
            println!("   Tokens will be saved without Project ID, but models may not work.");
            String::new()
        }
    };

    let auth = GoogleAuth::new(db);
    // Store project_id if we got one
    let project_id_opt = if project_id.is_empty() {
        None
    } else {
        Some(project_id.as_str())
    };
    auth.save_tokens(&tokens, project_id_opt)?;

    println!("✅ Authentication successful!");

    if !project_id.is_empty() {
        // Register models
        if let Err(e) = save_google_models_to_db(db, &project_id) {
            println!("⚠️  Failed to save models: {}", e);
        }
    } else {
        println!("⚠️  Skipping model registration due to missing Project ID.");
    }

    println!();
    println!("🎉 Google authentication complete!");
    println!("   Use /model to select a google-* model.");

    Ok(())
}

// ============================================================================
// Model Retrieval
// ============================================================================

/// Get a Google model, refreshing tokens if needed.
pub async fn get_google_model(
    db: &Database,
    model_name: &str,
) -> Result<GoogleModel, GoogleAuthError> {
    let auth = GoogleAuth::new(db);
    let access_token = auth.refresh_if_needed().await?;

    let tokens = auth
        .get_tokens()?
        .ok_or(GoogleAuthError::NotAuthenticated)?;

    let project_id = tokens
        .account_id
        .ok_or_else(|| GoogleAuthError::Onboarding("No Project ID found in storage".to_string()))?;

    // Strip prefix if present
    let actual_model_name = model_name.strip_prefix("google-").unwrap_or(model_name);

    // Use Vertex constructor since we have a project ID
    // Location is typically us-central1 for Antigravity, but could be configurable
    // For now hardcode or use a default
    let location = "us-central1";

    // Note: The `GoogleModel::vertex` constructor takes (model_name, project_id, location)
    // However, we also need to pass the OAuth token.
    // The `GoogleModel` struct in serdes-ai-models might not have a method to set the token directly if it expects ADC or similar?
    // Let's check the GoogleModel source again.
    // It has `api_key` but doesn't seem to have an explicit `oauth_token` field in `new` or `vertex`.
    // Wait, the `GoogleModel` source I read earlier had:
    // pub struct GoogleModel { ... api_key: Option<String>, ... }
    // It didn't explicitly show a Bearer token field.
    // However, looking at `request` method:
    // `format!("... key={}", self.api_key ...)` for non-Vertex.
    // For Vertex: `format!("{}/v1/projects/{}/...`
    // It says "For Vertex AI, would need OAuth token".
    // "For now, API key is in URL for Google AI".

    // If the `GoogleModel` in `serdes-ai-models` doesn't support passing an OAuth token, I might need to
    // subclass it or use a custom client setup.
    // BUT, since `GoogleModel` has `with_client`, I can configure the reqwest client to include the Authorization header!
    // Or I can use the `api_key` field as the token if the implementation allows (unlikely).

    // Let's assume I can use `with_client` or similar, OR I might need to check if `serdes-ai-models` has been updated to support OAuth tokens.
    // The source I read was:
    // `// For Vertex AI, would need OAuth token`
    // `// For now, API key is in URL for Google AI`

    // This implies `serdes-ai-models` might NOT support OAuth token injection out of the box for Vertex yet?
    // Wait, `stockpot` depends on `serdes-ai-models`.
    // If `serdes-ai-models` doesn't support it, I might be blocked or need to patch it.

    // However, `GoogleModel` has `with_client(mut self, client: Client)`.
    // I can create a `Client` that includes the default headers!

    let mut headers = reqwest::header::HeaderMap::new();
    let mut auth_val = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|e| GoogleAuthError::Onboarding(e.to_string()))?;
    auth_val.set_sensitive(true);
    headers.insert(reqwest::header::AUTHORIZATION, auth_val);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let model = GoogleModel::vertex(actual_model_name, project_id, location).with_client(client); // Inject authenticated client

    Ok(model)
}
