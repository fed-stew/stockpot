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
