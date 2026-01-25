//! API Key Pool Manager for multi-key rotation and failover.
//!
//! Provides intelligent key rotation when rate limits (429) are hit,
//! with configurable retry cycles and cooldown periods.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::db::{Database, PoolKey};

/// Configuration for the key pool retry behavior
#[derive(Debug, Clone)]
pub struct KeyPoolConfig {
    /// Maximum number of full rotation cycles before giving up (default: 5)
    pub max_retry_cycles: u32,
    /// Time to wait when all keys are exhausted before retrying (default: 15s)
    pub cooldown_duration: Duration,
    /// Whether to track usage statistics in the database
    pub track_stats: bool,
}

impl Default for KeyPoolConfig {
    fn default() -> Self {
        Self {
            max_retry_cycles: 5,
            cooldown_duration: Duration::from_secs(15),
            track_stats: true,
        }
    }
}

/// Result of attempting to rotate to the next key
#[derive(Debug, Clone, PartialEq)]
pub enum RotationResult {
    /// Successfully rotated to a new key
    Rotated { key: String, key_id: i64 },
    /// All keys exhausted, need to wait for cooldown
    Exhausted { cycle: u32, max_cycles: u32 },
    /// No keys configured for this provider
    NoKeys,
    /// Max retry cycles reached, giving up
    MaxRetriesExceeded,
}

/// Internal state for a provider's key pool
struct ProviderPoolState {
    keys: Vec<PoolKey>,
    current_index: AtomicUsize,
    exhaustion_cycle: AtomicU32,
    last_exhaustion: RwLock<Option<Instant>>,
}

impl ProviderPoolState {
    fn new(keys: Vec<PoolKey>) -> Self {
        Self {
            keys,
            current_index: AtomicUsize::new(0),
            exhaustion_cycle: AtomicU32::new(0),
            last_exhaustion: RwLock::new(None),
        }
    }

    fn current_key(&self) -> Option<&PoolKey> {
        let idx = self.current_index.load(Ordering::SeqCst);
        self.keys.get(idx)
    }

    fn rotate_to_next(&self) -> Option<&PoolKey> {
        if self.keys.is_empty() {
            return None;
        }
        let new_idx = (self.current_index.load(Ordering::SeqCst) + 1) % self.keys.len();
        self.current_index.store(new_idx, Ordering::SeqCst);
        self.keys.get(new_idx)
    }

    fn is_cycle_complete(&self) -> bool {
        self.current_index.load(Ordering::SeqCst) == 0
    }

    fn reset(&self) {
        self.current_index.store(0, Ordering::SeqCst);
        self.exhaustion_cycle.store(0, Ordering::SeqCst);
        *self.last_exhaustion.write() = None;
    }
}

/// Manages API key pools for multiple providers with rotation and failover
pub struct ApiKeyPoolManager {
    db: Arc<Database>,
    pools: RwLock<HashMap<String, ProviderPoolState>>,
    config: KeyPoolConfig,
}

