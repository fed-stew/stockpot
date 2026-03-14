//! Layered typed configuration.
//!
//! Provides a strongly-typed `SpotConfig` struct that supports layered loading:
//!
//! 1. Built-in defaults (`SpotConfig::default()`)
//! 2. User-level TOML file (`~/.spot/config.toml`)
//! 3. Project-level TOML file (`.spot/config.toml` in the current directory)
//! 4. Environment variables (e.g., `SPOT_USER_MODE=expert`)
//! 5. SQLite settings (backward compatibility)
//!
//! Later layers override earlier ones, but only for fields that are explicitly set.
//! The existing SQLite-based `Settings` struct continues to work unchanged.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::settings::{PdfMode, Settings};
use crate::agents::UserMode;

// ─────────────────────────────────────────────────────────────────────────────
// Core config structs
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level application configuration.
///
/// All fields use `Option` in the overlay representation so that
/// partial TOML files only override the fields they mention. The
/// public API exposes non-optional accessors that fall back to defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpotConfig {
    /// User experience level (normal / expert / developer).
    pub user_mode: UserMode,

    /// Auto-accept shell commands without confirmation.
    pub yolo_mode: bool,

    /// PDF processing mode (image / text).
    pub pdf_mode: PdfMode,

    /// Show agent reasoning in the UI.
    pub show_reasoning: bool,

    /// Default model name.
    pub model: String,

    /// Assistant display name.
    pub assistant_name: String,

    /// Owner display name.
    pub owner_name: String,

    /// Whether automatic update checks are enabled.
    pub update_check_enabled: bool,

    /// Context compression settings.
    pub compression: CompressionConfig,

    /// VDI (Virtual Desktop Infrastructure) settings.
    pub vdi: VdiConfig,
}

/// Context-compression configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompressionConfig {
    /// Enable context compression.
    pub enabled: bool,

    /// Strategy: "truncate" or "summarize".
    pub strategy: String,

    /// Usage-ratio threshold (0.0 -- 1.0) at which compression kicks in.
    pub threshold: f64,

    /// Target token count after compression.
    pub target_tokens: usize,
}

/// VDI-mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VdiConfig {
    /// VDI mode override. `None` means auto-detect.
    pub mode: Option<bool>,

    /// Animation frame interval in milliseconds.
    pub frame_interval_ms: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Serde helpers for PdfMode
// ─────────────────────────────────────────────────────────────────────────────

impl Serialize for PdfMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PdfMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(s.parse::<PdfMode>().unwrap_or_default())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Defaults
// ─────────────────────────────────────────────────────────────────────────────

impl Default for SpotConfig {
    fn default() -> Self {
        Self {
            user_mode: UserMode::Normal,
            yolo_mode: false,
            pdf_mode: PdfMode::Image,
            show_reasoning: false,
            model: "gpt-4o".to_string(),
            assistant_name: "Spot".to_string(),
            owner_name: "Master".to_string(),
            update_check_enabled: true,
            compression: CompressionConfig::default(),
            vdi: VdiConfig::default(),
        }
    }
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: "truncate".to_string(),
            threshold: 0.75,
            target_tokens: 30000,
        }
    }
}

