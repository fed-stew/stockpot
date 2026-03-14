//! Error resilience patterns for API calls.
//!
//! Provides:
//! - `CircuitBreaker`: Prevents cascading failures by stopping requests to failing providers
//! - `RetryPolicy`: Configurable retry with exponential backoff and jitter
//! - `FallbackChain`: Maps models to fallback alternatives when a provider is unavailable

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{info, warn};

// =========================================================================
// Circuit Breaker
// =========================================================================

/// State of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation. Requests flow through, failures are tracked.
    Closed,
    /// Circuit tripped. All requests immediately fail until `reset_timeout` elapses.
    Open,
    /// Probe state after timeout. One request is allowed through to test recovery.
    HalfOpen,
}

/// Circuit breaker for provider API calls.
///
/// Prevents cascading failures by tracking consecutive failures and
/// short-circuiting requests when a provider is consistently failing.
///
/// State transitions:
/// - **Closed -> Open**: When `failure_count` reaches `failure_threshold`.
/// - **Open -> HalfOpen**: After `reset_timeout` has elapsed since the last failure.
/// - **HalfOpen -> Closed**: When a probe request succeeds.
/// - **HalfOpen -> Open**: When a probe request fails.
pub struct CircuitBreaker {
    inner: Mutex<CircuitBreakerInner>,
}

struct CircuitBreakerInner {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,
    reset_timeout: Duration,
    last_failure: Option<Instant>,
    /// Name of the provider this breaker protects (for logging).
    provider: String,
}

impl CircuitBreaker {
    /// Create a new circuit breaker for the given provider.
    ///
    /// # Arguments
    /// - `provider`: Name of the provider (for logging)
    /// - `failure_threshold`: Number of consecutive failures before the circuit opens (e.g., 5)
    /// - `reset_timeout`: How long to wait before probing again (e.g., 60s)
    pub fn new(provider: &str, failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            inner: Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                failure_count: 0,
                failure_threshold,
                reset_timeout,
                last_failure: None,
                provider: provider.to_string(),
            }),
        }
    }

    /// Create a circuit breaker with sensible defaults (5 failures, 60s timeout).
    pub fn with_defaults(provider: &str) -> Self {
        Self::new(provider, 5, Duration::from_secs(60))
    }

    /// Check whether a request should be allowed through.
    ///
    /// Returns `Ok(())` if the request can proceed, or `Err(message)` if the
    /// circuit is open.
    pub fn check(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();

        match inner.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if enough time has passed to transition to HalfOpen
                if let Some(last_failure) = inner.last_failure {
                    if last_failure.elapsed() >= inner.reset_timeout {
                        info!(
                            provider = %inner.provider,
                            "Circuit breaker transitioning to HalfOpen (probe allowed)"
                        );
                        inner.state = CircuitState::HalfOpen;
                        Ok(())
                    } else {
                        let remaining = inner.reset_timeout - last_failure.elapsed();
                        Err(format!(
                            "Circuit breaker open for provider '{}'. \
                             Too many consecutive failures ({}). \
                             Will retry in {:.0}s.",
                            inner.provider,
                            inner.failure_count,
                            remaining.as_secs_f64()
                        ))
                    }
                } else {
                    // No last failure recorded, shouldn't happen, but allow
                    inner.state = CircuitState::HalfOpen;
                    Ok(())
                }
            }
            CircuitState::HalfOpen => {
                // Already in HalfOpen, allow the probe request
                Ok(())
            }
        }
    }

    /// Record a successful request. Resets the circuit to Closed.
    pub fn record_success(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.state != CircuitState::Closed {
            info!(
                provider = %inner.provider,
                previous_state = ?inner.state,
                "Circuit breaker closing after successful request"
            );
        }
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.last_failure = None;
    }

    /// Record a failed request. May trip the circuit to Open.
    pub fn record_failure(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.failure_count += 1;
        inner.last_failure = Some(Instant::now());

        match inner.state {
            CircuitState::Closed => {
                if inner.failure_count >= inner.failure_threshold {
                    warn!(
                        provider = %inner.provider,
                        failure_count = inner.failure_count,
                        threshold = inner.failure_threshold,
                        timeout_secs = inner.reset_timeout.as_secs(),
                        "Circuit breaker OPEN - provider has too many consecutive failures"
                    );
                    inner.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Probe failed, go back to Open
                warn!(
                    provider = %inner.provider,
                    "Circuit breaker probe failed, returning to Open state"
                );
                inner.state = CircuitState::Open;
            }
            CircuitState::Open => {
                // Already open, just update failure tracking
            }
        }
    }

    /// Get the current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        let inner = self.inner.lock().unwrap();
        inner.state
    }

    /// Get the current failure count.
    pub fn failure_count(&self) -> u32 {
        let inner = self.inner.lock().unwrap();
        inner.failure_count
    }

    /// Get the provider name.
    pub fn provider(&self) -> String {
        let inner = self.inner.lock().unwrap();
        inner.provider.clone()
    }

    /// Reset the circuit breaker to its initial closed state.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.last_failure = None;
    }
}

