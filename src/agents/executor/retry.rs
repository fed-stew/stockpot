//! Retry and failover logic for API requests.
//!
//! Wraps model execution with intelligent retry handling for rate limits (429)
//! and key rotation using the ApiKeyPoolManager.

use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, error, info, warn};

use crate::db::Database;
use crate::models::{ApiKeyPoolManager, KeyPoolConfig, RotationResult};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum full cycles through all keys before giving up
    pub max_cycles: u32,
    /// Time to wait when all keys exhausted before retrying
    pub cooldown_secs: u64,
    /// HTTP status codes that trigger rotation (default: [429])
    pub retry_status_codes: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_cycles: 5,
            cooldown_secs: 15,
            retry_status_codes: vec![429],
        }
    }
}

/// Events emitted during retry operations
#[derive(Debug, Clone, PartialEq)]
pub enum RetryEvent {
    /// Key rotation occurred
    KeyRotated {
        provider: String,
        attempt: u32,
        reason: String,
    },
    /// All keys exhausted, starting cooldown wait
    CooldownStarted {
        provider: String,
        cycle: u32,
        max_cycles: u32,
        wait_secs: u64,
    },
    /// Cooldown complete, retrying
    CooldownComplete { provider: String, cycle: u32 },
    /// Max retries exceeded, giving up
    MaxRetriesExceeded {
        provider: String,
        total_attempts: u32,
    },
}

/// Result of a retry attempt
#[derive(Debug, PartialEq)]
pub enum RetryDecision {
    /// Retry with a new key
    RetryWithKey { key: String, key_id: i64 },
    /// Wait for cooldown then retry
    WaitAndRetry { wait_duration: Duration },
    /// Give up - max retries exceeded
    GiveUp { reason: String },
    /// Error doesn't warrant retry (not a rate limit)
    DontRetry,
}

/// Handles retry decisions for API rate limits
pub struct RetryHandler {
    pool_manager: Arc<ApiKeyPoolManager>,
    config: RetryConfig,
}

impl RetryHandler {
    /// Create a new retry handler with default configuration
    pub fn new(db: Arc<Database>) -> Self {
        let pool_config = KeyPoolConfig::default();
        Self {
            pool_manager: Arc::new(ApiKeyPoolManager::new(db, pool_config)),
            config: RetryConfig::default(),
        }
    }

    /// Create a retry handler with custom configuration
    pub fn with_config(db: Arc<Database>, config: RetryConfig) -> Self {
        let pool_config = KeyPoolConfig {
            max_retry_cycles: config.max_cycles,
            cooldown_duration: Duration::from_secs(config.cooldown_secs),
            track_stats: true,
        };
        Self {
            pool_manager: Arc::new(ApiKeyPoolManager::new(db, pool_config)),
            config,
        }
    }

    /// Check if the provider has multiple keys and should use rotation
    pub fn should_use_rotation(&self, provider: &str) -> bool {
        self.pool_manager.has_pool_keys(provider)
    }

    /// Get the pool manager for direct access
    pub fn pool_manager(&self) -> &Arc<ApiKeyPoolManager> {
        &self.pool_manager
    }