impl Default for VdiConfig {
    fn default() -> Self {
        Self {
            mode: None,
            frame_interval_ms: 66,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay (partial config for layered merging)
// ─────────────────────────────────────────────────────────────────────────────

/// A partial representation of `SpotConfig` where every field is optional.
///
/// Used for layered merging: later layers override only the fields they set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct SpotConfigOverlay {
    pub user_mode: Option<UserMode>,
    pub yolo_mode: Option<bool>,
    pub pdf_mode: Option<PdfMode>,
    pub show_reasoning: Option<bool>,
    pub model: Option<String>,
    pub assistant_name: Option<String>,
    pub owner_name: Option<String>,
    pub update_check_enabled: Option<bool>,
    pub compression: Option<CompressionOverlay>,
    pub vdi: Option<VdiOverlay>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CompressionOverlay {
    pub enabled: Option<bool>,
    pub strategy: Option<String>,
    pub threshold: Option<f64>,
    pub target_tokens: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct VdiOverlay {
    pub mode: Option<bool>,
    pub frame_interval_ms: Option<u64>,
}

impl SpotConfigOverlay {
    /// Apply this overlay on top of an existing config, returning a new config.
    fn apply_to(self, mut base: SpotConfig) -> SpotConfig {
        if let Some(v) = self.user_mode {
            base.user_mode = v;
        }
        if let Some(v) = self.yolo_mode {
            base.yolo_mode = v;
        }
        if let Some(v) = self.pdf_mode {
            base.pdf_mode = v;
        }
        if let Some(v) = self.show_reasoning {
            base.show_reasoning = v;
        }
        if let Some(v) = self.model {
            base.model = v;
        }
        if let Some(v) = self.assistant_name {
            base.assistant_name = v;
        }
        if let Some(v) = self.owner_name {
            base.owner_name = v;
        }
        if let Some(v) = self.update_check_enabled {
            base.update_check_enabled = v;
        }
        if let Some(overlay) = self.compression {
            if let Some(v) = overlay.enabled {
                base.compression.enabled = v;
            }
            if let Some(v) = overlay.strategy {
                base.compression.strategy = v;
            }
            if let Some(v) = overlay.threshold {
                base.compression.threshold = v;
            }
            if let Some(v) = overlay.target_tokens {
                base.compression.target_tokens = v;
            }
        }
        if let Some(overlay) = self.vdi {
            if let Some(v) = overlay.mode {
                base.vdi.mode = Some(v);
            }
            if let Some(v) = overlay.frame_interval_ms {
                base.vdi.frame_interval_ms = v;
            }
        }
        base
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation
// ─────────────────────────────────────────────────────────────────────────────

/// Validation errors for `SpotConfig`.
#[derive(Debug, Clone)]
pub struct ConfigValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl SpotConfig {
    /// Validate the configuration. Returns a list of problems (empty = valid).
    pub fn validate(&self) -> Vec<ConfigValidationError> {
        let mut errors = Vec::new();

        if self.model.trim().is_empty() {
            errors.push(ConfigValidationError {
                field: "model".into(),
                message: "model name must not be empty".into(),
            });
        }

        if self.assistant_name.trim().is_empty() {
            errors.push(ConfigValidationError {
                field: "assistant_name".into(),
                message: "assistant name must not be empty".into(),
            });
        }

        if self.owner_name.trim().is_empty() {
            errors.push(ConfigValidationError {
                field: "owner_name".into(),
                message: "owner name must not be empty".into(),
            });
        }

        if !(0.0..=1.0).contains(&self.compression.threshold) {
            errors.push(ConfigValidationError {
                field: "compression.threshold".into(),
                message: format!(
                    "must be between 0.0 and 1.0, got {}",
                    self.compression.threshold
                ),
            });
        }

        let valid_strategies = ["truncate", "summarize"];
        if !valid_strategies.contains(&self.compression.strategy.as_str()) {
            errors.push(ConfigValidationError {
                field: "compression.strategy".into(),
                message: format!(
                    "must be one of {:?}, got {:?}",
                    valid_strategies, self.compression.strategy
                ),
            });
        }

        if self.compression.target_tokens == 0 {
            errors.push(ConfigValidationError {
                field: "compression.target_tokens".into(),
                message: "must be greater than 0".into(),
            });
        }

        if !(16..=500).contains(&self.vdi.frame_interval_ms) {
            errors.push(ConfigValidationError {
                field: "vdi.frame_interval_ms".into(),
                message: format!(
                    "must be between 16 and 500 ms, got {}",
                    self.vdi.frame_interval_ms
                ),
            });
        }

        errors
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading layers
// ─────────────────────────────────────────────────────────────────────────────

/// Return the path to the user-level config file (`~/.spot/config.toml`).
pub fn user_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".spot").join("config.toml"))
}

/// Return the path to the project-level config file (`.spot/config.toml`
/// relative to the current working directory).
pub fn project_config_path() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(".spot").join("config.toml"))
}

/// Parse a TOML file into an overlay. Returns `None` if the file does not
/// exist; returns an error if the file exists but cannot be parsed.
fn load_overlay_from_file(path: &Path) -> Result<Option<SpotConfigOverlay>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let overlay: SpotConfigOverlay = toml::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {}", path.display(), e))?;
    Ok(Some(overlay))
}

/// Build an overlay from environment variables.
///
/// Supported variables:
/// - `SPOT_USER_MODE` (normal / expert / developer)
/// - `SPOT_YOLO_MODE` (true / false)
/// - `SPOT_PDF_MODE` (image / text)
/// - `SPOT_SHOW_REASONING` (true / false)
/// - `SPOT_MODEL`
/// - `SPOT_ASSISTANT_NAME`
/// - `SPOT_OWNER_NAME`
/// - `SPOT_UPDATE_CHECK_ENABLED` (true / false)
/// - `SPOT_COMPRESSION_ENABLED` (true / false)
/// - `SPOT_COMPRESSION_STRATEGY` (truncate / summarize)
/// - `SPOT_COMPRESSION_THRESHOLD` (0.0 -- 1.0)
/// - `SPOT_COMPRESSION_TARGET_TOKENS` (integer)
/// - `SPOT_VDI_MODE` (true / false)
/// - `SPOT_VDI_FRAME_INTERVAL_MS` (integer)
fn load_overlay_from_env() -> SpotConfigOverlay {
    let mut overlay = SpotConfigOverlay::default();

    if let Ok(v) = std::env::var("SPOT_USER_MODE") {
        if let Ok(m) = v.parse::<UserMode>() {
            overlay.user_mode = Some(m);
        }
    }
    if let Ok(v) = std::env::var("SPOT_YOLO_MODE") {
        overlay.yolo_mode = Some(parse_bool(&v));
    }
    if let Ok(v) = std::env::var("SPOT_PDF_MODE") {
        overlay.pdf_mode = Some(v.parse::<PdfMode>().unwrap_or_default());
    }
    if let Ok(v) = std::env::var("SPOT_SHOW_REASONING") {
        overlay.show_reasoning = Some(parse_bool(&v));
    }
    if let Ok(v) = std::env::var("SPOT_MODEL") {
        overlay.model = Some(v);
    }
    if let Ok(v) = std::env::var("SPOT_ASSISTANT_NAME") {
        overlay.assistant_name = Some(v);
    }
    if let Ok(v) = std::env::var("SPOT_OWNER_NAME") {
        overlay.owner_name = Some(v);
    }
    if let Ok(v) = std::env::var("SPOT_UPDATE_CHECK_ENABLED") {
        overlay.update_check_enabled = Some(parse_bool(&v));
    }

    // Compression env vars
    let mut comp = CompressionOverlay::default();
    let mut has_comp = false;
    if let Ok(v) = std::env::var("SPOT_COMPRESSION_ENABLED") {
        comp.enabled = Some(parse_bool(&v));
        has_comp = true;
    }
    if let Ok(v) = std::env::var("SPOT_COMPRESSION_STRATEGY") {
        comp.strategy = Some(v);
        has_comp = true;
    }
    if let Ok(v) = std::env::var("SPOT_COMPRESSION_THRESHOLD") {
        if let Ok(f) = v.parse::<f64>() {
            comp.threshold = Some(f);
            has_comp = true;
        }
    }
    if let Ok(v) = std::env::var("SPOT_COMPRESSION_TARGET_TOKENS") {
        if let Ok(n) = v.parse::<usize>() {
            comp.target_tokens = Some(n);
            has_comp = true;
        }
    }
    if has_comp {
        overlay.compression = Some(comp);
    }

    // VDI env vars
    let mut vdi = VdiOverlay::default();
    let mut has_vdi = false;
    if let Ok(v) = std::env::var("SPOT_VDI_MODE") {
        vdi.mode = Some(parse_bool(&v));
        has_vdi = true;
    }
    if let Ok(v) = std::env::var("SPOT_VDI_FRAME_INTERVAL_MS") {
        if let Ok(n) = v.parse::<u64>() {
            vdi.frame_interval_ms = Some(n);
            has_vdi = true;
        }
    }
    if has_vdi {
        overlay.vdi = Some(vdi);
    }

    overlay
}

/// Build an overlay from existing SQLite settings (backward compatibility).
fn load_overlay_from_sqlite(settings: &Settings<'_>) -> SpotConfigOverlay {
    let mut overlay = SpotConfigOverlay::default();

    // Only override fields that actually have values stored in SQLite.
    if let Some(v) = settings.get_string(super::keys::USER_MODE) {
        if let Ok(m) = v.parse::<UserMode>() {
            overlay.user_mode = Some(m);
        }
    }
    if let Ok(Some(v)) = settings.get(super::keys::YOLO_MODE) {
        overlay.yolo_mode = Some(parse_bool(&v));
    }
    if let Some(v) = settings.get_string(super::keys::PDF_MODE) {
        overlay.pdf_mode = Some(v.parse::<PdfMode>().unwrap_or_default());
    }
    if let Some(v) = settings.get_string(super::keys::SHOW_REASONING) {
        overlay.show_reasoning = Some(parse_bool(&v));
    }
    if let Some(v) = settings.get_string(super::keys::MODEL) {
        overlay.model = Some(v);
    }
    if let Some(v) = settings.get_string(super::keys::ASSISTANT_NAME) {
        overlay.assistant_name = Some(v);
    }
    if let Some(v) = settings.get_string(super::keys::OWNER_NAME) {
        overlay.owner_name = Some(v);
    }
    if let Ok(Some(v)) = settings.get("update_check.enabled") {
        overlay.update_check_enabled = Some(parse_bool(&v));
    }

    // Compression
    let mut comp = CompressionOverlay::default();
    let mut has_comp = false;
    if let Ok(Some(v)) = settings.get(super::keys::COMPRESSION_ENABLED) {
        comp.enabled = Some(parse_bool(&v));
        has_comp = true;
    }
    if let Some(v) = settings.get_string(super::keys::COMPRESSION_STRATEGY) {
        comp.strategy = Some(v);
        has_comp = true;
    }
    if let Some(v) = settings.get_float(super::keys::COMPRESSION_THRESHOLD) {
        comp.threshold = Some(v);
        has_comp = true;
    }
    if let Some(v) = settings.get_int(super::keys::COMPRESSION_TARGET_TOKENS) {
        comp.target_tokens = Some(v as usize);
        has_comp = true;
    }
    if has_comp {
        overlay.compression = Some(comp);
    }

    // VDI
    let mut vdi = VdiOverlay::default();
    let mut has_vdi = false;
    if let Ok(Some(v)) = settings.get(super::keys::VDI_MODE) {
        match v.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => {
                vdi.mode = Some(true);
                has_vdi = true;
            }
            "false" | "0" | "no" | "off" => {
                vdi.mode = Some(false);
                has_vdi = true;
            }
            _ => {} // "auto" or unrecognized -> don't override
        }
    }
    if let Some(v) = settings.get_int(super::keys::VDI_FRAME_INTERVAL_MS) {
        vdi.frame_interval_ms = Some(v.clamp(16, 500) as u64);
        has_vdi = true;
    }
    if has_vdi {
        overlay.vdi = Some(vdi);
    }

    overlay
}

/// Parse a string as a boolean (matches the existing `Settings::get_bool` logic).
fn parse_bool(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

impl SpotConfig {
    /// Load configuration by merging all layers:
    ///
    /// 1. `SpotConfig::default()`
    /// 2. `~/.spot/config.toml`
    /// 3. `.spot/config.toml` (project-level)
    /// 4. Environment variables (`SPOT_*`)
    /// 5. SQLite settings (backward compat)
    ///
    /// Errors from TOML file parsing are logged but do not prevent loading;
    /// the remaining layers are still applied.
    pub fn load(settings: &Settings<'_>) -> Self {
        let mut config = Self::default();

        // Layer 2: user-level TOML
        if let Some(path) = user_config_path() {
            match load_overlay_from_file(&path) {
                Ok(Some(overlay)) => {
                    tracing::debug!("Loaded user config from {}", path.display());
                    config = overlay.apply_to(config);
                }
                Ok(None) => {} // file doesn't exist, skip
                Err(e) => tracing::warn!("Skipping user config: {}", e),
            }
        }

        // Layer 3: project-level TOML
        if let Some(path) = project_config_path() {
            match load_overlay_from_file(&path) {
                Ok(Some(overlay)) => {
                    tracing::debug!("Loaded project config from {}", path.display());
                    config = overlay.apply_to(config);
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("Skipping project config: {}", e),
            }
        }

        // Layer 4: environment variables
        let env_overlay = load_overlay_from_env();
        config = env_overlay.apply_to(config);

        // Layer 5: SQLite settings (highest priority for backward compat)
        let sqlite_overlay = load_overlay_from_sqlite(settings);
        config = sqlite_overlay.apply_to(config);

        // Log validation warnings (don't fail, just warn)
        let errors = config.validate();
        for e in &errors {
            tracing::warn!("Config validation: {}", e);
        }

        config
    }

    /// Load configuration from file layers and environment only
    /// (without SQLite). Useful during early startup before the
    /// database is available.
    pub fn load_without_db() -> Self {
        let mut config = Self::default();

        if let Some(path) = user_config_path() {
            match load_overlay_from_file(&path) {
                Ok(Some(overlay)) => config = overlay.apply_to(config),
                Ok(None) => {}
                Err(e) => tracing::warn!("Skipping user config: {}", e),
            }
        }

        if let Some(path) = project_config_path() {
            match load_overlay_from_file(&path) {
                Ok(Some(overlay)) => config = overlay.apply_to(config),
                Ok(None) => {}
                Err(e) => tracing::warn!("Skipping project config: {}", e),
            }
        }

        let env_overlay = load_overlay_from_env();
        config = env_overlay.apply_to(config);

        config
    }

    /// Load configuration from a specific TOML file path
    /// (useful for testing or custom config locations).
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let mut config = Self::default();
        if let Some(overlay) = load_overlay_from_file(path)? {
            config = overlay.apply_to(config);
        }
        Ok(config)
    }

    /// Serialize the current config to a TOML string.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| format!("failed to serialize config: {}", e))
    }

    /// Export the current configuration to a TOML file.
    /// Creates parent directories if needed.
    pub fn export_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory {}: {}", parent.display(), e))?;
        }
        let content = self.to_toml()?;
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write {}: {}", path.display(), e))
    }

    /// Generate a config.toml from current SQLite settings if no user
    /// config file exists yet. Returns the path written, or `None` if
    /// a config file already exists.
    pub fn migrate_from_sqlite(settings: &Settings<'_>) -> Result<Option<PathBuf>, String> {
        let path = user_config_path().ok_or("could not determine home directory")?;
        if path.is_file() {
            return Ok(None); // already exists, don't overwrite
        }

        let config = Self::load(settings);
        config.export_to_file(&path)?;
        Ok(Some(path))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, db)
    }

    // =====================================================================
    // Default values
    // =====================================================================

    #[test]
    fn test_default_config() {
        let cfg = SpotConfig::default();
        assert_eq!(cfg.user_mode, UserMode::Normal);
        assert!(!cfg.yolo_mode);
        assert_eq!(cfg.pdf_mode, PdfMode::Image);
        assert!(!cfg.show_reasoning);
        assert_eq!(cfg.model, "gpt-4o");
        assert_eq!(cfg.assistant_name, "Spot");
        assert_eq!(cfg.owner_name, "Master");
        assert!(cfg.update_check_enabled);
        assert!(cfg.compression.enabled);
        assert_eq!(cfg.compression.strategy, "truncate");
        assert!((cfg.compression.threshold - 0.75).abs() < f64::EPSILON);
        assert_eq!(cfg.compression.target_tokens, 30000);
        assert!(cfg.vdi.mode.is_none());
        assert_eq!(cfg.vdi.frame_interval_ms, 66);
    }

    // =====================================================================
    // TOML round-trip
    // =====================================================================

    #[test]
    fn test_toml_roundtrip() {
        let cfg = SpotConfig::default();
        let toml_str = cfg.to_toml().unwrap();
        let parsed = SpotConfig::load_from_file(Path::new("/dev/null/nonexistent")).unwrap();
        // load_from_file on a nonexistent file returns defaults
        assert_eq!(parsed.model, cfg.model);

        // Now test actual round-trip through a temp file
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        cfg.export_to_file(&path).unwrap();
        let loaded = SpotConfig::load_from_file(&path).unwrap();
        assert_eq!(loaded.model, "gpt-4o");
        assert_eq!(loaded.user_mode, UserMode::Normal);
        assert!(!loaded.yolo_mode);

        // Verify the serialized TOML is parseable
        assert!(!toml_str.is_empty());
    }

    #[test]
    fn test_partial_toml_only_overrides_specified_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
