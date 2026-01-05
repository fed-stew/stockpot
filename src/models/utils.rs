//! Utility functions for model configuration.
//!
//! This module provides helper functions for:
//! - Parsing model types from strings
//! - Building custom endpoints from database fields
//! - Checking API key and OAuth token availability
//! - Resolving environment variables

use std::collections::HashMap;

use crate::auth::TokenStorage;
use crate::db::Database;

use super::types::{CustomEndpoint, ModelConfigError, ModelType};

/// Parse a model type string from the database.
pub fn parse_model_type(s: &str) -> ModelType {
    match s {
        "openai" => ModelType::Openai,
        "anthropic" => ModelType::Anthropic,
        "gemini" => ModelType::Gemini,
        "custom_openai" => ModelType::CustomOpenai,
        "custom_anthropic" => ModelType::CustomAnthropic,
        "claude_code" => ModelType::ClaudeCode,
        "chatgpt_oauth" => ModelType::ChatgptOauth,
        "azure_openai" => ModelType::AzureOpenai,
        "openrouter" => ModelType::Openrouter,
        "round_robin" => ModelType::RoundRobin,
        _ => ModelType::CustomOpenai,
    }
}

/// Build a CustomEndpoint from database fields.
pub fn build_custom_endpoint(
    url: Option<String>,
    api_key: Option<String>,
    headers_json: Option<String>,
) -> Option<CustomEndpoint> {
    let url = url?;
    Some(CustomEndpoint {
        url,
        api_key,
        headers: headers_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
        ca_certs_path: None,
    })
}

/// Check if an API key is available (in database or environment).
pub fn has_api_key(db: &Database, key_name: &str) -> bool {
    db.has_api_key(key_name) || std::env::var(key_name).is_ok()
}

/// Check if valid OAuth tokens exist for a provider.
/// Returns true if tokens exist and are not expired (or have a refresh token).
pub fn has_oauth_tokens(db: &Database, provider: &str) -> bool {
    let storage = TokenStorage::new(db);
    let result = match storage.load(provider) {
        Ok(Some(tokens)) => {
            // Tokens exist - check if valid or refreshable
            let is_expired = tokens.is_expired();
            let has_refresh = tokens.refresh_token.is_some();
            let valid = if is_expired { has_refresh } else { true };
            tracing::debug!(
                provider = %provider,
                is_expired = is_expired,
                has_refresh_token = has_refresh,
                result = valid,
                "OAuth token check"
            );
            valid
        }
        Ok(None) => {
            tracing::debug!(provider = %provider, "No OAuth tokens found");
            false
        }
        Err(e) => {
            tracing::debug!(provider = %provider, error = %e, "OAuth token load error");
            false
        }
    };
    result
}

/// Resolve an API key, checking database first, then environment.
/// Returns None if the key is not found in either location.
pub fn resolve_api_key(db: &Database, key_name: &str) -> Option<String> {
    // First check database
    if let Ok(Some(key)) = db.get_api_key(key_name) {
        return Some(key);
    }
    // Fall back to environment variable
    std::env::var(key_name).ok()
}

/// Resolve environment variable references in a string.
///
/// Supports both `$VAR` and `${VAR}` syntax.
///
/// # Examples
/// ```ignore
/// let resolved = resolve_env_var("Bearer $API_KEY").unwrap();
/// let resolved = resolve_env_var("${HOME}/config").unwrap();
/// ```
pub fn resolve_env_var(input: &str) -> Result<String, ModelConfigError> {
    // Use shellexpand which handles both $VAR and ${VAR}
    shellexpand::full(input)
        .map(|s| s.into_owned())
        .map_err(|e| ModelConfigError::EnvVarNotFound(e.var_name))
}

/// Resolve all environment variables in a CustomEndpoint.
pub fn resolve_endpoint_env_vars(
    endpoint: &CustomEndpoint,
) -> Result<CustomEndpoint, ModelConfigError> {
    let mut resolved = endpoint.clone();

    resolved.url = resolve_env_var(&endpoint.url)?;

    if let Some(ref api_key) = endpoint.api_key {
        resolved.api_key = Some(resolve_env_var(api_key)?);
    }

    if let Some(ref ca_path) = endpoint.ca_certs_path {
        resolved.ca_certs_path = Some(resolve_env_var(ca_path)?);
    }

    let mut resolved_headers = HashMap::new();
    for (key, value) in &endpoint.headers {
        resolved_headers.insert(key.clone(), resolve_env_var(value)?);
    }
    resolved.headers = resolved_headers;

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_var() {
        std::env::set_var("PUPPY_TEST_VAR", "woof");

        // Test ${VAR} syntax (recommended for embedding)
        let result = resolve_env_var("prefix_${PUPPY_TEST_VAR}_suffix");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "prefix_woof_suffix");

        // Test $VAR at end of string
        let result = resolve_env_var("bark_$PUPPY_TEST_VAR");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "bark_woof");

        // Test non-existent var returns error
        let result = resolve_env_var("${NONEXISTENT_PUPPY_VAR_XYZ}");
        assert!(result.is_err());

        std::env::remove_var("PUPPY_TEST_VAR");
    }
}
