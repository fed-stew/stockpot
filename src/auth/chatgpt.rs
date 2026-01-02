//! ChatGPT OAuth authentication.

use super::storage::{StoredTokens, TokenStorage, TokenStorageError};
use crate::db::Database;
use serdes_ai_models::chatgpt_oauth::ChatGptOAuthModel;
use serdes_ai_providers::oauth::{
    config::chatgpt_oauth_config, refresh_token as oauth_refresh_token, run_pkce_flow, OAuthError,
    TokenResponse,
};
use thiserror::Error;

const PROVIDER: &str = "chatgpt";

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

    /// Check if authenticated.
    pub fn is_authenticated(&self) -> Result<bool, ChatGptAuthError> {
        Ok(self.storage.is_authenticated(PROVIDER)?)
    }

    /// Get stored tokens.
    pub fn get_tokens(&self) -> Result<Option<StoredTokens>, ChatGptAuthError> {
        Ok(self.storage.load(PROVIDER)?)
    }

    /// Save tokens from OAuth response.
    pub fn save_tokens(&self, tokens: &TokenResponse) -> Result<(), ChatGptAuthError> {
        self.storage.save(
            PROVIDER,
            &tokens.access_token,
            tokens.refresh_token.as_deref(),
            tokens.expires_in,
            None, // account_id not in standard response
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

    /// Delete stored tokens (logout).
    pub fn logout(&self) -> Result<(), ChatGptAuthError> {
        self.storage.delete(PROVIDER)?;
        Ok(())
    }
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

    println!("✅ ChatGPT authentication successful!");
    println!("   You can now use chatgpt-* models.");

    Ok(())
}

/// Get a ChatGPT OAuth model, refreshing tokens if needed.
pub async fn get_chatgpt_model(
    db: &Database,
    model_name: &str,
) -> Result<ChatGptOAuthModel, ChatGptAuthError> {
    let auth = ChatGptAuth::new(db);
    let access_token = auth.refresh_if_needed().await?;
    Ok(ChatGptOAuthModel::new(model_name, access_token))
}