model = "claude-3-opus"
yolo_mode = true
"#,
        )
        .unwrap();

        let loaded = SpotConfig::load_from_file(&path).unwrap();
        // Overridden fields
        assert_eq!(loaded.model, "claude-3-opus");
        assert!(loaded.yolo_mode);
        // Fields not in the TOML should keep defaults
        assert_eq!(loaded.user_mode, UserMode::Normal);
        assert_eq!(loaded.pdf_mode, PdfMode::Image);
        assert_eq!(loaded.assistant_name, "Spot");
        assert!(loaded.compression.enabled);
    }

    #[test]
    fn test_nested_toml_override() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[compression]
threshold = 0.5
target_tokens = 50000

[vdi]
mode = true
frame_interval_ms = 100
"#,
        )
        .unwrap();

        let loaded = SpotConfig::load_from_file(&path).unwrap();
        assert!((loaded.compression.threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(loaded.compression.target_tokens, 50000);
        // Unspecified compression fields keep defaults
        assert!(loaded.compression.enabled);
        assert_eq!(loaded.compression.strategy, "truncate");
        // VDI overrides
        assert_eq!(loaded.vdi.mode, Some(true));
        assert_eq!(loaded.vdi.frame_interval_ms, 100);
    }

    // =====================================================================
    // Validation
    // =====================================================================

    #[test]
    fn test_default_validates_ok() {
        let cfg = SpotConfig::default();
        assert!(cfg.validate().is_empty());
    }

    #[test]
    fn test_validation_empty_model() {
        let cfg = SpotConfig {
            model: "".into(),
            ..Default::default()
        };
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "model"));
    }

    #[test]
    fn test_validation_bad_threshold() {
        let mut cfg = SpotConfig::default();
        cfg.compression.threshold = 1.5;
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "compression.threshold"));
    }

    #[test]
    fn test_validation_bad_strategy() {
        let mut cfg = SpotConfig::default();
        cfg.compression.strategy = "unknown".into();
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "compression.strategy"));
    }

    #[test]
    fn test_validation_zero_target_tokens() {
        let mut cfg = SpotConfig::default();
        cfg.compression.target_tokens = 0;
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "compression.target_tokens"));
    }

    #[test]
    fn test_validation_bad_frame_interval() {
        let mut cfg = SpotConfig::default();
        cfg.vdi.frame_interval_ms = 5; // too low
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "vdi.frame_interval_ms"));

        cfg.vdi.frame_interval_ms = 1000; // too high
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "vdi.frame_interval_ms"));
    }

    #[test]
    fn test_validation_empty_names() {
        let cfg = SpotConfig {
            assistant_name: " ".into(),
            owner_name: "".into(),
            ..Default::default()
        };
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "assistant_name"));
        assert!(errors.iter().any(|e| e.field == "owner_name"));
    }

    #[test]
    fn test_validation_multiple_errors() {
        let cfg = SpotConfig {
            model: "".into(),
            compression: CompressionConfig {
                threshold: -1.0,
                ..Default::default()
            },
            vdi: VdiConfig {
                frame_interval_ms: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let errors = cfg.validate();
        assert!(errors.len() >= 3);
    }

    // =====================================================================
    // SQLite overlay
    // =====================================================================

    #[test]
    fn test_sqlite_overlay_applies() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("model", "claude-3-opus").unwrap();
        settings.set("yolo_mode", "true").unwrap();
        settings.set("user_mode", "expert").unwrap();
        settings.set("compression.threshold", "0.9").unwrap();

        let overlay = load_overlay_from_sqlite(&settings);
        let config = overlay.apply_to(SpotConfig::default());

        assert_eq!(config.model, "claude-3-opus");
        assert!(config.yolo_mode);
        assert_eq!(config.user_mode, UserMode::Expert);
        assert!((config.compression.threshold - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sqlite_overlay_empty_db_returns_defaults() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        let overlay = load_overlay_from_sqlite(&settings);
        let config = overlay.apply_to(SpotConfig::default());

        // Everything should be defaults
        assert_eq!(config.model, "gpt-4o");
        assert!(!config.yolo_mode);
        assert_eq!(config.user_mode, UserMode::Normal);
    }

    #[test]
    fn test_load_with_sqlite() {
        let (_temp, db) = setup_test_db();
        let settings = Settings::new(&db);

        settings.set("model", "gpt-4-turbo").unwrap();
        settings.set("owner_name", "Alice").unwrap();

        let config = SpotConfig::load(&settings);
        assert_eq!(config.model, "gpt-4-turbo");
        assert_eq!(config.owner_name, "Alice");
        // Untouched fields keep defaults
        assert_eq!(config.assistant_name, "Spot");
    }

    // =====================================================================
    // Environment variable overlay
    // =====================================================================

    #[test]
    fn test_env_overlay() {
        // Set env vars for this test, then clean up
        std::env::set_var("SPOT_MODEL", "env-model");
        std::env::set_var("SPOT_YOLO_MODE", "true");
        std::env::set_var("SPOT_COMPRESSION_THRESHOLD", "0.6");

        let overlay = load_overlay_from_env();

        // Clean up before asserting (in case of panic)
        std::env::remove_var("SPOT_MODEL");
        std::env::remove_var("SPOT_YOLO_MODE");
        std::env::remove_var("SPOT_COMPRESSION_THRESHOLD");

        assert_eq!(overlay.model, Some("env-model".into()));
        assert_eq!(overlay.yolo_mode, Some(true));
        let comp = overlay.compression.unwrap();
        assert!((comp.threshold.unwrap() - 0.6).abs() < f64::EPSILON);
    }

    // =====================================================================
    // Layered override order
    // =====================================================================

    #[test]
    fn test_overlay_apply_order() {
        let base = SpotConfig::default();
        assert_eq!(base.model, "gpt-4o");

        // First overlay sets model
        let overlay1 = SpotConfigOverlay {
            model: Some("layer1-model".into()),
            ..Default::default()
        };
        let config = overlay1.apply_to(base);
        assert_eq!(config.model, "layer1-model");

        // Second overlay overrides model again
        let overlay2 = SpotConfigOverlay {
            model: Some("layer2-model".into()),
            ..Default::default()
        };
        let config = overlay2.apply_to(config);
        assert_eq!(config.model, "layer2-model");

        // An overlay with model=None does NOT override
        let overlay3 = SpotConfigOverlay {
            yolo_mode: Some(true),
            ..Default::default()
        };
        let config = overlay3.apply_to(config);
        assert_eq!(config.model, "layer2-model"); // unchanged
        assert!(config.yolo_mode); // new field applied
    }

    // =====================================================================
    // Export / migration
    // =====================================================================

    #[test]
    fn test_export_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("config.toml");
        let cfg = SpotConfig::default();
        cfg.export_to_file(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn test_toml_contains_expected_keys() {
        let cfg = SpotConfig::default();
        let toml_str = cfg.to_toml().unwrap();
        assert!(toml_str.contains("model"));
        assert!(toml_str.contains("yolo_mode"));
        assert!(toml_str.contains("user_mode"));
        assert!(toml_str.contains("[compression]"));
        assert!(toml_str.contains("[vdi]"));
    }

    // =====================================================================
    // parse_bool helper
    // =====================================================================

    #[test]
    fn test_parse_bool_true_variants() {
        for v in &["true", "1", "yes", "on", "TRUE", "Yes", "ON"] {
            assert!(parse_bool(v), "expected '{}' to be true", v);
        }
    }

    #[test]
    fn test_parse_bool_false_variants() {
        for v in &["false", "0", "no", "off", "random", ""] {
            assert!(!parse_bool(v), "expected '{}' to be false", v);
        }
    }

    // =====================================================================
    // PdfMode serde
    // =====================================================================

    #[test]
    fn test_pdf_mode_serde_roundtrip() {
        let cfg = SpotConfig {
            pdf_mode: PdfMode::TextExtract,
            ..Default::default()
        };
        let toml_str = cfg.to_toml().unwrap();
        assert!(toml_str.contains("pdf_mode = \"text\""));

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, &toml_str).unwrap();
        let loaded = SpotConfig::load_from_file(&path).unwrap();
        assert_eq!(loaded.pdf_mode, PdfMode::TextExtract);
    }

    // =====================================================================
    // ConfigValidationError display
    // =====================================================================

    #[test]
    fn test_validation_error_display() {
        let err = ConfigValidationError {
            field: "model".into(),
            message: "must not be empty".into(),
        };
        let s = err.to_string();
        assert!(s.contains("model"));
        assert!(s.contains("must not be empty"));
    }
}
