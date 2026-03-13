//! Context compression settings.

use super::keys;
use super::settings::Settings;

impl<'a> Settings<'a> {
    /// Get whether context compression is enabled (default: true).
    pub fn get_compression_enabled(&self) -> bool {
        self.get(keys::COMPRESSION_ENABLED)
            .ok()
            .flatten()
            .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0" | "no" | "off"))
            .unwrap_or(true)
    }

    /// Set whether context compression is enabled.
    pub fn set_compression_enabled(&self, enabled: bool) {
        self.set_bool(keys::COMPRESSION_ENABLED, enabled);
    }

    /// Get compression strategy: "truncate" or "summarize" (default: "truncate").
    pub fn get_compression_strategy(&self) -> String {
        self.get_string(keys::COMPRESSION_STRATEGY)
            .unwrap_or_else(|| "truncate".to_string())
    }

    /// Set compression strategy.
    pub fn set_compression_strategy(&self, strategy: &str) {
        self.set_string(keys::COMPRESSION_STRATEGY, strategy);
    }

    /// Get compression threshold (0.0-1.0, default: 0.75).
    pub fn get_compression_threshold(&self) -> f64 {
        self.get_float(keys::COMPRESSION_THRESHOLD).unwrap_or(0.75)
    }

    /// Set compression threshold.
    pub fn set_compression_threshold(&self, threshold: f64) {
        self.set_float(keys::COMPRESSION_THRESHOLD, threshold);
    }

    /// Get target tokens for compression (default: 30000).
    pub fn get_compression_target_tokens(&self) -> usize {
        self.get_int(keys::COMPRESSION_TARGET_TOKENS)
            .unwrap_or(30000) as usize
    }

    /// Set target tokens for compression.
    pub fn set_compression_target_tokens(&self, tokens: usize) {
        self.set_int(keys::COMPRESSION_TARGET_TOKENS, tokens as i64);
    }
}
