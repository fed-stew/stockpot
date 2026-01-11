//! ChatGPT OAuth authentication.

use super::storage::{StoredTokens, TokenStorage, TokenStorageError};
use crate::db::Database;
use crate::models::{ModelConfig, ModelType};
use base64::Engine;
use serdes_ai_models::chatgpt_oauth::ChatGptOAuthModel;
use serdes_ai_providers::oauth::{
    config::chatgpt_oauth_config, refresh_token as oauth_refresh_token, run_pkce_flow, OAuthError,
    TokenResponse,
};
use thiserror::Error;
use tracing::{error, info};

const PROVIDER: &str = "chatgpt";

/// Extract account ID from JWT token claims.
/// The account ID is in the `https://api.openai.com/auth` claim object
/// under the `chatgpt_account_id` field.
fn extract_account_id_from_token(id_token: &str) -> Option<String> {
    // JWT format: header.payload.signature
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // Decode the payload (second part)
    let payload = parts[1];

    // Use URL-safe base64 decoding (try without padding first, then with)
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| {
            // Add padding if needed
            let padded = match payload.len() % 4 {
                2 => format!("{}==", payload),
                3 => format!("{}=", payload),
                _ => payload.to_string(),
            };
            base64::engine::general_purpose::URL_SAFE.decode(&padded)
        })
        .ok()?;

    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    // The correct claim structure per Python code:
    // claims["https://api.openai.com/auth"]["chatgpt_account_id"]
    if let Some(auth) = claims.get("https://api.openai.com/auth") {
        if let Some(account_id) = auth.get("chatgpt_account_id").and_then(|v| v.as_str()) {
            if !account_id.is_empty() {
                tracing::debug!(account_id = %account_id, "Found chatgpt_account_id in JWT");
                return Some(account_id.to_string());
            }
        }
    }

    // Fallback: Try organizations array like Python does
    if let Some(auth) = claims.get("https://api.openai.com/auth") {
        if let Some(organizations) = auth.get("organizations").and_then(|v| v.as_array()) {
            // Find default org, or use first one
            let org = organizations
                .iter()
                .find(|o| {
                    o.get("is_default")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                })
                .or_else(|| organizations.first());

            if let Some(org) = org {
                if let Some(org_id) = org.get("id").and_then(|v| v.as_str()) {
                    tracing::debug!(org_id = %org_id, "Found org_id in JWT organizations");
                    return Some(org_id.to_string());
                }
            }
        }
    }

    // Last fallback: top-level organization_id
    if let Some(org_id) = claims.get("organization_id").and_then(|v| v.as_str()) {
        tracing::debug!(org_id = %org_id, "Found top-level organization_id in JWT");
        return Some(org_id.to_string());
    }

    tracing::warn!("Could not find chatgpt_account_id in JWT claims");
    None
}

#[derive(Debug, Error)]
pub enum ChatGptAuthError {
    #[error("OAuth error: {0}")]
    OAuth(#[from] OAuthError),
    #[error("Storage error: {0}")]
    Storage(#[from] TokenStorageError),
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Browser error: {0}")]
    Browser(String),
}

/// ChatGPT authentication manager.
pub struct ChatGptAuth<'a> {
    storage: TokenStorage<'a>,
}

impl<'a> ChatGptAuth<'a> {
    /// Create a new ChatGPT auth manager.
    pub fn new(db: &'a Database) -> Self {
        Self {
            storage: TokenStorage::new(db),
        }
    }

    /// Get stored tokens.
    pub fn get_tokens(&self) -> Result<Option<StoredTokens>, ChatGptAuthError> {
        Ok(self.storage.load(PROVIDER)?)
    }

    /// Save tokens from OAuth response.
    pub fn save_tokens(&self, tokens: &TokenResponse) -> Result<(), ChatGptAuthError> {
        // Try to extract account_id from id_token if available
        let account_id = tokens
            .id_token
            .as_ref()
            .and_then(|id_token| extract_account_id_from_token(id_token));

        if account_id.is_some() {
            info!("Extracted account_id from OAuth token");
        }

        self.storage.save(
            PROVIDER,
            &tokens.access_token,
            tokens.refresh_token.as_deref(),
            tokens.expires_in,
            account_id.as_deref(),
            None,
        )?;
        Ok(())
    }

