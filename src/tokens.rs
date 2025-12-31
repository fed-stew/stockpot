//! Token estimation utilities.
//!
//! Provides rough token counting for messages to help users
//! understand context usage and trigger compaction.

use serdes_ai_core::ModelRequest;

/// Rough token estimate for a collection of messages.
/// Uses ~4 chars per token approximation based on JSON serialization.
pub fn estimate_tokens(messages: &[ModelRequest]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

/// Estimate tokens for a single message.
pub fn estimate_message_tokens(msg: &ModelRequest) -> usize {
    // Serialize to JSON and estimate tokens from character count
    // ~4 chars per token is a reasonable approximation
    serde_json::to_string(msg)
        .map(|s| (s.len() / 4).max(10))
        .unwrap_or(25)
}

/// Check if context usage exceeds a threshold.
///
/// Returns true if the estimated token usage is at or above
/// the specified percentage of the context length.
pub fn should_compact(estimated_tokens: usize, context_length: usize, threshold: f64) -> bool {
    if context_length == 0 {
        return false;
    }
    let usage = estimated_tokens as f64 / context_length as f64;
    usage >= threshold
}

/// Calculate context usage as a percentage.
pub fn usage_percent(estimated_tokens: usize, context_length: usize) -> f64 {
    if context_length == 0 {
        return 0.0;
    }
    (estimated_tokens as f64 / context_length as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_empty() {
        let messages: Vec<ModelRequest> = vec![];
        assert_eq!(estimate_tokens(&messages), 0);
    }

    #[test]
    fn test_should_compact() {
        // 80% threshold
        assert!(should_compact(80000, 100000, 0.8));
        assert!(!should_compact(79000, 100000, 0.8));

        // Edge case: zero context
        assert!(!should_compact(1000, 0, 0.8));
    }

    #[test]
    fn test_usage_percent() {
        assert!((usage_percent(50000, 100000) - 50.0).abs() < 0.01);
        assert!((usage_percent(0, 100000)).abs() < 0.01);
        assert!((usage_percent(1000, 0)).abs() < 0.01);
    }
}
