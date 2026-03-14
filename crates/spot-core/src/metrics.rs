//! Session metrics for tracking token usage and API activity.
//!
//! Provides a thread-safe `SessionMetrics` struct that accumulates
//! statistics during an agent session, including per-model token breakdowns.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Per-model token breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelTokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u64,
}

/// Session-level metrics for token/cost tracking.
///
/// All fields use atomic operations so metrics can be updated
/// from multiple concurrent tasks without locking (except the
/// per-model breakdown which uses a mutex).
pub struct SessionMetrics {
    pub total_input_tokens: AtomicU64,
    pub total_output_tokens: AtomicU64,
    pub tool_call_count: AtomicU64,
    pub api_request_count: AtomicU64,
    session_start: Instant,
    per_model: Mutex<HashMap<String, ModelTokenUsage>>,
}

impl Default for SessionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMetrics {
    /// Create a new metrics tracker with the current time as session start.
    pub fn new() -> Self {
        Self {
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
            tool_call_count: AtomicU64::new(0),
            api_request_count: AtomicU64::new(0),
            session_start: Instant::now(),
            per_model: Mutex::new(HashMap::new()),
        }
    }

    /// Record tokens from a single API request.
    pub fn record_tokens(&self, model_name: &str, input_tokens: u64, output_tokens: u64) {
        self.total_input_tokens
            .fetch_add(input_tokens, Ordering::Relaxed);
        self.total_output_tokens
            .fetch_add(output_tokens, Ordering::Relaxed);
        self.api_request_count.fetch_add(1, Ordering::Relaxed);

        if let Ok(mut map) = self.per_model.lock() {
            let entry = map
                .entry(model_name.to_string())
                .or_insert_with(ModelTokenUsage::default);
            entry.input_tokens += input_tokens;
            entry.output_tokens += output_tokens;
            entry.requests += 1;
        }
    }

    /// Record a tool call.
    pub fn record_tool_call(&self) {
        self.tool_call_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the elapsed session duration in seconds.
    pub fn elapsed_secs(&self) -> f64 {
        self.session_start.elapsed().as_secs_f64()
    }

    /// Get a snapshot of current metrics.
    pub fn snapshot(&self) -> SessionMetricsSnapshot {
        let per_model = self.per_model.lock().map(|m| m.clone()).unwrap_or_default();

        SessionMetricsSnapshot {
            total_input_tokens: self.total_input_tokens.load(Ordering::Relaxed),
            total_output_tokens: self.total_output_tokens.load(Ordering::Relaxed),
            tool_call_count: self.tool_call_count.load(Ordering::Relaxed),
            api_request_count: self.api_request_count.load(Ordering::Relaxed),
            elapsed_secs: self.elapsed_secs(),
            per_model,
        }
    }
}

/// Immutable snapshot of session metrics, suitable for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetricsSnapshot {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub tool_call_count: u64,
    pub api_request_count: u64,
    pub elapsed_secs: f64,
    pub per_model: HashMap<String, ModelTokenUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_metrics_are_zero() {
        let m = SessionMetrics::new();
        assert_eq!(m.total_input_tokens.load(Ordering::Relaxed), 0);
        assert_eq!(m.total_output_tokens.load(Ordering::Relaxed), 0);
        assert_eq!(m.tool_call_count.load(Ordering::Relaxed), 0);
        assert_eq!(m.api_request_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_tokens_accumulates() {
        let m = SessionMetrics::new();
        m.record_tokens("gpt-4", 100, 50);
        m.record_tokens("gpt-4", 200, 150);
        assert_eq!(m.total_input_tokens.load(Ordering::Relaxed), 300);
        assert_eq!(m.total_output_tokens.load(Ordering::Relaxed), 200);
        assert_eq!(m.api_request_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_per_model_breakdown() {
        let m = SessionMetrics::new();
        m.record_tokens("gpt-4", 100, 50);
        m.record_tokens("claude-3", 200, 80);
        m.record_tokens("gpt-4", 50, 30);

        let snap = m.snapshot();
        assert_eq!(snap.per_model.len(), 2);
        let gpt4 = snap.per_model.get("gpt-4").unwrap();
        assert_eq!(gpt4.input_tokens, 150);
        assert_eq!(gpt4.output_tokens, 80);
        assert_eq!(gpt4.requests, 2);

        let claude = snap.per_model.get("claude-3").unwrap();
        assert_eq!(claude.input_tokens, 200);
        assert_eq!(claude.output_tokens, 80);
        assert_eq!(claude.requests, 1);
    }

    #[test]
    fn test_record_tool_call() {
        let m = SessionMetrics::new();
        m.record_tool_call();
        m.record_tool_call();
        m.record_tool_call();
        assert_eq!(m.tool_call_count.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_snapshot_reflects_current_state() {
        let m = SessionMetrics::new();
        m.record_tokens("model-a", 500, 250);
        m.record_tool_call();

        let snap = m.snapshot();
        assert_eq!(snap.total_input_tokens, 500);
        assert_eq!(snap.total_output_tokens, 250);
        assert_eq!(snap.tool_call_count, 1);
        assert_eq!(snap.api_request_count, 1);
        assert!(snap.elapsed_secs >= 0.0);
    }

    #[test]
    fn test_elapsed_secs_positive() {
        let m = SessionMetrics::new();
        // Even immediately, elapsed should be non-negative
        assert!(m.elapsed_secs() >= 0.0);
    }

    #[test]
    fn test_default_trait() {
        let m = SessionMetrics::default();
        let snap = m.snapshot();
        assert_eq!(snap.total_input_tokens, 0);
        assert_eq!(snap.total_output_tokens, 0);
    }
}