impl ApiKeyPoolManager {
    /// Create a new pool manager
    pub fn new(db: Arc<Database>, config: KeyPoolConfig) -> Self {
        Self {
            db,
            pools: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(db: Arc<Database>) -> Self {
        Self::new(db, KeyPoolConfig::default())
    }

    /// Load or refresh keys for a provider from the database
    pub fn load_provider(&self, provider: &str) -> Result<usize, rusqlite::Error> {
        let keys = self.db.get_active_pool_keys(provider)?;
        let count = keys.len();

        let mut pools = self.pools.write();
        pools.insert(provider.to_string(), ProviderPoolState::new(keys));

        debug!(provider = %provider, key_count = count, "Loaded key pool");
        Ok(count)
    }

    /// Check if a provider has multiple keys configured (worth using rotation)
    pub fn has_multiple_keys(&self, provider: &str) -> bool {
        // Check cache first
        if let Some(state) = self.pools.read().get(provider) {
            return state.keys.len() > 1;
        }
        // Check database
        self.db.count_active_pool_keys(provider).unwrap_or(0) > 1
    }

    /// Check if provider has any pool keys
    pub fn has_pool_keys(&self, provider: &str) -> bool {
        if let Some(state) = self.pools.read().get(provider) {
            return !state.keys.is_empty();
        }
        self.db.has_pool_keys(provider)
    }

    /// Get the current active key for a provider
    /// Returns None if no keys configured or pool not loaded
    pub fn get_current_key(&self, provider: &str) -> Option<(String, i64)> {
        // Try to get from cache
        if let Some(state) = self.pools.read().get(provider) {
            if let Some(key) = state.current_key() {
                return Some((key.api_key.clone(), key.id));
            }
        }

        // Try loading from database
        if self.load_provider(provider).ok()? > 0 {
            let pools = self.pools.read();
            let state = pools.get(provider)?;
            let key = state.current_key()?;
            return Some((key.api_key.clone(), key.id));
        }

        None
    }

    /// Rotate to the next key after a rate limit error
    /// Returns the rotation result indicating what action to take
    pub async fn rotate_on_rate_limit(&self, provider: &str, failed_key_id: i64) -> RotationResult {
        // Mark the error in database
        if self.config.track_stats {
            let _ = self.db.mark_key_error(failed_key_id);
        }

        // Get pool state
        let pools = self.pools.read();
        let Some(state) = pools.get(provider) else {
            return RotationResult::NoKeys;
        };

        if state.keys.is_empty() {
            return RotationResult::NoKeys;
        }

        // Single key - can't rotate, must wait
        if state.keys.len() == 1 {
            let cycle = state.exhaustion_cycle.fetch_add(1, Ordering::SeqCst) + 1;
            if cycle > self.config.max_retry_cycles {
                return RotationResult::MaxRetriesExceeded;
            }
            *state.last_exhaustion.write() = Some(Instant::now());
            return RotationResult::Exhausted {
                cycle,
                max_cycles: self.config.max_retry_cycles,
            };
        }

        // Rotate to next key
        if let Some(next_key) = state.rotate_to_next() {
            // Check if we've completed a full cycle (back to first key)
            if state.is_cycle_complete() {
                let cycle = state.exhaustion_cycle.fetch_add(1, Ordering::SeqCst) + 1;
                if cycle > self.config.max_retry_cycles {
                    return RotationResult::MaxRetriesExceeded;
                }
                *state.last_exhaustion.write() = Some(Instant::now());
                return RotationResult::Exhausted {
                    cycle,
                    max_cycles: self.config.max_retry_cycles,
                };
            }

            info!(
                provider = %provider,
                key_id = next_key.id,
                label = ?next_key.label,
                "Rotated to next API key"
            );

            return RotationResult::Rotated {
                key: next_key.api_key.clone(),
                key_id: next_key.id,
            };
        }

        RotationResult::NoKeys
    }

    /// Wait for the cooldown period (call when Exhausted is returned)
    pub async fn wait_cooldown(&self) {
        info!(
            duration_secs = self.config.cooldown_duration.as_secs(),
            "All API keys rate-limited, waiting for cooldown"
        );
        sleep(self.config.cooldown_duration).await;
    }

    /// Mark a key as successfully used (resets error count)
    pub fn mark_success(&self, provider: &str, key_id: i64) {
        if self.config.track_stats {
            let _ = self.db.mark_key_used(key_id);
            let _ = self.db.reset_key_errors(key_id);
        }

        // Reset exhaustion cycle on success
        if let Some(state) = self.pools.read().get(provider) {
            state.exhaustion_cycle.store(0, Ordering::SeqCst);
        }
    }

    /// Reset the pool state for a provider (e.g., after manual intervention)
    pub fn reset_pool(&self, provider: &str) {
        if let Some(state) = self.pools.read().get(provider) {
            state.reset();
        }
    }

    /// Get current exhaustion status for a provider
    pub fn get_exhaustion_status(&self, provider: &str) -> Option<(u32, u32)> {
        self.pools.read().get(provider).map(|state| {
            (
                state.exhaustion_cycle.load(Ordering::SeqCst),
                self.config.max_retry_cycles,
            )
        })
    }

    /// Reload all provider pools from database
    pub fn reload_all(&self) -> Result<(), rusqlite::Error> {
        let providers: Vec<String> = self.pools.read().keys().cloned().collect();
        for provider in providers {
            self.load_provider(&provider)?;
        }
        Ok(())
    }

    /// Get the configuration (for testing)
    #[cfg(test)]
    pub fn config(&self) -> &KeyPoolConfig {
        &self.config
    }
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

    fn create_pool_manager(db: Arc<Database>) -> ApiKeyPoolManager {
        ApiKeyPoolManager::with_defaults(db)
    }

    fn create_pool_manager_with_config(
        db: Arc<Database>,
        max_cycles: u32,
        cooldown: Duration,
    ) -> ApiKeyPoolManager {
        let config = KeyPoolConfig {
            max_retry_cycles: max_cycles,
            cooldown_duration: cooldown,
            track_stats: true,
        };
        ApiKeyPoolManager::new(db, config)
    }

    fn add_pool_key(db: &Database, provider: &str, api_key: &str, priority: i32) -> i64 {
        db.save_pool_key(provider, api_key, None, Some(priority))
            .unwrap()
    }

    fn add_pool_key_with_label(
        db: &Database,
        provider: &str,
        api_key: &str,
        priority: i32,
        label: &str,
    ) -> i64 {
        db.save_pool_key(provider, api_key, Some(label), Some(priority))
            .unwrap()
    }

    // =========================================================================
    // KeyPoolConfig Tests
    // =========================================================================

    #[test]
    fn test_key_pool_config_default() {
        let config = KeyPoolConfig::default();
        assert_eq!(config.max_retry_cycles, 5);
        assert_eq!(config.cooldown_duration, Duration::from_secs(15));
        assert!(config.track_stats);
    }

    #[test]
    fn test_key_pool_config_custom() {
        let config = KeyPoolConfig {
            max_retry_cycles: 10,
            cooldown_duration: Duration::from_secs(30),
            track_stats: false,
        };
        assert_eq!(config.max_retry_cycles, 10);
        assert_eq!(config.cooldown_duration, Duration::from_secs(30));
        assert!(!config.track_stats);
    }

    // =========================================================================
    // RotationResult Tests
    // =========================================================================

    #[test]
    fn test_rotation_result_equality() {
        let r1 = RotationResult::Rotated {
            key: "key1".to_string(),
            key_id: 1,
        };
        let r2 = RotationResult::Rotated {
            key: "key1".to_string(),
            key_id: 1,
        };
        assert_eq!(r1, r2);

        let r3 = RotationResult::Exhausted {
            cycle: 1,
            max_cycles: 5,
        };
        let r4 = RotationResult::Exhausted {
            cycle: 1,
            max_cycles: 5,
        };
        assert_eq!(r3, r4);

        assert_eq!(RotationResult::NoKeys, RotationResult::NoKeys);
        assert_eq!(
            RotationResult::MaxRetriesExceeded,
            RotationResult::MaxRetriesExceeded
        );
    }

    // =========================================================================
    // Basic Key Retrieval Tests
    // =========================================================================

    #[test]
    fn test_get_current_key_no_keys() {
        let (_temp, db) = setup_test_db();
        let manager = create_pool_manager(db);

        let result = manager.get_current_key("openai");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_current_key_single_key() {
        let (_temp, db) = setup_test_db();
        let key_id = add_pool_key(&db, "openai", "sk-test-key-1", 1);
        let manager = create_pool_manager(db);

        let result = manager.get_current_key("openai");
        assert!(result.is_some());
        let (key, id) = result.unwrap();
        assert_eq!(key, "sk-test-key-1");
        assert_eq!(id, key_id);
    }

    #[test]
    fn test_get_current_key_multiple_keys_returns_first() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let _key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager(db);

        let result = manager.get_current_key("openai");
        assert!(result.is_some());
        let (key, id) = result.unwrap();
        assert_eq!(key, "sk-key-1");
        assert_eq!(id, key_id_1);
    }

    #[test]
    fn test_get_current_key_caches_on_first_access() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        let manager = create_pool_manager(db);

        // First access loads from DB
        let result1 = manager.get_current_key("openai");
        assert!(result1.is_some());

        // Second access should be cached (we'd need deeper testing for this,
        // but at least verify it still works)
        let result2 = manager.get_current_key("openai");
        assert!(result2.is_some());
        assert_eq!(result1, result2);
    }

    // =========================================================================
    // has_multiple_keys / has_pool_keys Tests
    // =========================================================================

    #[test]
    fn test_has_multiple_keys_none() {
        let (_temp, db) = setup_test_db();
        let manager = create_pool_manager(db);

        assert!(!manager.has_multiple_keys("openai"));
    }

    #[test]
    fn test_has_multiple_keys_single() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        let manager = create_pool_manager(db);

        assert!(!manager.has_multiple_keys("openai"));
    }