// =========================================================================
// Retry Policy
// =========================================================================

/// Configurable retry policy with exponential backoff and optional jitter.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries (default: 3).
    pub max_retries: u32,
    /// Initial delay before the first retry (default: 1s).
    pub base_delay: Duration,
    /// Maximum delay cap (default: 30s).
    pub max_delay: Duration,
    /// Whether to add random jitter to delays (default: true).
    pub jitter: bool,
    /// Multiplier for exponential backoff (default: 2.0).
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            jitter: true,
            backoff_factor: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of retries.
    pub fn with_max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Set the base delay.
    pub fn with_base_delay(mut self, d: Duration) -> Self {
        self.base_delay = d;
        self
    }

    /// Set the maximum delay cap.
    pub fn with_max_delay(mut self, d: Duration) -> Self {
        self.max_delay = d;
        self
    }

    /// Enable or disable jitter.
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Set the backoff factor.
    pub fn with_backoff_factor(mut self, factor: f64) -> Self {
        self.backoff_factor = factor;
        self
    }

    /// Calculate the delay for a given attempt number (0-indexed).
    ///
    /// Uses exponential backoff: `base_delay * backoff_factor^attempt`,
    /// capped at `max_delay`, with optional jitter.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_ms = self.base_delay.as_millis() as f64;
        let delay_ms = base_ms * self.backoff_factor.powi(attempt as i32);
        let capped_ms = delay_ms.min(self.max_delay.as_millis() as f64);

        let final_ms = if self.jitter {
            // Add jitter: random value between 0% and 100% of the computed delay
            let jitter_factor = pseudo_random_factor(attempt);
            capped_ms * jitter_factor
        } else {
            capped_ms
        };

        Duration::from_millis(final_ms.max(0.0) as u64)
    }

    /// Check if the given attempt number (0-indexed) is within the retry limit.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Simple deterministic pseudo-random factor for jitter based on attempt number.
///
/// Returns a value between 0.5 and 1.5 to add variety to delays without
/// requiring a full RNG dependency. In production, the actual timing
/// variations from system scheduling provide additional natural jitter.
fn pseudo_random_factor(attempt: u32) -> f64 {
    // Use a simple hash-like computation for deterministic but varied output
    let hash = ((attempt.wrapping_mul(2654435761)) >> 16) & 0xFFFF;
    let normalized = hash as f64 / 65535.0; // 0.0 to 1.0
    0.5 + normalized // 0.5 to 1.5
}

// =========================================================================
// Fallback Chain
// =========================================================================

/// Maps primary models to fallback alternatives.
///
/// When a provider's circuit breaker is open, the fallback chain provides
/// an alternative model to try instead. Fallbacks are configurable and
/// not hardcoded.
///
/// # Example
/// ```ignore
/// let mut chain = FallbackChain::new();
/// chain.add_fallback("gpt-4o", "claude-sonnet-4-20250514");
/// chain.add_fallback("claude-sonnet-4-20250514", "gpt-4o");
///
/// if let Some(alt) = chain.get_fallback("gpt-4o") {
///     // Use alt model instead
/// }
/// ```
pub struct FallbackChain {
    /// Maps model name -> fallback model name.
    mappings: HashMap<String, String>,
}

impl FallbackChain {
    /// Create an empty fallback chain.
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Create a fallback chain from a map of model -> fallback pairs.
    pub fn from_mappings(mappings: HashMap<String, String>) -> Self {
        Self { mappings }
    }

    /// Add a fallback mapping.
    pub fn add_fallback(&mut self, model: &str, fallback: &str) {
        self.mappings
            .insert(model.to_string(), fallback.to_string());
    }

    /// Get the fallback model for a given model, if one exists.
    pub fn get_fallback(&self, model: &str) -> Option<&str> {
        self.mappings.get(model).map(|s| s.as_str())
    }

    /// Check if any fallbacks are configured.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Get the number of fallback mappings.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Get all configured mappings.
    pub fn mappings(&self) -> &HashMap<String, String> {
        &self.mappings
    }
}

impl Default for FallbackChain {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // CircuitBreaker Tests
    // =====================================================================

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::with_defaults("test-provider");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_allows_requests_when_closed() {
        let cb = CircuitBreaker::with_defaults("test-provider");
        assert!(cb.check().is_ok());
    }