    /// Get the configuration
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }

    /// Check if an error string indicates a rate limit
    pub fn is_rate_limit_error(&self, error: &str) -> bool {
        for code in &self.config.retry_status_codes {
            if error.contains(&format!("status: {}", code))
                || error.contains(&format!("status:{}", code))
                || (error.contains(&format!("{}", code)) && error.to_lowercase().contains("rate"))
            {
                return true;
            }
        }

        // Also check for common rate limit messages (case insensitive)
        let lower = error.to_lowercase();
        lower.contains("rate limit")   // matches "rate limit", "rate limited", "rate-limit"
            || lower.contains("rate_limit")   // matches "rate_limit_error"
            || lower.contains("ratelimit")    // matches "ratelimited", "ratelimiting"
            || lower.contains("too many requests")
            || lower.contains("quota exceeded")
            || lower.contains("throttle") // some APIs use "throttled"
    }

    /// Decide what to do after a rate limit error
    pub async fn handle_rate_limit(&self, provider: &str, failed_key_id: i64) -> RetryDecision {
        match self
            .pool_manager
            .rotate_on_rate_limit(provider, failed_key_id)
            .await
        {
            RotationResult::Rotated { key, key_id } => {
                info!(
                    provider = %provider,
                    key_id = key_id,
                    "Rotated to next API key after rate limit"
                );
                RetryDecision::RetryWithKey { key, key_id }
            }
            RotationResult::Exhausted { cycle, max_cycles } => {
                warn!(
                    provider = %provider,
                    cycle = cycle,
                    max_cycles = max_cycles,
                    wait_secs = self.config.cooldown_secs,
                    "All keys rate-limited, entering cooldown"
                );
                RetryDecision::WaitAndRetry {
                    wait_duration: Duration::from_secs(self.config.cooldown_secs),
                }
            }
            RotationResult::MaxRetriesExceeded => {
                error!(
                    provider = %provider,
                    max_cycles = self.config.max_cycles,
                    "Max retry cycles exceeded, giving up"
                );
                RetryDecision::GiveUp {
                    reason: format!(
                        "All API keys for {} are rate-limited. Tried {} full rotation cycles with {}s cooldowns. Please wait and try again later.",
                        provider, self.config.max_cycles, self.config.cooldown_secs
                    ),
                }
            }
            RotationResult::NoKeys => {
                debug!(provider = %provider, "No pool keys configured, cannot rotate");
                RetryDecision::DontRetry
            }
        }
    }

    /// Wait for cooldown period
    /// Note: This will be used by the streaming executor in Phase 4
    #[allow(dead_code)]
    pub async fn wait_cooldown(&self) {
        self.pool_manager.wait_cooldown().await;
    }

    /// Mark a successful request (resets error counters)
    pub fn mark_success(&self, provider: &str, key_id: i64) {
        self.pool_manager.mark_success(provider, key_id);
    }

    /// Load keys for a provider (call before using rotation)
    pub fn load_provider(&self, provider: &str) -> Result<usize, rusqlite::Error> {
        self.pool_manager.load_provider(provider)
    }

    /// Get current key for a provider
    pub fn get_current_key(&self, provider: &str) -> Option<(String, i64)> {
        self.pool_manager.get_current_key(provider)
    }

    /// Check if provider has multiple keys (worth rotating)
    pub fn has_multiple_keys(&self, provider: &str) -> bool {
        self.pool_manager.has_multiple_keys(provider)
    }
}

