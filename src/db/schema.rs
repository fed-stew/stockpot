//! Database schema types.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// A stored setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
    pub updated_at: i64,
}

/// A stored session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub name: String,
    pub agent_name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A stored message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub token_count: Option<i64>,
    pub created_at: i64,
}

/// Stored OAuth tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub account_id: Option<String>,
    pub extra_data: Option<String>,
    pub updated_at: i64,
}

impl OAuthTokens {
    /// Check if the token is expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = Utc::now().timestamp();
            now >= expires_at
        } else {
            false
        }
    }

    /// Check if the token will expire soon (within 5 minutes).
    pub fn expires_soon(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = Utc::now().timestamp();
            now >= expires_at - 300
        } else {
            false
        }
    }
}
