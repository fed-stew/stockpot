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

use super::key_pool::ApiKeyPoolManager;
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
        "google_vertex" => ModelType::GoogleVertex,
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
/// DEPRECATED: Use `has_any_api_key` for unified pool + legacy support.
pub fn has_api_key(db: &Database, key_name: &str) -> bool {
    db.has_api_key(key_name) || std::env::var(key_name).is_ok()
}

/// Check if an API key is available from ANY source.
/// Checks in order:
/// 1. Pool keys (new unified system)
/// 2. Legacy api_keys table (backward compatibility)
/// 3. Environment variables
pub fn has_any_api_key(db: &Database, key_name: &str) -> bool {
    // Check pool first (new unified system)
    if db.has_pool_keys(key_name) {
        return true;
    }
    // Fall back to legacy table
    if db.has_api_key(key_name) {
        return true;
    }
    // Finally check environment
    std::env::var(key_name).is_ok()
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

/// Resolve an API key, checking all sources.
/// Checks in order:
/// 1. Pool keys (new unified system) - returns first active key
/// 2. Legacy api_keys table (backward compatibility)
/// 3. Environment variables
///
/// Returns None if the key is not found in any location.
pub fn resolve_api_key(db: &Database, key_name: &str) -> Option<String> {
    // First check pool (new unified system)
    if let Ok(keys) = db.get_active_pool_keys(key_name) {
        if let Some(first_key) = keys.first() {
            return Some(first_key.api_key.clone());
        }
    }
    // Fall back to legacy single-key table
    if let Ok(Some(key)) = db.get_api_key(key_name) {
        return Some(key);
    }
    // Fall back to environment variable
    std::env::var(key_name).ok()
}

/// Resolve an API key, checking the key pool first for multi-key support.
/// Falls back to single-key storage and environment variables.
///
/// Returns a tuple of (api_key, optional_key_id). The key_id is Some if the key
/// came from the pool (useful for tracking usage and errors).
pub fn resolve_api_key_with_pool(
    db: &Database,
    key_name: &str,
    pool_manager: Option<&ApiKeyPoolManager>,
) -> Option<(String, Option<i64>)> {
    // If pool manager provided, check for pooled keys first
    if let Some(pool) = pool_manager {
        if let Some((key, key_id)) = pool.get_current_key(key_name) {
            return Some((key, Some(key_id)));
        }
    }

    // Fall back to single key resolution (existing behavior)
    resolve_api_key(db, key_name).map(|k| (k, None))
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
    use tempfile::TempDir;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, db)
    }

    // =========================================================================
    // resolve_env_var Tests
    // =========================================================================

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

    // =========================================================================
    // has_api_key Tests
    // =========================================================================

    #[test]
    fn test_has_api_key_in_db() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("MY_TEST_KEY", "secret-value").unwrap();

        assert!(has_api_key(&db, "MY_TEST_KEY"));
    }

    #[test]
    fn test_has_api_key_in_env() {
        let (_temp, db) = setup_test_db();

        // Set env var but not in DB
        std::env::set_var("STOCKPOT_TEST_API_KEY_ENV", "from-env");

        assert!(has_api_key(&db, "STOCKPOT_TEST_API_KEY_ENV"));

        std::env::remove_var("STOCKPOT_TEST_API_KEY_ENV");
    }

    #[test]
    fn test_has_api_key_missing() {
        let (_temp, db) = setup_test_db();

        // Ensure not in env
        std::env::remove_var("NONEXISTENT_STOCKPOT_KEY_XYZ");

        assert!(!has_api_key(&db, "NONEXISTENT_STOCKPOT_KEY_XYZ"));
    }

    #[test]
    fn test_has_api_key_db_takes_precedence() {
        let (_temp, db) = setup_test_db();

        // Key in both DB and env - should still return true
        db.save_api_key("DUAL_KEY", "db-value").unwrap();
        std::env::set_var("DUAL_KEY", "env-value");

        assert!(has_api_key(&db, "DUAL_KEY"));

        std::env::remove_var("DUAL_KEY");
    }

    // =========================================================================
    // has_any_api_key Tests (unified pool + legacy + env)
    // =========================================================================

    #[test]
    fn test_has_any_api_key_from_pool() {
        let (_temp, db) = setup_test_db();
        db.save_pool_key("POOL_KEY_TEST", "sk-pool", None, None)
            .unwrap();

        assert!(has_any_api_key(&db, "POOL_KEY_TEST"));
    }

    #[test]
    fn test_has_any_api_key_from_legacy() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("LEGACY_KEY_TEST", "sk-legacy").unwrap();

        assert!(has_any_api_key(&db, "LEGACY_KEY_TEST"));
    }

    #[test]
    fn test_has_any_api_key_from_env() {
        let (_temp, db) = setup_test_db();
        std::env::set_var("STOCKPOT_ENV_UNIFIED_TEST", "sk-env");

        assert!(has_any_api_key(&db, "STOCKPOT_ENV_UNIFIED_TEST"));

        std::env::remove_var("STOCKPOT_ENV_UNIFIED_TEST");
    }

    #[test]
    fn test_has_any_api_key_missing() {
        let (_temp, db) = setup_test_db();
        std::env::remove_var("NONEXISTENT_UNIFIED_KEY");

        assert!(!has_any_api_key(&db, "NONEXISTENT_UNIFIED_KEY"));
    }

    #[test]
    fn test_has_any_api_key_pool_preferred() {
        let (_temp, db) = setup_test_db();
        // Key in pool should make it return true even without legacy
        db.save_pool_key("POOL_PREFERRED", "sk-pool", None, None)
            .unwrap();

        assert!(has_any_api_key(&db, "POOL_PREFERRED"));
        // Legacy should NOT have this key
        assert!(!db.has_api_key("POOL_PREFERRED"));
    }

    // =========================================================================
    // resolve_api_key Tests
    // =========================================================================

    #[test]
    fn test_resolve_api_key_from_db() {
        let (_temp, db) = setup_test_db();
        db.save_api_key("RESOLVE_DB_KEY", "db-secret").unwrap();

        let result = resolve_api_key(&db, "RESOLVE_DB_KEY");
        assert_eq!(result, Some("db-secret".to_string()));
    }

    #[test]
    fn test_resolve_api_key_from_env() {
        let (_temp, db) = setup_test_db();

        std::env::set_var("STOCKPOT_RESOLVE_ENV_KEY", "env-secret");

        let result = resolve_api_key(&db, "STOCKPOT_RESOLVE_ENV_KEY");
        assert_eq!(result, Some("env-secret".to_string()));

        std::env::remove_var("STOCKPOT_RESOLVE_ENV_KEY");
    }

    #[test]
    fn test_resolve_api_key_missing() {
        let (_temp, db) = setup_test_db();

        std::env::remove_var("NONEXISTENT_RESOLVE_KEY");

        let result = resolve_api_key(&db, "NONEXISTENT_RESOLVE_KEY");
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_api_key_db_preferred_over_env() {
        let (_temp, db) = setup_test_db();

        // Set both DB and env
        db.save_api_key("PRIORITY_KEY", "db-value").unwrap();
        std::env::set_var("PRIORITY_KEY", "env-value");

        let result = resolve_api_key(&db, "PRIORITY_KEY");
        // DB should take precedence
        assert_eq!(result, Some("db-value".to_string()));

        std::env::remove_var("PRIORITY_KEY");
    }

    #[test]
    fn test_resolve_api_key_pool_preferred_over_legacy() {
        let (_temp, db) = setup_test_db();

        // Set both pool and legacy
        db.save_pool_key("POOL_VS_LEGACY", "pool-value", None, None)
            .unwrap();
        db.save_api_key("POOL_VS_LEGACY", "legacy-value").unwrap();

        let result = resolve_api_key(&db, "POOL_VS_LEGACY");
        // Pool should take precedence over legacy
        assert_eq!(result, Some("pool-value".to_string()));
    }

    #[test]
    fn test_resolve_api_key_from_pool() {
        let (_temp, db) = setup_test_db();
        db.save_pool_key("POOL_ONLY_KEY", "pool-secret", None, None)
            .unwrap();

        let result = resolve_api_key(&db, "POOL_ONLY_KEY");
        assert_eq!(result, Some("pool-secret".to_string()));
    }

    // =========================================================================
    // build_custom_endpoint Tests
    // =========================================================================

    #[test]
    fn test_build_custom_endpoint_full_fields() {
        let url = Some("https://api.example.com/v1".to_string());
        let api_key = Some("sk-test-key".to_string());
        let headers_json = Some(r#"{"X-Custom": "value"}"#.to_string());

        let result = build_custom_endpoint(url, api_key, headers_json);

        assert!(result.is_some());
        let endpoint = result.unwrap();
        assert_eq!(endpoint.url, "https://api.example.com/v1");
        assert_eq!(endpoint.api_key, Some("sk-test-key".to_string()));
        assert_eq!(endpoint.headers.get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_build_custom_endpoint_no_url_returns_none() {
        let result = build_custom_endpoint(None, Some("key".to_string()), None);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_custom_endpoint_url_only() {
        let result = build_custom_endpoint(Some("https://api.example.com".to_string()), None, None);

        assert!(result.is_some());
        let endpoint = result.unwrap();
        assert_eq!(endpoint.url, "https://api.example.com");
        assert!(endpoint.api_key.is_none());
        assert!(endpoint.headers.is_empty());
    }

    #[test]
    fn test_build_custom_endpoint_invalid_headers_json_defaults_empty() {
        let result = build_custom_endpoint(
            Some("https://api.example.com".to_string()),
            None,
            Some("not valid json".to_string()),
        );

        assert!(result.is_some());
        let endpoint = result.unwrap();
        // Invalid JSON should result in empty headers (default)
        assert!(endpoint.headers.is_empty());
    }

    #[test]
    fn test_build_custom_endpoint_empty_headers_json() {
        let result = build_custom_endpoint(
            Some("https://api.example.com".to_string()),
            None,
            Some("{}".to_string()),
        );

        assert!(result.is_some());
        let endpoint = result.unwrap();
        assert!(endpoint.headers.is_empty());
    }

    // =========================================================================
    // resolve_endpoint_env_vars Tests
    // =========================================================================

    #[test]
    fn test_resolve_endpoint_env_vars_all_fields() {
        // Set up env vars
        std::env::set_var("STOCKPOT_EP_URL", "https://resolved.example.com");
        std::env::set_var("STOCKPOT_EP_KEY", "resolved-api-key");
        std::env::set_var("STOCKPOT_EP_CA", "/path/to/ca.pem");
        std::env::set_var("STOCKPOT_EP_HEADER", "resolved-header-value");

        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "${STOCKPOT_EP_HEADER}".to_string());

        let endpoint = CustomEndpoint {
            url: "${STOCKPOT_EP_URL}".to_string(),
            api_key: Some("${STOCKPOT_EP_KEY}".to_string()),
            headers,
            ca_certs_path: Some("${STOCKPOT_EP_CA}".to_string()),
        };

        let resolved = resolve_endpoint_env_vars(&endpoint).unwrap();

        assert_eq!(resolved.url, "https://resolved.example.com");
        assert_eq!(resolved.api_key, Some("resolved-api-key".to_string()));
        assert_eq!(resolved.ca_certs_path, Some("/path/to/ca.pem".to_string()));
        assert_eq!(
            resolved.headers.get("X-Custom"),
            Some(&"resolved-header-value".to_string())
        );

        // Cleanup
        std::env::remove_var("STOCKPOT_EP_URL");
        std::env::remove_var("STOCKPOT_EP_KEY");
        std::env::remove_var("STOCKPOT_EP_CA");
        std::env::remove_var("STOCKPOT_EP_HEADER");
    }

    #[test]
    fn test_resolve_endpoint_env_vars_no_env_vars() {
        let endpoint = CustomEndpoint {
            url: "https://literal.example.com".to_string(),
            api_key: Some("literal-key".to_string()),
            headers: HashMap::new(),
            ca_certs_path: None,
        };

        let resolved = resolve_endpoint_env_vars(&endpoint).unwrap();

        assert_eq!(resolved.url, "https://literal.example.com");
        assert_eq!(resolved.api_key, Some("literal-key".to_string()));
    }

    #[test]
    fn test_resolve_endpoint_env_vars_missing_var_errors() {
        let endpoint = CustomEndpoint {
            url: "${NONEXISTENT_STOCKPOT_VAR_XYZ}".to_string(),
            api_key: None,
            headers: HashMap::new(),
            ca_certs_path: None,
        };

        let result = resolve_endpoint_env_vars(&endpoint);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_endpoint_env_vars_partial_resolution() {
        std::env::set_var("STOCKPOT_PARTIAL_KEY", "my-key");

        let endpoint = CustomEndpoint {
            url: "https://api.example.com".to_string(), // literal
            api_key: Some("${STOCKPOT_PARTIAL_KEY}".to_string()), // env var
            headers: HashMap::new(),
            ca_certs_path: None,
        };

        let resolved = resolve_endpoint_env_vars(&endpoint).unwrap();

        assert_eq!(resolved.url, "https://api.example.com");
        assert_eq!(resolved.api_key, Some("my-key".to_string()));

        std::env::remove_var("STOCKPOT_PARTIAL_KEY");
    }

    // =========================================================================
    // parse_model_type Tests
    // =========================================================================

    #[test]
    fn test_parse_model_type_openai() {
        assert!(matches!(parse_model_type("openai"), ModelType::Openai));
    }

    #[test]
    fn test_parse_model_type_anthropic() {
        assert!(matches!(
            parse_model_type("anthropic"),
            ModelType::Anthropic
        ));
    }

    #[test]
    fn test_parse_model_type_gemini() {
        assert!(matches!(parse_model_type("gemini"), ModelType::Gemini));
    }

    #[test]
    fn test_parse_model_type_custom_openai() {
        assert!(matches!(
            parse_model_type("custom_openai"),
            ModelType::CustomOpenai
        ));
    }

    #[test]
    fn test_parse_model_type_custom_anthropic() {
        assert!(matches!(
            parse_model_type("custom_anthropic"),
            ModelType::CustomAnthropic
        ));
    }

    #[test]
    fn test_parse_model_type_claude_code() {
        assert!(matches!(
            parse_model_type("claude_code"),
            ModelType::ClaudeCode
        ));
    }

    #[test]
    fn test_parse_model_type_chatgpt_oauth() {
        assert!(matches!(
            parse_model_type("chatgpt_oauth"),
            ModelType::ChatgptOauth
        ));
    }

    #[test]
    fn test_parse_model_type_azure_openai() {
        assert!(matches!(
            parse_model_type("azure_openai"),
            ModelType::AzureOpenai
        ));
    }

    #[test]
    fn test_parse_model_type_openrouter() {
        assert!(matches!(
            parse_model_type("openrouter"),
            ModelType::Openrouter
        ));
    }

    #[test]
    fn test_parse_model_type_round_robin() {
        assert!(matches!(
            parse_model_type("round_robin"),
            ModelType::RoundRobin
        ));
    }

    #[test]
    fn test_parse_model_type_unknown_defaults_to_custom_openai() {
        assert!(matches!(
            parse_model_type("unknown_type"),
            ModelType::CustomOpenai
        ));
        assert!(matches!(
            parse_model_type("foobar"),
            ModelType::CustomOpenai
        ));
        assert!(matches!(parse_model_type(""), ModelType::CustomOpenai));
    }

    // =========================================================================
    // resolve_api_key_with_pool Tests
    // =========================================================================

    #[test]
    fn test_resolve_api_key_with_pool_from_pool() {
        let (_temp, db) = setup_test_db();
        let db_arc =
            std::sync::Arc::new(Database::open_at(_temp.path().join("test_pool.db")).unwrap());
        db_arc.migrate().unwrap();

        // Add a pool key
        db_arc
            .save_pool_key("OPENAI_API_KEY", "sk-pool-key", None, Some(1))
            .unwrap();

        let pool = super::super::key_pool::ApiKeyPoolManager::with_defaults(db_arc);

        let result = resolve_api_key_with_pool(&db, "OPENAI_API_KEY", Some(&pool));
        assert!(result.is_some());
        let (key, key_id) = result.unwrap();
        assert_eq!(key, "sk-pool-key");
        assert!(key_id.is_some()); // Key came from pool
    }

    #[test]
    fn test_resolve_api_key_with_pool_falls_back_to_single() {
        let (_temp, db) = setup_test_db();
        let db_arc =
            std::sync::Arc::new(Database::open_at(_temp.path().join("test_pool2.db")).unwrap());
        db_arc.migrate().unwrap();

        // Add a single key (not pool)
        db.save_api_key("MY_API_KEY", "sk-single-key").unwrap();

        let pool = super::super::key_pool::ApiKeyPoolManager::with_defaults(db_arc);

        let result = resolve_api_key_with_pool(&db, "MY_API_KEY", Some(&pool));
        assert!(result.is_some());
        let (key, key_id) = result.unwrap();
        assert_eq!(key, "sk-single-key");
        assert!(key_id.is_none()); // Key came from single storage, not pool
    }

    #[test]
    fn test_resolve_api_key_with_pool_no_pool_manager() {
        let (_temp, db) = setup_test_db();

        // Add a single key
        db.save_api_key("ENV_KEY", "sk-env-key").unwrap();

        let result = resolve_api_key_with_pool(&db, "ENV_KEY", None);
        assert!(result.is_some());
        let (key, key_id) = result.unwrap();
        assert_eq!(key, "sk-env-key");
        assert!(key_id.is_none());
    }

    #[test]
    fn test_resolve_api_key_with_pool_not_found() {
        let (_temp, db) = setup_test_db();
        let db_arc =
            std::sync::Arc::new(Database::open_at(_temp.path().join("test_pool3.db")).unwrap());
        db_arc.migrate().unwrap();

        // Remove any potential env var
        std::env::remove_var("NONEXISTENT_POOL_KEY_XYZ");

        let pool = super::super::key_pool::ApiKeyPoolManager::with_defaults(db_arc);

        let result = resolve_api_key_with_pool(&db, "NONEXISTENT_POOL_KEY_XYZ", Some(&pool));
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_api_key_with_pool_prefers_pool_over_single() {
        let (_temp, db) = setup_test_db();
        let db_arc =
            std::sync::Arc::new(Database::open_at(_temp.path().join("test_pool4.db")).unwrap());
        db_arc.migrate().unwrap();

        // Add both a pool key and a single key with the same name
        db_arc
            .save_pool_key("DUAL_KEY", "sk-pool-version", None, Some(1))
            .unwrap();
        db.save_api_key("DUAL_KEY", "sk-single-version").unwrap();

        let pool = super::super::key_pool::ApiKeyPoolManager::with_defaults(db_arc);

        let result = resolve_api_key_with_pool(&db, "DUAL_KEY", Some(&pool));
        assert!(result.is_some());
        let (key, key_id) = result.unwrap();
        // Pool should take precedence
        assert_eq!(key, "sk-pool-version");
        assert!(key_id.is_some());
    }
}