    #[test]
    fn test_circuit_breaker_tracks_failures() {
        let cb = CircuitBreaker::new("test", 5, Duration::from_secs(60));

        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 3);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_opens_at_threshold() {
        let cb = CircuitBreaker::new("test", 3, Duration::from_secs(60));

        cb.record_failure(); // 1
        cb.record_failure(); // 2
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure(); // 3 = threshold
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_blocks_when_open() {
        let cb = CircuitBreaker::new("test", 2, Duration::from_secs(60));

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        let result = cb.check();
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Circuit breaker open"));
        assert!(err_msg.contains("test"));
    }

    #[test]
    fn test_circuit_breaker_transitions_to_half_open_after_timeout() {
        let cb = CircuitBreaker::new("test", 2, Duration::from_millis(0));

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // With 0ms timeout, the next check should transition to HalfOpen
        let result = cb.check();
        assert!(result.is_ok());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_half_open_success_closes() {
        let cb = CircuitBreaker::new("test", 2, Duration::from_millis(0));

        // Open it
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Transition to HalfOpen
        cb.check().unwrap();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record success -> Closed
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let cb = CircuitBreaker::new("test", 2, Duration::from_millis(0));

        // Open it
        cb.record_failure();
        cb.record_failure();

        // Transition to HalfOpen
        cb.check().unwrap();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record failure -> back to Open
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_success_resets_failure_count() {
        let cb = CircuitBreaker::new("test", 5, Duration::from_secs(60));

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let cb = CircuitBreaker::new("test", 2, Duration::from_secs(60));

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_provider_name() {
        let cb = CircuitBreaker::with_defaults("my-provider");
        assert_eq!(cb.provider(), "my-provider");
    }

    #[test]
    fn test_circuit_breaker_custom_threshold_and_timeout() {
        let cb = CircuitBreaker::new("custom", 10, Duration::from_secs(120));

        // Should not open until 10 failures
        for _ in 0..9 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure(); // 10th
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_multiple_successes_keep_closed() {
        let cb = CircuitBreaker::new("test", 3, Duration::from_secs(60));

        cb.record_success();
        cb.record_success();
        cb.record_success();

        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_interleaved_success_resets() {
        let cb = CircuitBreaker::new("test", 3, Duration::from_secs(60));

        cb.record_failure(); // 1
        cb.record_failure(); // 2
        cb.record_success(); // resets
        cb.record_failure(); // 1
        cb.record_failure(); // 2

        // Should still be closed since success reset the count
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 2);
    }

    #[test]
    fn test_circuit_state_debug_format() {
        assert_eq!(format!("{:?}", CircuitState::Closed), "Closed");
        assert_eq!(format!("{:?}", CircuitState::Open), "Open");
        assert_eq!(format!("{:?}", CircuitState::HalfOpen), "HalfOpen");
    }

    #[test]
    fn test_circuit_state_equality() {
        assert_eq!(CircuitState::Closed, CircuitState::Closed);
        assert_eq!(CircuitState::Open, CircuitState::Open);
        assert_eq!(CircuitState::HalfOpen, CircuitState::HalfOpen);
        assert_ne!(CircuitState::Closed, CircuitState::Open);
        assert_ne!(CircuitState::Open, CircuitState::HalfOpen);
    }

    // =====================================================================
    // RetryPolicy Tests
    // =====================================================================

    #[test]
    fn test_retry_policy_defaults() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.base_delay, Duration::from_secs(1));
        assert_eq!(policy.max_delay, Duration::from_secs(30));
        assert!(policy.jitter);
        assert!((policy.backoff_factor - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_policy_builder() {
        let policy = RetryPolicy::new()
            .with_max_retries(5)
            .with_base_delay(Duration::from_millis(500))
            .with_max_delay(Duration::from_secs(60))
            .with_jitter(false)
            .with_backoff_factor(3.0);

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.base_delay, Duration::from_millis(500));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
        assert!(!policy.jitter);
        assert!((policy.backoff_factor - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::new().with_max_retries(3);

        assert!(policy.should_retry(0)); // 1st retry
        assert!(policy.should_retry(1)); // 2nd retry
        assert!(policy.should_retry(2)); // 3rd retry
        assert!(!policy.should_retry(3)); // exceeded
        assert!(!policy.should_retry(10)); // way exceeded
    }

    #[test]
    fn test_retry_policy_exponential_backoff_no_jitter() {
        let policy = RetryPolicy::new()
            .with_base_delay(Duration::from_secs(1))
            .with_backoff_factor(2.0)
            .with_max_delay(Duration::from_secs(60))
            .with_jitter(false);

        assert_eq!(policy.delay_for_attempt(0), Duration::from_secs(1)); // 1 * 2^0
        assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(2)); // 1 * 2^1
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(4)); // 1 * 2^2
        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(8)); // 1 * 2^3
    }

    #[test]
    fn test_retry_policy_delay_capped_at_max() {
        let policy = RetryPolicy::new()
            .with_base_delay(Duration::from_secs(1))
            .with_backoff_factor(10.0)
            .with_max_delay(Duration::from_secs(30))
            .with_jitter(false);

        // 1 * 10^0 = 1s
        assert_eq!(policy.delay_for_attempt(0), Duration::from_secs(1));
        // 1 * 10^1 = 10s
        assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(10));
        // 1 * 10^2 = 100s, capped to 30s
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(30));
        // 1 * 10^3 = 1000s, capped to 30s
        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(30));
    }

    #[test]
    fn test_retry_policy_with_jitter_varies() {
        let policy = RetryPolicy::new()
            .with_base_delay(Duration::from_secs(1))
            .with_backoff_factor(2.0)
            .with_max_delay(Duration::from_secs(60))
            .with_jitter(true);

        let d0 = policy.delay_for_attempt(0);
        let d1 = policy.delay_for_attempt(1);

        // With jitter, delays should be between 0.5x and 1.5x of the base
        assert!(d0 >= Duration::from_millis(500));
        assert!(d0 <= Duration::from_millis(1500));

        // Second attempt base is 2s, jittered between 1s and 3s
        assert!(d1 >= Duration::from_millis(1000));
        assert!(d1 <= Duration::from_millis(3000));
    }

    #[test]
    fn test_retry_policy_zero_retries() {
        let policy = RetryPolicy::new().with_max_retries(0);
        assert!(!policy.should_retry(0));
    }

    #[test]
    fn test_pseudo_random_factor_range() {
        for attempt in 0..100 {
            let factor = pseudo_random_factor(attempt);
            assert!(
                (0.5..=1.5).contains(&factor),
                "Factor {} out of range for attempt {}",
                factor,
                attempt
            );
        }
    }

    // =====================================================================
    // FallbackChain Tests
    // =====================================================================

    #[test]
    fn test_fallback_chain_empty() {
        let chain = FallbackChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert!(chain.get_fallback("gpt-4o").is_none());
    }

    #[test]
    fn test_fallback_chain_add_and_get() {
        let mut chain = FallbackChain::new();
        chain.add_fallback("gpt-4o", "claude-sonnet-4-20250514");

        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 1);
        assert_eq!(
            chain.get_fallback("gpt-4o"),
            Some("claude-sonnet-4-20250514")
        );
    }

