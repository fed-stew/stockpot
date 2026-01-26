//! OAuth token storage in SQLite.

use crate::db::Database;
use chrono::Utc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenStorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Provider not authenticated: {0}")]
    NotAuthenticated(String),
    #[error("Token expired")]
    Expired,
}

/// Stored OAuth tokens.
#[derive(Debug, Clone)]
pub struct StoredTokens {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub account_id: Option<String>,
    pub extra_data: Option<String>,
    pub updated_at: i64,
}

impl StoredTokens {
    /// Check if the token is expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now().timestamp() >= expires_at
        } else {
            false
        }
    }

    /// Check if the token will expire within the given seconds.
    pub fn expires_within(&self, seconds: i64) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now().timestamp() >= expires_at - seconds
        } else {
            false
        }
    }
}

/// Token storage operations.
pub struct TokenStorage<'a> {
    db: &'a Database,
}

impl<'a> TokenStorage<'a> {
    /// Create a new token storage.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Save tokens for a provider.
    pub fn save(
        &self,
        provider: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: Option<u64>,
        account_id: Option<&str>,
        extra_data: Option<&str>,
    ) -> Result<(), TokenStorageError> {
        let expires_at = expires_in.map(|secs| Utc::now().timestamp() + secs as i64);

        self.db.conn().execute(
            "INSERT INTO oauth_tokens (provider, access_token, refresh_token, expires_at, account_id, extra_data, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, unixepoch())
             ON CONFLICT(provider) DO UPDATE SET 
                access_token = excluded.access_token,
                refresh_token = COALESCE(excluded.refresh_token, oauth_tokens.refresh_token),
                expires_at = excluded.expires_at,
                account_id = COALESCE(excluded.account_id, oauth_tokens.account_id),
                extra_data = COALESCE(excluded.extra_data, oauth_tokens.extra_data),
                updated_at = excluded.updated_at",
            rusqlite::params![
                provider,
                access_token,
                refresh_token,
                expires_at,
                account_id,
                extra_data,
            ],
        )?;

        Ok(())
    }

