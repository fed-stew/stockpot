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

/// Format a token count with space as thousands separator.
///
/// Examples:
/// - 500 → "500"
/// - 1500 → "1 500"
/// - 128000 → "128 000"
/// - 1500000 → "1 500 000"
pub fn format_tokens_with_separator(count: usize) -> String {
    let s = count.to_string();
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(' ');
        }
        result.push(*ch);
    }
    
    result
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

    #[test]
    fn test_format_tokens_with_separator() {
        // Small numbers - no separator needed
        assert_eq!(format_tokens_with_separator(0), "0");
        assert_eq!(format_tokens_with_separator(1), "1");
        assert_eq!(format_tokens_with_separator(12), "12");
        assert_eq!(format_tokens_with_separator(123), "123");
        assert_eq!(format_tokens_with_separator(500), "500");
        assert_eq!(format_tokens_with_separator(999), "999");
        
        // Thousands
        assert_eq!(format_tokens_with_separator(1000), "1 000");
        assert_eq!(format_tokens_with_separator(1231), "1 231");
        assert_eq!(format_tokens_with_separator(1500), "1 500");
        assert_eq!(format_tokens_with_separator(12345), "12 345");
        assert_eq!(format_tokens_with_separator(128000), "128 000");
        
        // Millions
        assert_eq!(format_tokens_with_separator(200000), "200 000");
        assert_eq!(format_tokens_with_separator(1000000), "1 000 000");
        assert_eq!(format_tokens_with_separator(1500000), "1 500 000");
        assert_eq!(format_tokens_with_separator(12345678), "12 345 678");
    }
}