    /// Refresh tokens if needed.
    pub async fn refresh_if_needed(&self) -> Result<String, ChatGptAuthError> {
        let tokens = self
            .storage
            .load(PROVIDER)?
            .ok_or(ChatGptAuthError::NotAuthenticated)?;

        // Refresh if expired or expiring within 5 minutes
        if tokens.expires_within(300) {
            if let Some(refresh_token) = &tokens.refresh_token {
                let config = chatgpt_oauth_config();
                let new_tokens = oauth_refresh_token(&config, refresh_token).await?;
                self.save_tokens(&new_tokens)?;
                return Ok(new_tokens.access_token);
            }
            // No refresh token and expired
            if tokens.is_expired() {
                return Err(ChatGptAuthError::NotAuthenticated);
            }
        }

        Ok(tokens.access_token)
    }

}

// ============================================================================
// Model definitions
// ============================================================================

/// Get the list of known ChatGPT models available via OAuth.
///
/// We use a hardcoded list because the ChatGPT OAuth token lacks the
/// `api.model.read` scope required to call `/v1/models`.
fn known_chatgpt_models() -> Vec<String> {
    vec!["gpt-5.2".to_string(), "gpt-5.2-codex".to_string()]
}

/// Save ChatGPT models to database
fn save_chatgpt_models_to_db(db: &Database, models: &[String]) -> Result<(), std::io::Error> {
    use crate::models::ModelRegistry;

    println!("💾 Saving {} models to database...", models.len());
    let mut success_count = 0;
    let mut fail_count = 0;

    for model_name in models {
        // Create prefixed name like "chatgpt-gpt-4o"
        let prefixed = format!("chatgpt-{}", model_name);

        // Determine capabilities based on model name
        let supports_thinking = model_name.starts_with("o1")
            || model_name.starts_with("o3")
            || model_name.starts_with("o4")
            || model_name.contains("gpt-5"); // GPT-5 likely supports thinking

        let supports_vision = model_name.contains("gpt-4")
            || model_name.contains("gpt-4o")
            || model_name.contains("gpt-5") // GPT-5 likely supports vision
            || model_name.starts_with("o1")
            || model_name.starts_with("o3")
            || model_name.starts_with("o4");

        // Context length varies by model
        // GPT-5 and newer models likely have larger context windows
        let context_length = if model_name.contains("gpt-5") {
            270_000 // GPT-5 has 270k context
        } else if model_name.contains("gpt-4o") {
            128_000
        } else if model_name.starts_with("o1")
            || model_name.starts_with("o3")
            || model_name.starts_with("o4")
        {
            200_000
        } else if model_name.contains("gpt-4-turbo") || model_name.contains("gpt-4-1106") {
            128_000
        } else if model_name.contains("gpt-4-32k") {
            32_768
        } else if model_name.contains("gpt-4") {
            8_192
        } else {
            16_384 // Default for gpt-3.5-turbo, gpt-3.5-turbo-16k, etc.
        };

        let config = ModelConfig {
            name: prefixed.clone(),
            model_type: ModelType::ChatgptOauth,
            model_id: Some(model_name.clone()),
            context_length,
            supports_thinking,
            supports_vision,
            supports_tools: true,
            description: Some(format!("ChatGPT OAuth: {}", model_name)),
            custom_endpoint: None,
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        match ModelRegistry::add_model_to_db(db, &config) {
            Ok(()) => {
                println!("   ✓ Saved: {}", prefixed);
                success_count += 1;
            }
            Err(e) => {
                println!("   ✗ FAILED to save {}: {}", prefixed, e);
                error!(model = %prefixed, error = %e, "Failed to save model");
                fail_count += 1;
            }
        }
    }

    println!(
        "📊 Save complete: {} succeeded, {} failed",
        success_count, fail_count
    );
    info!(
        "Saved {} ChatGPT models to database ({} failed)",
        success_count, fail_count
    );

    if fail_count > 0 && success_count == 0 {
        return Err(std::io::Error::other("All model saves failed"));
    }

    Ok(())
}

/// Run the ChatGPT OAuth flow.
pub async fn run_chatgpt_auth(db: &Database) -> Result<(), ChatGptAuthError> {
    println!("🔐 Starting ChatGPT OAuth authentication...");

    let config = chatgpt_oauth_config();
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

    let auth = ChatGptAuth::new(db);
    auth.save_tokens(&tokens)?;

    println!("✅ Authentication successful!");

    // Use hardcoded list of known ChatGPT models
    // (OAuth token lacks api.model.read scope to fetch from API)
    let models = known_chatgpt_models();
    println!("📋 Using {} known ChatGPT models", models.len());

    // Save models to database
    match save_chatgpt_models_to_db(db, &models) {
        Ok(()) => {
            println!("✅ Registered {} ChatGPT models:", models.len());
            for model in &models {
                println!("   • chatgpt-{}", model);
            }
        }
        Err(e) => {
            println!("⚠️  Failed to save models: {}", e);
        }
    }

    // Verify models were actually saved by querying the database
    println!();
    println!("🔍 Verifying saved models in database...");
    match db
        .conn()
        .prepare("SELECT name, model_type FROM models WHERE model_type = 'chatgpt_oauth'")
    {
        Ok(mut stmt) => {
            let rows: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map(|iter| iter.flatten().collect())
                .unwrap_or_default();
            println!("📊 Found {} chatgpt_oauth models in database:", rows.len());
            for name in &rows {
                println!("   • {}", name);
            }
            if rows.is_empty() {
                println!("❌ WARNING: No models found in database after save!");
                println!("   This suggests the INSERT is failing silently.");
            }
        }
        Err(e) => println!("❌ Failed to verify: {}", e),
    }

    println!();
    println!("🎉 ChatGPT authentication complete!");
    println!("   Use /model to select a chatgpt-* model.");

    Ok(())
}

/// Get a ChatGPT OAuth model, refreshing tokens if needed.
pub async fn get_chatgpt_model(
    db: &Database,
    model_name: &str,
) -> Result<ChatGptOAuthModel, ChatGptAuthError> {
    let auth = ChatGptAuth::new(db);
    let access_token = auth.refresh_if_needed().await?;

    // Get account_id from stored tokens
    let tokens = auth
        .get_tokens()?
        .ok_or(ChatGptAuthError::NotAuthenticated)?;

    let mut model = ChatGptOAuthModel::new(model_name, access_token);

    // Add account_id if available (required by Codex API)
    if let Some(account_id) = tokens.account_id {
        model = model.with_account_id(account_id);
    } else {
        // Log warning - API calls may fail without account_id
        tracing::warn!("No account_id found for ChatGPT OAuth - API calls may return 403");
    }

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_chatgpt_models() {
        let models = known_chatgpt_models();
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"gpt-5.2".to_string()));
        assert!(models.contains(&"gpt-5.2-codex".to_string()));
    }
}
