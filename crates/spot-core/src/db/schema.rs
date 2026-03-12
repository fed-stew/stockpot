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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_oauth_tokens(expires_at: Option<i64>) -> OAuthTokens {
        OAuthTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at,
            account_id: None,
            extra_data: None,
            updated_at: 0,
        }
    }

    // ==================== OAuthTokens::is_expired tests ====================

    #[test]
    fn is_expired_returns_false_when_no_expiry() {
        let tokens = make_oauth_tokens(None);
        assert!(!tokens.is_expired());
    }

    #[test]
    fn is_expired_returns_false_when_future_expiry() {
        let future = Utc::now().timestamp() + 3600; // 1 hour from now
        let tokens = make_oauth_tokens(Some(future));
        assert!(!tokens.is_expired());
    }

    #[test]
    fn is_expired_returns_true_when_past_expiry() {
        let past = Utc::now().timestamp() - 1; // 1 second ago
        let tokens = make_oauth_tokens(Some(past));
        assert!(tokens.is_expired());
    }

    #[test]
    fn is_expired_returns_true_when_exactly_now() {
        let now = Utc::now().timestamp();
        let tokens = make_oauth_tokens(Some(now));
        assert!(tokens.is_expired());
    }

    #[test]
    fn is_expired_with_very_old_timestamp() {
        let tokens = make_oauth_tokens(Some(0)); // Unix epoch
        assert!(tokens.is_expired());
    }

    #[test]
    fn is_expired_with_far_future_timestamp() {
        let tokens = make_oauth_tokens(Some(i64::MAX - 1000));
        assert!(!tokens.is_expired());
    }

    #[test]
    fn is_expired_with_negative_timestamp() {
        // Before Unix epoch (1969)
        let tokens = make_oauth_tokens(Some(-1000));
        assert!(tokens.is_expired());
    }

    // ==================== OAuthTokens::expires_soon tests ====================

    #[test]
    fn expires_soon_returns_false_when_no_expiry() {
        let tokens = make_oauth_tokens(None);
        assert!(!tokens.expires_soon());
    }

    #[test]
    fn expires_soon_returns_false_when_far_future() {
        let future = Utc::now().timestamp() + 3600; // 1 hour from now
        let tokens = make_oauth_tokens(Some(future));
        assert!(!tokens.expires_soon());
    }

    #[test]
    fn expires_soon_returns_true_when_within_5_minutes() {
        let soon = Utc::now().timestamp() + 200; // 200 seconds from now (< 300)
        let tokens = make_oauth_tokens(Some(soon));
        assert!(tokens.expires_soon());
    }

    #[test]
    fn expires_soon_returns_true_when_already_expired() {
        let past = Utc::now().timestamp() - 100;
        let tokens = make_oauth_tokens(Some(past));
        assert!(tokens.expires_soon());
    }

    #[test]
    fn expires_soon_returns_true_at_exactly_5_minutes() {
        let boundary = Utc::now().timestamp() + 300; // exactly 5 minutes
        let tokens = make_oauth_tokens(Some(boundary));
        assert!(tokens.expires_soon());
    }

    #[test]
    fn expires_soon_returns_false_just_over_5_minutes() {
        let safe = Utc::now().timestamp() + 301; // 1 second over 5 minutes
        let tokens = make_oauth_tokens(Some(safe));
        assert!(!tokens.expires_soon());
    }

    #[test]
    fn expires_soon_consistency_with_is_expired() {
        // If expired, must also expire soon
        let past = Utc::now().timestamp() - 1;
        let tokens = make_oauth_tokens(Some(past));
        assert!(tokens.is_expired());
        assert!(tokens.expires_soon());

        // If far future, neither expired nor expires soon
        let future = Utc::now().timestamp() + 3600;
        let tokens = make_oauth_tokens(Some(future));
        assert!(!tokens.is_expired());
        assert!(!tokens.expires_soon());
    }

    // ==================== Setting struct tests ====================

    #[test]
    fn setting_struct_fields() {
        let setting = Setting {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            updated_at: 12345,
        };
        assert_eq!(setting.key, "test_key");
        assert_eq!(setting.value, "test_value");
        assert_eq!(setting.updated_at, 12345);
    }

    #[test]
    fn setting_with_empty_strings() {
        let setting = Setting {
            key: String::new(),
            value: String::new(),
            updated_at: 0,
        };
        assert!(setting.key.is_empty());
        assert!(setting.value.is_empty());
    }

    #[test]
    fn setting_clone() {
        let original = Setting {
            key: "key".to_string(),
            value: "value".to_string(),
            updated_at: 100,
        };
        let cloned = original.clone();
        assert_eq!(original.key, cloned.key);
        assert_eq!(original.value, cloned.value);
        assert_eq!(original.updated_at, cloned.updated_at);
    }

    #[test]
    fn setting_debug() {
        let setting = Setting {
            key: "k".to_string(),
            value: "v".to_string(),
            updated_at: 1,
        };
        let debug = format!("{:?}", setting);
        assert!(debug.contains("Setting"));
        assert!(debug.contains("key"));
        assert!(debug.contains("value"));
    }

    #[test]
    fn setting_serde_roundtrip() {
        let original = Setting {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            updated_at: 12345,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Setting = serde_json::from_str(&json).unwrap();
        assert_eq!(original.key, deserialized.key);
        assert_eq!(original.value, deserialized.value);
        assert_eq!(original.updated_at, deserialized.updated_at);
    }

    #[test]
    fn setting_deserialize_from_json() {
        let json = r#"{"key":"foo","value":"bar","updated_at":999}"#;
        let setting: Setting = serde_json::from_str(json).unwrap();
        assert_eq!(setting.key, "foo");
        assert_eq!(setting.value, "bar");
        assert_eq!(setting.updated_at, 999);
    }

    // ==================== Session struct tests ====================

    #[test]
    fn session_struct_fields() {
        let session = Session {
            id: 1,
            name: "test session".to_string(),
            agent_name: "agent".to_string(),
            created_at: 100,
            updated_at: 200,
        };
        assert_eq!(session.id, 1);
        assert_eq!(session.name, "test session");
        assert_eq!(session.agent_name, "agent");
        assert_eq!(session.created_at, 100);
        assert_eq!(session.updated_at, 200);
    }

    #[test]
    fn session_clone() {
        let original = Session {
            id: 42,
            name: "session".to_string(),
            agent_name: "agent".to_string(),
            created_at: 1,
            updated_at: 2,
        };
        let cloned = original.clone();
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.agent_name, cloned.agent_name);
    }

    #[test]
    fn session_debug() {
        let session = Session {
            id: 1,
            name: "n".to_string(),
            agent_name: "a".to_string(),
            created_at: 0,
            updated_at: 0,
        };
        let debug = format!("{:?}", session);
        assert!(debug.contains("Session"));
        assert!(debug.contains("name"));
    }

    #[test]
    fn session_serde_roundtrip() {
        let original = Session {
            id: 100,
            name: "my session".to_string(),
            agent_name: "coder".to_string(),
            created_at: 1000,
            updated_at: 2000,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.name, deserialized.name);
        assert_eq!(original.agent_name, deserialized.agent_name);
        assert_eq!(original.created_at, deserialized.created_at);
        assert_eq!(original.updated_at, deserialized.updated_at);
    }

    #[test]
    fn session_deserialize_from_json() {
        let json = r#"{"id":5,"name":"s","agent_name":"a","created_at":10,"updated_at":20}"#;
        let session: Session = serde_json::from_str(json).unwrap();
        assert_eq!(session.id, 5);
        assert_eq!(session.name, "s");
    }

    // ==================== Message struct tests ====================

    #[test]
    fn message_struct_fields() {
        let msg = Message {
            id: 42,
            session_id: 1,
            role: "user".to_string(),
            content: "hello".to_string(),
            token_count: Some(5),
            created_at: 999,
        };
        assert_eq!(msg.id, 42);
        assert_eq!(msg.session_id, 1);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.token_count, Some(5));
        assert_eq!(msg.created_at, 999);
    }

    #[test]
    fn message_with_none_token_count() {
        let msg = Message {
            id: 1,
            session_id: 1,
            role: "assistant".to_string(),
            content: "hi".to_string(),
            token_count: None,
            created_at: 0,
        };
        assert!(msg.token_count.is_none());
    }

    #[test]
    fn message_with_empty_content() {
        let msg = Message {
            id: 1,
            session_id: 1,
            role: "user".to_string(),
            content: String::new(),
            token_count: Some(0),
            created_at: 0,
        };
        assert!(msg.content.is_empty());
        assert_eq!(msg.token_count, Some(0));
    }

    #[test]
    fn message_clone() {
        let original = Message {
            id: 1,
            session_id: 2,
            role: "user".to_string(),
            content: "test".to_string(),
            token_count: Some(10),
            created_at: 100,
        };
        let cloned = original.clone();
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.token_count, cloned.token_count);
    }

    #[test]
    fn message_debug() {
        let msg = Message {
            id: 1,
            session_id: 1,
            role: "user".to_string(),
            content: "c".to_string(),
            token_count: None,
            created_at: 0,
        };
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Message"));
        assert!(debug.contains("role"));
    }

    #[test]
    fn message_serde_roundtrip() {
        let original = Message {
            id: 99,
            session_id: 1,
            role: "system".to_string(),
            content: "You are helpful.".to_string(),
            token_count: Some(100),
            created_at: 5000,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.role, deserialized.role);
        assert_eq!(original.content, deserialized.content);
        assert_eq!(original.token_count, deserialized.token_count);
    }

    #[test]
    fn message_serde_with_none_token_count() {
        let original = Message {
            id: 1,
            session_id: 1,
            role: "user".to_string(),
            content: "hi".to_string(),
            token_count: None,
            created_at: 0,
        };
        let json = serde_json::to_string(&original).unwrap();
        assert!(json.contains("\"token_count\":null"));
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert!(deserialized.token_count.is_none());
    }

    #[test]
    fn message_deserialize_from_json() {
        let json = r#"{"id":1,"session_id":2,"role":"user","content":"hi","token_count":5,"created_at":100}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.id, 1);
        assert_eq!(msg.token_count, Some(5));
    }

    // ==================== OAuthTokens struct tests ====================

    #[test]
    fn oauth_tokens_all_fields() {
        let tokens = OAuthTokens {
            provider: "github".to_string(),
            access_token: "abc123".to_string(),
            refresh_token: Some("refresh456".to_string()),
            expires_at: Some(9999),
            account_id: Some("user@example.com".to_string()),
            extra_data: Some(r#"{"scope":"read"}"#.to_string()),
            updated_at: 1234567890,
        };
        assert_eq!(tokens.provider, "github");
        assert_eq!(tokens.access_token, "abc123");
        assert_eq!(tokens.refresh_token, Some("refresh456".to_string()));
        assert_eq!(tokens.expires_at, Some(9999));
        assert_eq!(tokens.account_id, Some("user@example.com".to_string()));
        assert!(tokens.extra_data.is_some());
        assert_eq!(tokens.updated_at, 1234567890);
    }

    #[test]
    fn oauth_tokens_minimal() {
        let tokens = OAuthTokens {
            provider: "test".to_string(),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            account_id: None,
            extra_data: None,
            updated_at: 0,
        };
        assert!(tokens.refresh_token.is_none());
        assert!(tokens.expires_at.is_none());
        assert!(tokens.account_id.is_none());
        assert!(tokens.extra_data.is_none());
    }

    #[test]
    fn oauth_tokens_clone() {
        let original = OAuthTokens {
            provider: "google".to_string(),
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(12345),
            account_id: Some("acct".to_string()),
            extra_data: None,
            updated_at: 100,
        };
        let cloned = original.clone();
        assert_eq!(original.provider, cloned.provider);
        assert_eq!(original.access_token, cloned.access_token);
        assert_eq!(original.refresh_token, cloned.refresh_token);
        assert_eq!(original.expires_at, cloned.expires_at);
    }

    #[test]
    fn oauth_tokens_debug() {
        let tokens = make_oauth_tokens(None);
        let debug = format!("{:?}", tokens);
        assert!(debug.contains("OAuthTokens"));
        assert!(debug.contains("provider"));
        assert!(debug.contains("access_token"));
    }

    #[test]
    fn oauth_tokens_serde_roundtrip() {
        let original = OAuthTokens {
            provider: "azure".to_string(),
            access_token: "az_token".to_string(),
            refresh_token: Some("az_refresh".to_string()),
            expires_at: Some(999999),
            account_id: Some("azure_user".to_string()),
            extra_data: Some(r#"{"tenant":"abc"}"#.to_string()),
            updated_at: 5000,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: OAuthTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(original.provider, deserialized.provider);
        assert_eq!(original.access_token, deserialized.access_token);
        assert_eq!(original.refresh_token, deserialized.refresh_token);
        assert_eq!(original.expires_at, deserialized.expires_at);
        assert_eq!(original.account_id, deserialized.account_id);
        assert_eq!(original.extra_data, deserialized.extra_data);
        assert_eq!(original.updated_at, deserialized.updated_at);
    }

    #[test]
    fn oauth_tokens_serde_with_nulls() {
        let original = make_oauth_tokens(None);
        let json = serde_json::to_string(&original).unwrap();
        assert!(json.contains("\"refresh_token\":null"));
        assert!(json.contains("\"expires_at\":null"));
        let deserialized: OAuthTokens = serde_json::from_str(&json).unwrap();
        assert!(deserialized.refresh_token.is_none());
        assert!(deserialized.expires_at.is_none());
    }

    #[test]
    fn oauth_tokens_deserialize_from_json() {
        let json = r#"{
            "provider": "openai",
            "access_token": "sk-xxx",
            "refresh_token": null,
            "expires_at": 1700000000,
            "account_id": "org-123",
            "extra_data": null,
            "updated_at": 1699999000
        }"#;
        let tokens: OAuthTokens = serde_json::from_str(json).unwrap();
        assert_eq!(tokens.provider, "openai");
        assert_eq!(tokens.access_token, "sk-xxx");
        assert!(tokens.refresh_token.is_none());
        assert_eq!(tokens.expires_at, Some(1700000000));
        assert_eq!(tokens.account_id, Some("org-123".to_string()));
    }

    #[test]
    fn oauth_tokens_with_unicode() {
        let tokens = OAuthTokens {
            provider: "日本語".to_string(),
            access_token: "token🔑".to_string(),
            refresh_token: Some("émoji 🎉".to_string()),
            expires_at: None,
            account_id: Some("用户@example.com".to_string()),
            extra_data: Some(r#"{"name":"名前"}"#.to_string()),
            updated_at: 0,
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let deserialized: OAuthTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(tokens.provider, deserialized.provider);
        assert_eq!(tokens.access_token, deserialized.access_token);
        assert_eq!(tokens.refresh_token, deserialized.refresh_token);
    }

    #[test]
    fn oauth_tokens_with_special_chars() {
        let tokens = OAuthTokens {
            provider: "test/provider".to_string(),
            access_token: "token\"with\\quotes".to_string(),
            refresh_token: Some("newline\ntoken".to_string()),
            expires_at: None,
            account_id: None,
            extra_data: Some(r#"{"key": "value with \"quotes\""}"#.to_string()),
            updated_at: 0,
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let deserialized: OAuthTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(tokens.access_token, deserialized.access_token);
        assert_eq!(tokens.refresh_token, deserialized.refresh_token);
    }

    // ==================== Edge cases ====================

    #[test]
    fn setting_with_very_long_value() {
        let long_value = "x".repeat(10000);
        let setting = Setting {
            key: "long_key".to_string(),
            value: long_value.clone(),
            updated_at: 0,
        };
        let json = serde_json::to_string(&setting).unwrap();
        let deserialized: Setting = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.value.len(), 10000);
    }

    #[test]
    fn message_with_multiline_content() {
        let content = "line1\nline2\nline3\n\ttabbed";
        let msg = Message {
            id: 1,
            session_id: 1,
            role: "user".to_string(),
            content: content.to_string(),
            token_count: None,
            created_at: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, content);
    }

    #[test]
    fn session_with_zero_timestamps() {
        let session = Session {
            id: 0,
            name: "zero".to_string(),
            agent_name: "a".to_string(),
            created_at: 0,
            updated_at: 0,
        };
        assert_eq!(session.created_at, 0);
        assert_eq!(session.updated_at, 0);
    }

    #[test]
    fn session_with_negative_id() {
        // While unusual, the type allows it
        let session = Session {
            id: -1,
            name: "neg".to_string(),
            agent_name: "a".to_string(),
            created_at: 0,
            updated_at: 0,
        };
        assert_eq!(session.id, -1);
    }

    #[test]
    fn message_with_large_token_count() {
        let msg = Message {
            id: 1,
            session_id: 1,
            role: "assistant".to_string(),
            content: "long response".to_string(),
            token_count: Some(i64::MAX),
            created_at: 0,
        };
        assert_eq!(msg.token_count, Some(i64::MAX));
    }
}
