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
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
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

    #[test]
    fn test_estimate_message_tokens_single() {
        // Create a simple ModelRequest with user prompt
        let mut msg = ModelRequest::new();
        msg.add_user_prompt("Hello, world!".to_string());

        let tokens = estimate_message_tokens(&msg);
        // Should be at least 10 (the minimum)
        assert!(tokens >= 10);
        // Serialized JSON will be longer than the raw text
        // so tokens should be reasonable (not absurdly large)
        assert!(tokens < 1000);
    }

    #[test]
    fn test_estimate_message_tokens_minimum() {
        // Even an empty ModelRequest should return at least 10
        let msg = ModelRequest::new();
        let tokens = estimate_message_tokens(&msg);
        assert!(tokens >= 10);
    }

    #[test]
    fn test_estimate_tokens_multiple_messages() {
        let mut msg1 = ModelRequest::new();
        msg1.add_user_prompt("First message".to_string());

        let mut msg2 = ModelRequest::new();
        msg2.add_user_prompt("Second message with more content".to_string());

        let messages = vec![msg1, msg2];
        let total = estimate_tokens(&messages);

        // Total should be sum of individual estimates
        assert!(total >= 20); // At least 10 per message
    }

    #[test]
    fn test_estimate_tokens_single_message() {
        let mut msg = ModelRequest::new();
        msg.add_user_prompt("Test".to_string());

        let messages = vec![msg.clone()];
        let total = estimate_tokens(&messages);
        let single = estimate_message_tokens(&msg);

        assert_eq!(total, single);
    }

    #[test]
    fn test_should_compact_at_threshold() {
        // Exactly at threshold should trigger
        assert!(should_compact(80, 100, 0.8));

        // Just above threshold
        assert!(should_compact(81, 100, 0.8));

        // Just below threshold
        assert!(!should_compact(79, 100, 0.8));
    }

    #[test]
    fn test_should_compact_various_thresholds() {
        // 50% threshold
        assert!(should_compact(50, 100, 0.5));
        assert!(!should_compact(49, 100, 0.5));

        // 90% threshold
        assert!(should_compact(90, 100, 0.9));
        assert!(!should_compact(89, 100, 0.9));

        // 100% threshold
        assert!(should_compact(100, 100, 1.0));
        assert!(!should_compact(99, 100, 1.0));
    }

    #[test]
    fn test_should_compact_large_numbers() {
        // Realistic context sizes
        assert!(should_compact(102400, 128000, 0.8));
        assert!(!should_compact(100000, 128000, 0.8));
    }

    #[test]
    fn test_usage_percent_full_context() {
        assert!((usage_percent(100, 100) - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_usage_percent_various_values() {
        // 25%
        assert!((usage_percent(25, 100) - 25.0).abs() < 0.01);

        // 75%
        assert!((usage_percent(75, 100) - 75.0).abs() < 0.01);

        // Over 100%
        assert!((usage_percent(150, 100) - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_usage_percent_large_numbers() {
        // Realistic context
        let tokens = 64000;
        let context = 128000;
        assert!((usage_percent(tokens, context) - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_message_tokens_long_content() {
        let mut msg = ModelRequest::new();
        // Create a longer message to test token estimation
        let long_content = "a".repeat(1000);
        msg.add_user_prompt(long_content);

        let tokens = estimate_message_tokens(&msg);
        // With ~1000 chars of content, should estimate ~250+ tokens
        assert!(tokens >= 100);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_estimate_message_tokens_very_short_content() {
        // Single char should still hit minimum of 10
        let mut msg = ModelRequest::new();
        msg.add_user_prompt("a".to_string());
        assert!(estimate_message_tokens(&msg) >= 10);
    }

    #[test]
    fn test_estimate_tokens_scales_linearly() {
        // Token count should scale roughly linearly with content size
        let mut small = ModelRequest::new();
        small.add_user_prompt("x".repeat(100));

        let mut large = ModelRequest::new();
        large.add_user_prompt("x".repeat(1000));

        let small_tokens = estimate_message_tokens(&small);
        let large_tokens = estimate_message_tokens(&large);

        // Large should be roughly 10x more (allowing some overhead variance)
        assert!(large_tokens > small_tokens * 5);
        assert!(large_tokens < small_tokens * 20);
    }

    // ==================== Large Text Handling ====================

    #[test]
    fn test_estimate_message_tokens_very_large_content() {
        let mut msg = ModelRequest::new();
        // 100KB of text
        let huge_content = "lorem ipsum ".repeat(10000);
        msg.add_user_prompt(huge_content);

        let tokens = estimate_message_tokens(&msg);
        // Should be substantial but not overflow
        assert!(tokens > 10000);
        assert!(tokens < usize::MAX / 2);
    }

    #[test]
    fn test_estimate_tokens_many_messages() {
        let messages: Vec<ModelRequest> = (0..1000)
            .map(|i| {
                let mut msg = ModelRequest::new();
                msg.add_user_prompt(format!("Message number {}", i));
                msg
            })
            .collect();

        let total = estimate_tokens(&messages);
        // Should aggregate without overflow
        assert!(total >= 10000); // At least 10 per message
    }

    #[test]
    fn test_estimate_message_tokens_megabyte_content() {
        let mut msg = ModelRequest::new();
        // ~1MB of text
        let mb_content = "x".repeat(1_000_000);
        msg.add_user_prompt(mb_content);

        let tokens = estimate_message_tokens(&msg);
        // ~250k tokens for 1MB at 4 chars/token
        assert!(tokens > 200_000);
        assert!(tokens < 500_000);
    }

    // ==================== Special Characters ====================

    #[test]
    fn test_estimate_message_tokens_unicode() {
        let mut msg = ModelRequest::new();
        // Unicode chars are multi-byte, affecting JSON size
        msg.add_user_prompt("Hello 世界! 🎉 Привет мир".to_string());

        let tokens = estimate_message_tokens(&msg);
        // Should still estimate reasonably
        assert!(tokens >= 10);
    }

    #[test]
    fn test_estimate_message_tokens_emoji_heavy() {
        let mut msg = ModelRequest::new();
        // Emojis are typically 4 bytes each
        msg.add_user_prompt("🚀🔥💻🎯🌟".repeat(100));

        let tokens = estimate_message_tokens(&msg);
        // Multi-byte chars will inflate JSON size
        assert!(tokens > 50);
    }

    #[test]
    fn test_estimate_message_tokens_newlines_and_whitespace() {
        let mut msg = ModelRequest::new();
        msg.add_user_prompt("line1\nline2\n\tindented\r\nwindows".to_string());

        let tokens = estimate_message_tokens(&msg);
        assert!(tokens >= 10);
    }

    #[test]
    fn test_estimate_message_tokens_json_special_chars() {
        let mut msg = ModelRequest::new();
        // Characters that need escaping in JSON: quotes, backslashes, etc.
        msg.add_user_prompt(r#"{"key": "value", "path": "C:\\Users\\test"}"#.to_string());

        let tokens = estimate_message_tokens(&msg);
        // JSON escaping will increase size
        assert!(tokens >= 10);
    }

    #[test]
    fn test_estimate_message_tokens_null_bytes() {
        let mut msg = ModelRequest::new();
        // Null bytes in string
        msg.add_user_prompt("before\0after".to_string());

        let tokens = estimate_message_tokens(&msg);
        assert!(tokens >= 10);
    }

    #[test]
    fn test_estimate_message_tokens_control_characters() {
        let mut msg = ModelRequest::new();
        // Various control characters
        msg.add_user_prompt("text\x00\x01\x02\x1b[0mmore".to_string());

        let tokens = estimate_message_tokens(&msg);
        assert!(tokens >= 10);
    }

    // ==================== Boundary Conditions ====================

    #[test]
    fn test_should_compact_zero_threshold() {
        // 0% threshold - always compact (even 0 tokens, since 0 >= 0)
        assert!(should_compact(1, 100, 0.0));
        assert!(should_compact(0, 100, 0.0)); // 0/100 = 0.0 >= 0.0 is true
    }

    #[test]
    fn test_should_compact_over_100_percent() {
        // Tokens exceed context (overflow scenario)
        assert!(should_compact(150, 100, 0.8));
        assert!(should_compact(200000, 100000, 0.8));
    }

    #[test]
    fn test_usage_percent_precision() {
        // Test fractional percentages
        assert!((usage_percent(1, 3) - 33.333333).abs() < 0.001);
        assert!((usage_percent(2, 3) - 66.666666).abs() < 0.001);
    }

    #[test]
    fn test_usage_percent_very_small_ratio() {
        // Very small usage
        assert!((usage_percent(1, 1_000_000) - 0.0001).abs() < 0.00001);
    }

    #[test]
    fn test_usage_percent_overflow_safe() {
        // Large numbers that might cause overflow in naive implementations
        let tokens = usize::MAX / 4;
        let context = usize::MAX / 2;
        let percent = usage_percent(tokens, context);
        // Should be ~50%
        assert!(percent > 45.0 && percent < 55.0);
    }

    #[test]
    fn test_should_compact_max_values() {
        // Test with very large values
        assert!(should_compact(usize::MAX, usize::MAX, 0.5));
        assert!(should_compact(usize::MAX, usize::MAX, 1.0));
    }

    #[test]
    fn test_format_tokens_boundary_at_thousands() {
        // Edge cases around 1000
        assert_eq!(format_tokens_with_separator(999), "999");
        assert_eq!(format_tokens_with_separator(1000), "1 000");
        assert_eq!(format_tokens_with_separator(1001), "1 001");
    }

    #[test]
    fn test_format_tokens_max_usize() {
        // Should not panic on max value
        let result = format_tokens_with_separator(usize::MAX);
        assert!(!result.is_empty());
        // Should contain spaces for the large number
        assert!(result.contains(' '));
    }

    // ==================== Consistency Tests ====================

    #[test]
    fn test_estimate_tokens_deterministic() {
        // Same input should always produce same output
        let mut msg = ModelRequest::new();
        msg.add_user_prompt("Deterministic test content".to_string());

        let tokens1 = estimate_message_tokens(&msg);
        let tokens2 = estimate_message_tokens(&msg);
        let tokens3 = estimate_message_tokens(&msg);

        assert_eq!(tokens1, tokens2);
        assert_eq!(tokens2, tokens3);
    }

    #[test]
    fn test_estimate_tokens_order_independent_sum() {
        let mut msg1 = ModelRequest::new();
        msg1.add_user_prompt("First".to_string());

        let mut msg2 = ModelRequest::new();
        msg2.add_user_prompt("Second".to_string());

        let forward = estimate_tokens(&[msg1.clone(), msg2.clone()]);
        let backward = estimate_tokens(&[msg2, msg1]);

        assert_eq!(forward, backward);
    }

    // ==================== Token Minimum Floor Tests ====================

    #[test]
    fn test_minimum_token_floor_enforced() {
        // Even the smallest possible serialization should hit the floor
        let msg = ModelRequest::new();
        let tokens = estimate_message_tokens(&msg);
        // Code uses .max(10) so minimum should be 10
        assert!(tokens >= 10);
    }

    #[test]
    fn test_format_tokens_single_digit() {
        for i in 0..=9 {
            let result = format_tokens_with_separator(i);
            assert_eq!(result, i.to_string());
            assert!(!result.contains(' '));
        }
    }

    #[test]
    fn test_format_tokens_two_digits() {
        for i in [10, 50, 99] {
            let result = format_tokens_with_separator(i);
            assert_eq!(result, i.to_string());
            assert!(!result.contains(' '));
        }
    }

    #[test]
    fn test_format_tokens_three_digits() {
        for i in [100, 500, 999] {
            let result = format_tokens_with_separator(i);
            assert_eq!(result, i.to_string());
            assert!(!result.contains(' '));
        }
    }
}