    #[test]
    fn test_has_multiple_keys_two() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager(db);

        assert!(manager.has_multiple_keys("openai"));
    }

    #[test]
    fn test_has_pool_keys_none() {
        let (_temp, db) = setup_test_db();
        let manager = create_pool_manager(db);

        assert!(!manager.has_pool_keys("openai"));
    }

    #[test]
    fn test_has_pool_keys_exists() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        let manager = create_pool_manager(db);

        assert!(manager.has_pool_keys("openai"));
    }

    // =========================================================================
    // Rotation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_rotation_through_multiple_keys() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let key_id_3 = add_pool_key(&db, "openai", "sk-key-3", 3);
        let manager = create_pool_manager(db);

        // Load the provider first
        manager.load_provider("openai").unwrap();

        // First key is active
        let (key, id) = manager.get_current_key("openai").unwrap();
        assert_eq!(key, "sk-key-1");
        assert_eq!(id, key_id_1);

        // Rotate after rate limit on key 1
        let result = manager.rotate_on_rate_limit("openai", key_id_1).await;
        assert!(matches!(
            result,
            RotationResult::Rotated {
                key_id: id,
                ..
            } if id == key_id_2
        ));

        // Now key 2 is active
        let (key, id) = manager.get_current_key("openai").unwrap();
        assert_eq!(key, "sk-key-2");
        assert_eq!(id, key_id_2);

        // Rotate after rate limit on key 2
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert!(matches!(
            result,
            RotationResult::Rotated {
                key_id: id,
                ..
            } if id == key_id_3
        ));

        // Now key 3 is active
        let (key, id) = manager.get_current_key("openai").unwrap();
        assert_eq!(key, "sk-key-3");
        assert_eq!(id, key_id_3);
    }

    #[tokio::test]
    async fn test_rotation_no_keys_returns_no_keys() {
        let (_temp, db) = setup_test_db();
        let manager = create_pool_manager(db);

        let result = manager.rotate_on_rate_limit("openai", 999).await;
        assert_eq!(result, RotationResult::NoKeys);
    }

    // =========================================================================
    // Exhaustion Detection Tests
    // =========================================================================

    #[tokio::test]
    async fn test_exhaustion_when_all_keys_fail() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager_with_config(db, 5, Duration::from_millis(10));

        manager.load_provider("openai").unwrap();

        // Key 1 fails -> rotate to key 2
        let result = manager.rotate_on_rate_limit("openai", key_id_1).await;
        assert!(matches!(result, RotationResult::Rotated { key_id, .. } if key_id == key_id_2));

        // Key 2 fails -> back to key 1 (cycle complete) -> exhausted
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 1,
                max_cycles: 5
            }
        ));

        // After waiting, we can retry. Key 1 fails again -> rotate to key 2
        let result = manager.rotate_on_rate_limit("openai", key_id_1).await;
        assert!(matches!(result, RotationResult::Rotated { key_id, .. } if key_id == key_id_2));

        // Key 2 fails again -> exhausted cycle 2
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 2,
                max_cycles: 5
            }
        ));
    }

    // =========================================================================
    // Max Retry Cycle Enforcement Tests
    // =========================================================================

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager_with_config(db, 2, Duration::from_millis(1));

        manager.load_provider("openai").unwrap();

        // Cycle 1: key1 -> key2 -> exhausted
        manager.rotate_on_rate_limit("openai", key_id_1).await;
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 1,
                max_cycles: 2
            }
        ));

        // Cycle 2: key1 -> key2 -> exhausted
        manager.rotate_on_rate_limit("openai", key_id_1).await;
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 2,
                max_cycles: 2
            }
        ));

        // Cycle 3: should exceed max retries
        manager.rotate_on_rate_limit("openai", key_id_1).await;
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert_eq!(result, RotationResult::MaxRetriesExceeded);
    }

    // =========================================================================
    // Success Resets Exhaustion Counter Tests
    // =========================================================================

    #[tokio::test]
    async fn test_success_resets_exhaustion_counter() {
        let (_temp, db) = setup_test_db();
        let key_id_1 = add_pool_key(&db, "openai", "sk-key-1", 1);
        let key_id_2 = add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager_with_config(db, 3, Duration::from_millis(1));

        manager.load_provider("openai").unwrap();

        // Build up exhaustion cycles
        manager.rotate_on_rate_limit("openai", key_id_1).await; // -> key2
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await; // -> exhausted cycle 1
        assert!(matches!(result, RotationResult::Exhausted { cycle: 1, .. }));

        manager.rotate_on_rate_limit("openai", key_id_1).await; // -> key2
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await; // -> exhausted cycle 2
        assert!(matches!(result, RotationResult::Exhausted { cycle: 2, .. }));

        // Now success! This should reset the counter
        manager.mark_success("openai", key_id_1);

        // Check that exhaustion status is reset
        let (current_cycle, _) = manager.get_exhaustion_status("openai").unwrap();
        assert_eq!(current_cycle, 0);

        // Verify we can go through cycles again from the start
        manager.rotate_on_rate_limit("openai", key_id_1).await;
        let result = manager.rotate_on_rate_limit("openai", key_id_2).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 1, // Back to cycle 1, not 3!
                max_cycles: 3
            }
        ));
    }

    // =========================================================================
    // Single Key Behavior Tests
    // =========================================================================

    #[tokio::test]
    async fn test_single_key_goes_to_exhausted() {
        let (_temp, db) = setup_test_db();
        let key_id = add_pool_key(&db, "openai", "sk-only-key", 1);
        let manager = create_pool_manager_with_config(db, 3, Duration::from_millis(1));

        manager.load_provider("openai").unwrap();

        // First failure -> exhausted (can't rotate with only one key)
        let result = manager.rotate_on_rate_limit("openai", key_id).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 1,
                max_cycles: 3
            }
        ));

        // Second failure
        let result = manager.rotate_on_rate_limit("openai", key_id).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 2,
                max_cycles: 3
            }
        ));

        // Third failure
        let result = manager.rotate_on_rate_limit("openai", key_id).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 3,
                max_cycles: 3
            }
        ));

        // Fourth failure exceeds max
        let result = manager.rotate_on_rate_limit("openai", key_id).await;
        assert_eq!(result, RotationResult::MaxRetriesExceeded);
    }

    #[tokio::test]
    async fn test_single_key_success_resets() {
        let (_temp, db) = setup_test_db();
        let key_id = add_pool_key(&db, "openai", "sk-only-key", 1);
        let manager = create_pool_manager_with_config(db, 3, Duration::from_millis(1));

        manager.load_provider("openai").unwrap();

        // Two failures
        manager.rotate_on_rate_limit("openai", key_id).await;
        manager.rotate_on_rate_limit("openai", key_id).await;

        // Success resets
        manager.mark_success("openai", key_id);

        // Start over
        let result = manager.rotate_on_rate_limit("openai", key_id).await;
        assert!(matches!(
            result,
            RotationResult::Exhausted {
                cycle: 1,
                max_cycles: 3
            }
        ));
    }

    // =========================================================================
    // Pool Management Tests
    // =========================================================================

    #[test]
    fn test_load_provider() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager(db);

        let count = manager.load_provider("openai").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_load_provider_empty() {
        let (_temp, db) = setup_test_db();
        let manager = create_pool_manager(db);

        let count = manager.load_provider("openai").unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_reset_pool() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        add_pool_key(&db, "openai", "sk-key-2", 2);
        let manager = create_pool_manager(db);

        manager.load_provider("openai").unwrap();

        // Get exhaustion status - should be 0
        let (cycle, _) = manager.get_exhaustion_status("openai").unwrap();
        assert_eq!(cycle, 0);

        // Reset pool (should still be 0, but verifies the method works)
        manager.reset_pool("openai");
        let (cycle, _) = manager.get_exhaustion_status("openai").unwrap();
        assert_eq!(cycle, 0);
    }

    #[test]
    fn test_get_exhaustion_status_not_loaded() {
        let (_temp, db) = setup_test_db();
        let manager = create_pool_manager(db);

        // Provider not loaded
        let status = manager.get_exhaustion_status("openai");
        assert!(status.is_none());
    }

    #[test]
    fn test_reload_all() {
        let (_temp, db) = setup_test_db();
        add_pool_key(&db, "openai", "sk-key-1", 1);
        add_pool_key(&db, "anthropic", "sk-ant-1", 1);
        let manager = create_pool_manager(Arc::clone(&db));

        // Load providers
        manager.load_provider("openai").unwrap();
        manager.load_provider("anthropic").unwrap();

        // Add more keys
        add_pool_key(&db, "openai", "sk-key-2", 2);
        add_pool_key(&db, "anthropic", "sk-ant-2", 2);

        // Reload should pick up new keys
        manager.reload_all().unwrap();

        // Both should now have multiple keys
        assert!(manager.has_multiple_keys("openai"));
        assert!(manager.has_multiple_keys("anthropic"));
    }

    // =========================================================================
    // Provider Isolation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_providers_are_isolated() {
        let (_temp, db) = setup_test_db();
        let openai_key = add_pool_key(&db, "openai", "sk-openai-1", 1);
        let anthropic_key = add_pool_key(&db, "anthropic", "sk-ant-1", 1);
        let manager = create_pool_manager(db);

        manager.load_provider("openai").unwrap();
        manager.load_provider("anthropic").unwrap();

        // Fail openai
        let result = manager.rotate_on_rate_limit("openai", openai_key).await;
        assert!(matches!(result, RotationResult::Exhausted { cycle: 1, .. }));

        // Anthropic should still be at 0
        let (anthropic_cycle, _) = manager.get_exhaustion_status("anthropic").unwrap();
        assert_eq!(anthropic_cycle, 0);

        // Success on anthropic shouldn't affect openai
        manager.mark_success("anthropic", anthropic_key);
        let (openai_cycle, _) = manager.get_exhaustion_status("openai").unwrap();
        assert_eq!(openai_cycle, 1); // Still 1
    }

    // =========================================================================
    // Label Tests
    // =========================================================================

    #[test]
    fn test_key_with_label() {
        let (_temp, db) = setup_test_db();
        add_pool_key_with_label(&db, "openai", "sk-key-1", 1, "Primary Key");
        let manager = create_pool_manager(db);

        // Just verify we can load and access
        manager.load_provider("openai").unwrap();
        let (key, _) = manager.get_current_key("openai").unwrap();
        assert_eq!(key, "sk-key-1");
    }
}