/// Extract HTTP status code from an error string
pub fn extract_status_code(error: &str) -> Option<u16> {
    // Look for patterns like "status: 429" or "status:429"
    let patterns = ["status: ", "status:"];
    for pattern in patterns {
        if let Some(pos) = error.find(pattern) {
            let start = pos + pattern.len();
            let code_str: String = error[start..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(code) = code_str.parse::<u16>() {
                return Some(code);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    fn setup_test_db() -> (TempDir, Arc<Database>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, Arc::new(db))
    }

    fn add_pool_key(db: &Database, provider: &str, api_key: &str, priority: i32) -> i64 {
        db.save_pool_key(provider, api_key, None, Some(priority))
            .unwrap()
    }

    // =========================================================================
    // RetryConfig Tests
    // =========================================================================

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_cycles, 5);
        assert_eq!(config.cooldown_secs, 15);
        assert_eq!(config.retry_status_codes, vec![429]);
    }

    #[test]
    fn test_retry_config_custom() {
        let config = RetryConfig {
            max_cycles: 10,
            cooldown_secs: 30,
            retry_status_codes: vec![429, 503],
        };
        assert_eq!(config.max_cycles, 10);
        assert_eq!(config.cooldown_secs, 30);
        assert_eq!(config.retry_status_codes, vec![429, 503]);
    }

    // =========================================================================
    // RetryEvent Tests
    // =========================================================================

    #[test]
    fn test_retry_event_equality() {
        let e1 = RetryEvent::KeyRotated {
            provider: "openai".to_string(),
            attempt: 1,
            reason: "Rate limited".to_string(),
        };
        let e2 = RetryEvent::KeyRotated {
            provider: "openai".to_string(),
            attempt: 1,
            reason: "Rate limited".to_string(),
        };
        assert_eq!(e1, e2);

        let e3 = RetryEvent::CooldownStarted {
            provider: "openai".to_string(),
            cycle: 1,
            max_cycles: 5,
            wait_secs: 15,
        };
        let e4 = RetryEvent::CooldownStarted {
            provider: "openai".to_string(),
            cycle: 1,
            max_cycles: 5,
            wait_secs: 15,
        };
        assert_eq!(e3, e4);
    }

    #[test]
    fn test_retry_event_variants() {
        let rotated = RetryEvent::KeyRotated {
            provider: "anthropic".to_string(),
            attempt: 2,
            reason: "429".to_string(),
        };
        assert!(matches!(rotated, RetryEvent::KeyRotated { .. }));

        let cooldown = RetryEvent::CooldownStarted {
            provider: "openai".to_string(),
            cycle: 1,
            max_cycles: 5,
            wait_secs: 15,
        };
        assert!(matches!(cooldown, RetryEvent::CooldownStarted { .. }));

        let complete = RetryEvent::CooldownComplete {
            provider: "openai".to_string(),
            cycle: 1,
        };
        assert!(matches!(complete, RetryEvent::CooldownComplete { .. }));

        let exceeded = RetryEvent::MaxRetriesExceeded {
            provider: "openai".to_string(),
            total_attempts: 15,
        };
        assert!(matches!(exceeded, RetryEvent::MaxRetriesExceeded { .. }));
    }

    // =========================================================================
    // RetryDecision Tests
    // =========================================================================

    #[test]
    fn test_retry_decision_equality() {
        let d1 = RetryDecision::RetryWithKey {
            key: "sk-key".to_string(),
            key_id: 1,
        };
        let d2 = RetryDecision::RetryWithKey {
            key: "sk-key".to_string(),
            key_id: 1,
        };
        assert_eq!(d1, d2);

        let d3 = RetryDecision::WaitAndRetry {
            wait_duration: Duration::from_secs(15),
        };
        let d4 = RetryDecision::WaitAndRetry {
            wait_duration: Duration::from_secs(15),
        };
        assert_eq!(d3, d4);

        assert_eq!(RetryDecision::DontRetry, RetryDecision::DontRetry);
    }

    #[test]
    fn test_retry_decision_give_up() {
        let d = RetryDecision::GiveUp {
            reason: "All keys exhausted".to_string(),
        };
        assert!(matches!(d, RetryDecision::GiveUp { .. }));
    }

    // =========================================================================
    // is_rate_limit_error Tests
    // =========================================================================

    #[test]
    fn test_is_rate_limit_error_status_429() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);

        assert!(handler.is_rate_limit_error("HTTP error: status: 429"));
        assert!(handler.is_rate_limit_error("status:429 too many requests"));
        assert!(handler.is_rate_limit_error("Error status: 429, rate limited"));
    }

    #[test]
    fn test_is_rate_limit_error_common_messages() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);

        assert!(handler.is_rate_limit_error("Rate limit exceeded"));
        assert!(handler.is_rate_limit_error("Error: rate_limit_error"));
        assert!(handler.is_rate_limit_error("Too many requests"));
        assert!(handler.is_rate_limit_error("Quota exceeded for today"));
    }

    #[test]
    fn test_is_rate_limit_error_case_insensitive() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);

        assert!(handler.is_rate_limit_error("RATE LIMIT EXCEEDED"));
        assert!(handler.is_rate_limit_error("Rate_Limit_Error"));
        assert!(handler.is_rate_limit_error("TOO MANY REQUESTS"));
    }

    #[test]
    fn test_is_rate_limit_error_real_api_messages() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);

        // The exact error message from Claude API
        assert!(handler.is_rate_limit_error("Rate limited, retry after Some(60s)"));
        assert!(handler.is_rate_limit_error("Model error: Rate limited, retry after Some(60s)"));

        // Other common patterns
        assert!(handler.is_rate_limit_error("Request was ratelimited"));
        assert!(handler.is_rate_limit_error("Your request has been throttled"));
    }

    #[test]
    fn test_is_rate_limit_error_false_for_other_errors() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);

        assert!(!handler.is_rate_limit_error("status: 500 internal server error"));
        assert!(!handler.is_rate_limit_error("Authentication failed"));
        assert!(!handler.is_rate_limit_error("Invalid API key"));
        assert!(!handler.is_rate_limit_error("Connection refused"));
    }

    #[test]
    fn test_is_rate_limit_error_custom_status_codes() {
        let (_temp, db) = setup_test_db();
        let config = RetryConfig {
            retry_status_codes: vec![429, 503],
            ..Default::default()
        };
        let handler = RetryHandler::with_config(db, config);

        assert!(handler.is_rate_limit_error("status: 429"));
        assert!(handler.is_rate_limit_error("status: 503 service unavailable rate"));
    }

    // =========================================================================
    // extract_status_code Tests
    // =========================================================================

    #[test]
    fn test_extract_status_code_with_space() {
        assert_eq!(extract_status_code("status: 429"), Some(429));
        assert_eq!(extract_status_code("HTTP status: 503"), Some(503));
        assert_eq!(
            extract_status_code("Error status: 400 bad request"),
            Some(400)
        );
    }

    #[test]
    fn test_extract_status_code_without_space() {
        assert_eq!(extract_status_code("status:429"), Some(429));
        assert_eq!(extract_status_code("HTTP status:200"), Some(200));
    }

    #[test]
    fn test_extract_status_code_no_match() {
        assert_eq!(extract_status_code("No status code here"), None);
        assert_eq!(extract_status_code("error: something went wrong"), None);
        assert_eq!(extract_status_code("status word but no number"), None);
    }

    #[test]
    fn test_extract_status_code_multiple_numbers() {
        // Should extract the first status code found
        assert_eq!(extract_status_code("status: 429, then 200"), Some(429));
    }

    // =========================================================================
    // RetryHandler Basic Tests
    // =========================================================================

    #[test]
    fn test_retry_handler_new() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);
        assert_eq!(handler.config().max_cycles, 5);
        assert_eq!(handler.config().cooldown_secs, 15);
    }

    #[test]
    fn test_retry_handler_with_config() {
        let (_temp, db) = setup_test_db();
        let config = RetryConfig {
            max_cycles: 10,
            cooldown_secs: 30,
            retry_status_codes: vec![429, 503],
        };
        let handler = RetryHandler::with_config(db, config);
        assert_eq!(handler.config().max_cycles, 10);
        assert_eq!(handler.config().cooldown_secs, 30);
    }

    #[test]
    fn test_should_use_rotation_no_keys() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);
        assert!(!handler.should_use_rotation("openai"));
    }

    #[test]
    fn test_should_use_rotation_with_keys() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        let handler = RetryHandler::new(db);
        assert!(handler.should_use_rotation("openai"));
    }

    #[test]
    fn test_has_multiple_keys() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        add_pool_key(&db, "openai", "sk-key-2", 2);
        let handler = RetryHandler::new(db);
        assert!(handler.has_multiple_keys("openai"));
    }

    #[test]
    fn test_load_provider() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        add_pool_key(&db, "openai", "sk-key-2", 2);
        let handler = RetryHandler::new(db);

        let count = handler.load_provider("openai").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_current_key() {
        let (_temp, db) = setup_test_db();
        let key_id = add_pool_key(&db, "openai", "sk-key-1", 1);
        let handler = RetryHandler::new(db);

        let result = handler.get_current_key("openai");
        assert!(result.is_some());
        let (key, id) = result.unwrap();
        assert_eq!(key, "sk-key-1");
        assert_eq!(id, key_id);
    }

    // =========================================================================
    // handle_rate_limit Tests
    // =========================================================================

    #[tokio::test]
    async fn test_handle_rate_limit_no_keys() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);

        let decision = handler.handle_rate_limit("openai", 999).await;
        assert_eq!(decision, RetryDecision::DontRetry);
    }

    #[tokio::test]
    async fn test_handle_rate_limit_rotates_to_next_key() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let handler = RetryHandler::new(db);
        handler.load_provider("openai").unwrap();

        let decision = handler.handle_rate_limit("openai", key_id_1).await;
        assert!(matches!(
            decision,
            RetryDecision::RetryWithKey { key_id, .. } if key_id == key_id_2
        ));
    }

    #[tokio::test]
    async fn test_handle_rate_limit_exhausted() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let config = RetryConfig {
            max_cycles: 3,
            cooldown_secs: 1,
            ..Default::default()
        };
        let handler = RetryHandler::with_config(db, config);
        handler.load_provider("openai").unwrap();

        // Rotate through all keys
        handler.handle_rate_limit("openai", key_id_1).await; // -> key 2
        let decision = handler.handle_rate_limit("openai", key_id_2).await; // -> exhausted

        assert!(matches!(
            decision,
            RetryDecision::WaitAndRetry { wait_duration } if wait_duration == Duration::from_secs(1)
        ));
    }

    #[tokio::test]
    async fn test_handle_rate_limit_max_retries_exceeded() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let config = RetryConfig {
            max_cycles: 2,
            cooldown_secs: 0,
            ..Default::default()
        };
        let handler = RetryHandler::with_config(db, config);
        handler.load_provider("openai").unwrap();

        // Exhaust cycle 1
        handler.handle_rate_limit("openai", key_id_1).await;
        handler.handle_rate_limit("openai", key_id_2).await;

        // Exhaust cycle 2
        handler.handle_rate_limit("openai", key_id_1).await;
        handler.handle_rate_limit("openai", key_id_2).await;

        // Should exceed max
        handler.handle_rate_limit("openai", key_id_1).await;
        let decision = handler.handle_rate_limit("openai", key_id_2).await;

        assert!(matches!(decision, RetryDecision::GiveUp { .. }));
    }

    #[tokio::test]
    async fn test_mark_success_resets_exhaustion() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let config = RetryConfig {
            max_cycles: 3,
            cooldown_secs: 0,
            ..Default::default()
        };
        let handler = RetryHandler::with_config(db, config);
        handler.load_provider("openai").unwrap();

        // Build up exhaustion
        handler.handle_rate_limit("openai", key_id_1).await;
        handler.handle_rate_limit("openai", key_id_2).await; // cycle 1 complete

        // Success resets
        handler.mark_success("openai", key_id_1);

        // Next exhaustion should be cycle 1 again, not cycle 2
        handler.handle_rate_limit("openai", key_id_1).await;
        let decision = handler.handle_rate_limit("openai", key_id_2).await;

        assert!(matches!(decision, RetryDecision::WaitAndRetry { .. }));
    }

    #[tokio::test]
    async fn test_single_key_goes_to_exhausted() {
        let (_temp, db) = setup_test_db();
        let key_id = add_pool_key(&db, "openai", "sk-only-key", 1);
        let config = RetryConfig {
            max_cycles: 2,
            cooldown_secs: 1,
            ..Default::default()
        };
        let handler = RetryHandler::with_config(db, config);
        handler.load_provider("openai").unwrap();

        // Single key can't rotate, goes straight to exhausted
        let decision = handler.handle_rate_limit("openai", key_id).await;
        assert!(matches!(decision, RetryDecision::WaitAndRetry { .. }));
    }

    // =========================================================================
    // Pool Manager Access Tests
    // =========================================================================

    #[test]
    fn test_pool_manager_accessible() {
        let (_temp, db) = setup_test_db();
        let handler = RetryHandler::new(db);
        let _pool_manager = handler.pool_manager();
        // Just verify we can access it
    }
}
