//! Typed settings key constants.
//!
//! Centralizes all settings key strings to avoid magic strings
//! scattered across the codebase.

// Core settings
pub const MODEL: &str = "model";
pub const YOLO_MODE: &str = "yolo_mode";
pub const ASSISTANT_NAME: &str = "assistant_name";
pub const OWNER_NAME: &str = "owner_name";
pub const USER_MODE: &str = "user_mode";
pub const PDF_MODE: &str = "pdf_mode";
pub const SHOW_REASONING: &str = "show_reasoning";

// Compression settings
pub const COMPRESSION_ENABLED: &str = "compression.enabled";
pub const COMPRESSION_STRATEGY: &str = "compression.strategy";
pub const COMPRESSION_THRESHOLD: &str = "compression.threshold";
pub const COMPRESSION_TARGET_TOKENS: &str = "compression.target_tokens";

// VDI settings
pub const VDI_MODE: &str = "vdi.mode";
pub const VDI_FRAME_INTERVAL_MS: &str = "vdi.frame_interval_ms";

// Update check settings
pub const UPDATE_CHECK_ENABLED: &str = "update_check.enabled";
pub const UPDATE_CHECK_LAST_CHECK: &str = "update_check.last_check";
pub const UPDATE_CHECK_LATEST_VERSION: &str = "update_check.latest_version";
pub const UPDATE_CHECK_LATEST_TAG: &str = "update_check.latest_tag";
pub const UPDATE_CHECK_LATEST_URL: &str = "update_check.latest_url";

// Logging settings
pub const LOG_LEVEL: &str = "log.level";
pub const LOG_FILE_ENABLED: &str = "log.file_enabled";

// Agent-scoped key prefixes
const AGENT_PIN_PREFIX: &str = "agent_pin.";
const AGENT_MCP_PREFIX: &str = "agent_mcp.";

/// Build the settings key for an agent's pinned model.
pub fn agent_pin_key(agent_name: &str) -> String {
    format!("{}{}", AGENT_PIN_PREFIX, agent_name)
}

/// Build the settings key for an agent's MCP attachments.
pub fn agent_mcp_key(agent_name: &str) -> String {
    format!("{}{}", AGENT_MCP_PREFIX, agent_name)
}

/// Get the prefix used for agent pin keys (for LIKE queries).
pub fn agent_pin_prefix() -> &'static str {
    AGENT_PIN_PREFIX
}

/// Get the prefix used for agent MCP keys (for LIKE queries).
pub fn agent_mcp_prefix() -> &'static str {
    AGENT_MCP_PREFIX
}