    #[test]
    fn test_fallback_chain_missing_model() {
        let mut chain = FallbackChain::new();
        chain.add_fallback("gpt-4o", "claude-sonnet-4-20250514");

        assert!(chain.get_fallback("gpt-3.5-turbo").is_none());
    }

    #[test]
    fn test_fallback_chain_bidirectional() {
        let mut chain = FallbackChain::new();
        chain.add_fallback("gpt-4o", "claude-sonnet-4-20250514");
        chain.add_fallback("claude-sonnet-4-20250514", "gpt-4o");

        assert_eq!(
            chain.get_fallback("gpt-4o"),
            Some("claude-sonnet-4-20250514")
        );
        assert_eq!(
            chain.get_fallback("claude-sonnet-4-20250514"),
            Some("gpt-4o")
        );
    }

    #[test]
    fn test_fallback_chain_from_mappings() {
        let mut mappings = HashMap::new();
        mappings.insert("model-a".to_string(), "model-b".to_string());
        mappings.insert("model-c".to_string(), "model-d".to_string());

        let chain = FallbackChain::from_mappings(mappings);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.get_fallback("model-a"), Some("model-b"));
        assert_eq!(chain.get_fallback("model-c"), Some("model-d"));
    }

    #[test]
    fn test_fallback_chain_overwrite() {
        let mut chain = FallbackChain::new();
        chain.add_fallback("gpt-4o", "claude-sonnet-4-20250514");
        chain.add_fallback("gpt-4o", "gemini-pro");

        // Should have the latest mapping
        assert_eq!(chain.get_fallback("gpt-4o"), Some("gemini-pro"));
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_fallback_chain_mappings_accessor() {
        let mut chain = FallbackChain::new();
        chain.add_fallback("a", "b");
        chain.add_fallback("c", "d");

        let mappings = chain.mappings();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings.get("a").unwrap(), "b");
        assert_eq!(mappings.get("c").unwrap(), "d");
    }

    #[test]
    fn test_fallback_chain_default() {
        let chain = FallbackChain::default();
        assert!(chain.is_empty());
    }
}
