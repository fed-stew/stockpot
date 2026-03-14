//! Log level and file output settings.

use super::keys;
use super::settings::Settings;

impl<'a> Settings<'a> {
    /// Get the configured log level (default: "warn").
    ///
    /// Also respects the `SPOT_LOG` environment variable, which takes priority.
    pub fn get_log_level(&self) -> String {
        // Environment variable takes priority
        if let Ok(env_level) = std::env::var("SPOT_LOG") {
            return env_level;
        }
        self.get_string(keys::LOG_LEVEL)
            .unwrap_or_else(|| "warn".to_string())
    }

    /// Set the log level in config.
    pub fn set_log_level(&self, level: &str) {
        self.set_string(keys::LOG_LEVEL, level);
    }

    /// Get whether file logging is enabled (default: false).
    ///
    /// When enabled, logs are written to `~/.spot/spot.log`.
    pub fn get_log_file_enabled(&self) -> bool {
        self.get(keys::LOG_FILE_ENABLED)
            .ok()
            .flatten()
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"))
            .unwrap_or(false)
    }

    /// Set whether file logging is enabled.
    pub fn set_log_file_enabled(&self, enabled: bool) {
        self.set_bool(keys::LOG_FILE_ENABLED, enabled);
    }
}