    /// Load tokens for a provider.
    pub fn load(&self, provider: &str) -> Result<Option<StoredTokens>, TokenStorageError> {
        let result = self.db.conn().query_row(
            "SELECT provider, access_token, refresh_token, expires_at, account_id, extra_data, updated_at
             FROM oauth_tokens WHERE provider = ?",
            [provider],
            |row| {
                Ok(StoredTokens {
                    provider: row.get(0)?,
                    access_token: row.get(1)?,
                    refresh_token: row.get(2)?,
                    expires_at: row.get(3)?,
                    account_id: row.get(4)?,
                    extra_data: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );

        match result {
            Ok(tokens) => Ok(Some(tokens)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(TokenStorageError::Database(e)),
        }
    }

    /// Delete tokens for a provider.
    pub fn delete(&self, provider: &str) -> Result<(), TokenStorageError> {
        self.db
            .conn()
            .execute("DELETE FROM oauth_tokens WHERE provider = ?", [provider])?;
        Ok(())
    }

    /// Check if a provider is authenticated (has tokens).
    pub fn is_authenticated(&self, provider: &str) -> Result<bool, TokenStorageError> {
        Ok(self.load(provider)?.is_some())
    }

    /// List all authenticated providers.
    pub fn list_providers(&self) -> Result<Vec<String>, TokenStorageError> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT provider FROM oauth_tokens ORDER BY provider")?;

        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut providers = Vec::new();
        for row in rows {
            providers.push(row?);
        }
        Ok(providers)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for OAuth token storage.
    //!
    //! Coverage:
    //! - StoredTokens expiry checks
    //! - TokenStorage CRUD operations
    //! - Provider listing
    //! - Token preservation on update

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
        (temp_dir, db) // TempDir must be kept alive
    }

    // =========================================================================
    // Task 2.1: StoredTokens Unit Tests
    // =========================================================================

    #[test]
    fn test_stored_tokens_not_expired() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp() + 3600), // 1 hour from now
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(!tokens.is_expired());
    }

    #[test]
    fn test_stored_tokens_is_expired() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp() - 3600), // 1 hour ago
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(tokens.is_expired());
    }

    #[test]
    fn test_stored_tokens_expires_within_true() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp() + 60), // 1 min from now
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(tokens.expires_within(120)); // Within 2 minutes
    }

    #[test]
    fn test_stored_tokens_expires_within_false() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp() + 3600), // 1 hour from now
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(!tokens.expires_within(120)); // Not within 2 minutes
    }

    #[test]
    fn test_stored_tokens_no_expiry_never_expires() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None, // No expiry
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(!tokens.is_expired());
        assert!(!tokens.expires_within(999999));
    }

    #[test]
    fn test_stored_tokens_just_expired() {
        // Edge case: token expires exactly now
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp()), // Exactly now
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(tokens.is_expired());
    }

    #[test]
    fn test_stored_tokens_clone() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(12345),
            account_id: Some("account".to_string()),
            extra_data: Some("extra".to_string()),
            updated_at: 67890,
        };

        let cloned = tokens.clone();
        assert_eq!(cloned.provider, tokens.provider);
        assert_eq!(cloned.access_token, tokens.access_token);
        assert_eq!(cloned.refresh_token, tokens.refresh_token);
        assert_eq!(cloned.expires_at, tokens.expires_at);
        assert_eq!(cloned.account_id, tokens.account_id);
        assert_eq!(cloned.extra_data, tokens.extra_data);
        assert_eq!(cloned.updated_at, tokens.updated_at);
    }

    // =========================================================================
    // Task 2.2: TokenStorage CRUD Tests
    // =========================================================================

    #[test]
    fn test_save_and_load_tokens() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save(
                "provider1",
                "access123",
                Some("refresh456"),
                Some(3600),
                None,
                None,
            )
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, "access123");
        assert_eq!(loaded.refresh_token, Some("refresh456".to_string()));
    }

    #[test]
    fn test_save_updates_existing_tokens() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider1", "old_token", None, None, None, None)
            .unwrap();
        storage
            .save("provider1", "new_token", None, None, None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, "new_token");
    }

    #[test]
    fn test_load_nonexistent_provider() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        let result = storage.load("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_tokens() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider1", "token", None, None, None, None)
            .unwrap();
        storage.delete("provider1").unwrap();

        assert!(storage.load("provider1").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_provider() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Should not error when deleting non-existent provider
        storage.delete("nonexistent").unwrap();
    }

    #[test]
    fn test_is_authenticated_true() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider1", "token", None, None, None, None)
            .unwrap();
        assert!(storage.is_authenticated("provider1").unwrap());
    }

    #[test]
    fn test_is_authenticated_false() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        assert!(!storage.is_authenticated("nonexistent").unwrap());
    }

    #[test]
    fn test_list_providers_empty() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        let providers = storage.list_providers().unwrap();
        assert!(providers.is_empty());
    }

    #[test]
    fn test_list_providers_multiple() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("anthropic", "token1", None, None, None, None)
            .unwrap();
        storage
            .save("openai", "token2", None, None, None, None)
            .unwrap();

        let providers = storage.list_providers().unwrap();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"anthropic".to_string()));
        assert!(providers.contains(&"openai".to_string()));
    }

    #[test]
    fn test_list_providers_sorted() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("zebra", "token1", None, None, None, None)
            .unwrap();
        storage
            .save("alpha", "token2", None, None, None, None)
            .unwrap();
        storage
            .save("middle", "token3", None, None, None, None)
            .unwrap();

        let providers = storage.list_providers().unwrap();
        assert_eq!(providers, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn test_save_preserves_refresh_token_on_update() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Initial save with refresh token
        storage
            .save("provider1", "access1", Some("refresh1"), None, None, None)
            .unwrap();

        // Update without refresh token (simulates token refresh response)
        storage
            .save("provider1", "access2", None, None, None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, "access2");
        assert_eq!(
            loaded.refresh_token,
            Some("refresh1".to_string()),
            "Refresh token should be preserved"
        );
    }

    #[test]
    fn test_save_preserves_account_id_on_update() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Initial save with account_id
        storage
            .save("provider1", "access1", None, None, Some("account123"), None)
            .unwrap();

        // Update without account_id
        storage
            .save("provider1", "access2", None, None, None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, "access2");
        assert_eq!(
            loaded.account_id,
            Some("account123".to_string()),
            "Account ID should be preserved"
        );
    }

    #[test]
    fn test_save_with_all_fields() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save(
                "full_provider",
                "access_token",
                Some("refresh_token"),
                Some(7200),
                Some("account_id"),
                Some("extra_data"),
            )
            .unwrap();

        let loaded = storage.load("full_provider").unwrap().unwrap();
        assert_eq!(loaded.provider, "full_provider");
        assert_eq!(loaded.access_token, "access_token");
        assert_eq!(loaded.refresh_token, Some("refresh_token".to_string()));
        assert!(loaded.expires_at.is_some());
        assert_eq!(loaded.account_id, Some("account_id".to_string()));
        assert_eq!(loaded.extra_data, Some("extra_data".to_string()));
        assert!(loaded.updated_at > 0);
    }

    #[test]
    fn test_save_overwrites_refresh_token_when_provided() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Initial save with refresh token
        storage
            .save("provider1", "access1", Some("refresh1"), None, None, None)
            .unwrap();

        // Update with new refresh token
        storage
            .save("provider1", "access2", Some("refresh2"), None, None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.refresh_token, Some("refresh2".to_string()));
    }

    // =========================================================================
    // Error Type Tests
    // =========================================================================

    #[test]
    fn test_token_storage_error_display() {
        let db_err = TokenStorageError::Database(rusqlite::Error::QueryReturnedNoRows);
        assert!(db_err.to_string().contains("Database error"));

        let not_auth = TokenStorageError::NotAuthenticated("test".to_string());
        assert!(not_auth.to_string().contains("Provider not authenticated"));
        assert!(not_auth.to_string().contains("test"));

        let expired = TokenStorageError::Expired;
        assert!(expired.to_string().contains("Token expired"));
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    #[test]
    fn test_stored_tokens_expires_within_boundary() {
        // Token expires in exactly 120 seconds
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp() + 120),
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        // Asking if it expires within 120 seconds - boundary condition
        // expires_at - seconds == now, so now >= expires_at - seconds is true
        assert!(tokens.expires_within(120));
        // But not within 119 seconds
        assert!(!tokens.expires_within(119));
    }

    #[test]
    fn test_stored_tokens_debug_impl() {
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "secret".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(12345),
            account_id: Some("acc".to_string()),
            extra_data: Some("extra".to_string()),
            updated_at: 67890,
        };
        let debug_str = format!("{:?}", tokens);
        assert!(debug_str.contains("StoredTokens"));
        assert!(debug_str.contains("provider"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_save_preserves_extra_data_on_update() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Initial save with extra_data
        storage
            .save(
                "provider1",
                "access1",
                None,
                None,
                None,
                Some(r#"{"scope":"read"}"#),
            )
            .unwrap();

        // Update without extra_data
        storage
            .save("provider1", "access2", None, None, None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, "access2");
        assert_eq!(
            loaded.extra_data,
            Some(r#"{"scope":"read"}"#.to_string()),
            "Extra data should be preserved"
        );
    }

    #[test]
    fn test_multiple_providers_isolated() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider_a", "token_a", Some("refresh_a"), None, None, None)
            .unwrap();
        storage
            .save("provider_b", "token_b", Some("refresh_b"), None, None, None)
            .unwrap();

        // Update provider_a should not affect provider_b
        storage
            .save("provider_a", "token_a_new", None, None, None, None)
            .unwrap();

        let loaded_a = storage.load("provider_a").unwrap().unwrap();
        let loaded_b = storage.load("provider_b").unwrap().unwrap();

        assert_eq!(loaded_a.access_token, "token_a_new");
        assert_eq!(loaded_b.access_token, "token_b");
        assert_eq!(loaded_b.refresh_token, Some("refresh_b".to_string()));
    }

    #[test]
    fn test_delete_one_provider_preserves_others() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider_a", "token_a", None, None, None, None)
            .unwrap();
        storage
            .save("provider_b", "token_b", None, None, None, None)
            .unwrap();

        storage.delete("provider_a").unwrap();

        assert!(storage.load("provider_a").unwrap().is_none());
        assert!(storage.load("provider_b").unwrap().is_some());
    }

    #[test]
    fn test_expires_in_calculation() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        let before = Utc::now().timestamp();
        storage
            .save("provider1", "token", None, Some(3600), None, None)
            .unwrap();
        let after = Utc::now().timestamp();

        let loaded = storage.load("provider1").unwrap().unwrap();
        let expires_at = loaded.expires_at.unwrap();

        // expires_at should be ~3600 seconds from now
        assert!(expires_at >= before + 3600);
        assert!(expires_at <= after + 3600);
    }

    #[test]
    fn test_save_with_empty_strings() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Empty access token (unusual but valid)
        storage
            .save("provider1", "", Some(""), None, Some(""), Some(""))
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, "");
        assert_eq!(loaded.refresh_token, Some("".to_string()));
        assert_eq!(loaded.account_id, Some("".to_string()));
        assert_eq!(loaded.extra_data, Some("".to_string()));
    }

    #[test]
    fn test_save_with_large_token() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // JWT tokens can be quite large
        let large_token = "a".repeat(10000);
        storage
            .save("provider1", &large_token, None, None, None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token.len(), 10000);
    }

    #[test]
    fn test_save_with_special_characters() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        let special_token = "token'with\"special;chars\nand\ttabs";
        let special_extra = r#"{"key": "value with 'quotes' and \"escapes\""}"#;

        storage
            .save(
                "provider1",
                special_token,
                None,
                None,
                None,
                Some(special_extra),
            )
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        assert_eq!(loaded.access_token, special_token);
        assert_eq!(loaded.extra_data, Some(special_extra.to_string()));
    }

    #[test]
    fn test_token_storage_error_from_rusqlite() {
        // Test the From<rusqlite::Error> implementation
        let rusqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let storage_err: TokenStorageError = rusqlite_err.into();

        match storage_err {
            TokenStorageError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    #[test]
    fn test_stored_tokens_with_zero_expiry() {
        // Unix timestamp 0 = epoch (1970-01-01), definitely expired
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(0),
            account_id: None,
            extra_data: None,
            updated_at: 0,
        };
        assert!(tokens.is_expired());
    }

    #[test]
    fn test_stored_tokens_with_far_future_expiry() {
        // Year 2100 timestamp
        let far_future = 4102444800_i64; // 2100-01-01
        let tokens = StoredTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(far_future),
            account_id: None,
            extra_data: None,
            updated_at: Utc::now().timestamp(),
        };
        assert!(!tokens.is_expired());
        assert!(!tokens.expires_within(86400 * 365 * 10)); // Not within 10 years
    }

    #[test]
    fn test_list_providers_after_delete() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider_a", "token", None, None, None, None)
            .unwrap();
        storage
            .save("provider_b", "token", None, None, None, None)
            .unwrap();

        storage.delete("provider_a").unwrap();

        let providers = storage.list_providers().unwrap();
        assert_eq!(providers, vec!["provider_b"]);
    }

    #[test]
    fn test_save_updates_updated_at() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider1", "token1", None, None, None, None)
            .unwrap();
        let first_load = storage.load("provider1").unwrap().unwrap();
        let first_updated = first_load.updated_at;

        // Small delay to ensure timestamp changes (SQLite uses unixepoch() which is seconds)
        std::thread::sleep(std::time::Duration::from_secs(1));

        storage
            .save("provider1", "token2", None, None, None, None)
            .unwrap();
        let second_load = storage.load("provider1").unwrap().unwrap();

        assert!(
            second_load.updated_at >= first_updated,
            "updated_at should increase on update"
        );
    }

    #[test]
    fn test_is_authenticated_after_delete() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        storage
            .save("provider1", "token", None, None, None, None)
            .unwrap();
        assert!(storage.is_authenticated("provider1").unwrap());

        storage.delete("provider1").unwrap();
        assert!(!storage.is_authenticated("provider1").unwrap());
    }

    #[test]
    fn test_save_with_unicode_provider_name() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // Unicode in provider name (edge case)
        storage
            .save("provider_日本語", "token", None, None, None, None)
            .unwrap();

        let loaded = storage.load("provider_日本語").unwrap().unwrap();
        assert_eq!(loaded.provider, "provider_日本語");
        assert!(storage.is_authenticated("provider_日本語").unwrap());

        let providers = storage.list_providers().unwrap();
        assert!(providers.contains(&"provider_日本語".to_string()));
    }

    #[test]
    fn test_save_expires_in_zero() {
        let (_temp, db) = setup_test_db();
        let storage = TokenStorage::new(&db);

        // expires_in = 0 means token expires immediately
        let before = Utc::now().timestamp();
        storage
            .save("provider1", "token", None, Some(0), None, None)
            .unwrap();

        let loaded = storage.load("provider1").unwrap().unwrap();
        // expires_at should be approximately equal to save time
        assert!(loaded.expires_at.unwrap() >= before);
        assert!(loaded.expires_at.unwrap() <= before + 1);
        // Token should be expired or about to expire
        assert!(loaded.is_expired() || loaded.expires_within(1));
    }

    #[test]
    fn test_token_storage_error_debug() {
        let err = TokenStorageError::Expired;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Expired"));

        let not_auth = TokenStorageError::NotAuthenticated("google".to_string());
        let debug_str = format!("{:?}", not_auth);
        assert!(debug_str.contains("NotAuthenticated"));
        assert!(debug_str.contains("google"));
    }
}
