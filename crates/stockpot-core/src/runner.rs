//! Application Runner Module
//!
//! Provides shared configuration for GUI, TUI, and render test modes.

/// Shared application configuration.
///
/// This struct contains the runtime configuration options that are shared
/// across all binary entry points. Routing flags (like --tui) are handled
/// by the individual binaries.
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    /// Enable debug logging
    pub debug: bool,
    /// Enable verbose (trace-level) logging
    pub verbose: bool,
    /// Skip the automatic update check on startup
    pub skip_update_check: bool,
}
