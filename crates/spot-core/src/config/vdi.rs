//! VDI (Virtual Desktop Infrastructure) detection and settings.

use super::keys;
use super::settings::Settings;

impl<'a> Settings<'a> {
    /// Get whether VDI mode is enabled (default: auto-detect).
    /// When "auto", detects Citrix VDI environment automatically.
    /// When explicitly set to true/false, uses that value.
    pub fn get_vdi_mode(&self) -> Option<bool> {
        match self.get(keys::VDI_MODE).ok().flatten() {
            Some(v) => match v.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None, // "auto" or unset = auto-detect
            },
            None => None, // Not set = auto-detect
        }
    }

    /// Set VDI mode. Pass None for auto-detect, Some(true) to force on, Some(false) to force off.
    pub fn set_vdi_mode(&self, mode: Option<bool>) {
        match mode {
            Some(true) => self.set_string(keys::VDI_MODE, "true"),
            Some(false) => self.set_string(keys::VDI_MODE, "false"),
            None => {
                let _ = self.delete(keys::VDI_MODE);
            }
        }
    }

    /// Get the VDI animation frame interval in milliseconds (default: 66ms = ~15fps).
    pub fn get_vdi_frame_interval_ms(&self) -> u64 {
        self.get_int(keys::VDI_FRAME_INTERVAL_MS)
            .map(|v| v.clamp(16, 500) as u64)
            .unwrap_or(66)
    }

    /// Set the VDI animation frame interval in milliseconds.
    pub fn set_vdi_frame_interval_ms(&self, ms: u64) {
        self.set_int(keys::VDI_FRAME_INTERVAL_MS, ms as i64);
    }
}

/// Detect if running inside a Citrix VDI environment.
///
/// Checks for common Citrix environment indicators:
/// - CITRIX_SESSION_ID env var (Citrix Virtual Apps/Desktops)
/// - SESSIONNAME containing "ICA" (Citrix ICA protocol)
/// - Citrix Receiver/Workspace process indicators
/// - ViewClient_* env vars (VMware Horizon, similar VDI)
pub fn detect_vdi_environment() -> bool {
    // Citrix-specific environment variables
    if std::env::var("CITRIX_SESSION_ID").is_ok() {
        tracing::info!("VDI detected: CITRIX_SESSION_ID present");
        return true;
    }

    // Citrix ICA session
    if let Ok(session) = std::env::var("SESSIONNAME") {
        if session.contains("ICA") || session.contains("Citrix") {
            tracing::info!("VDI detected: SESSIONNAME={}", session);
            return true;
        }
    }

    // VMware Horizon
    if std::env::var("ViewClient_IP_Address").is_ok()
        || std::env::var("ViewClient_Machine_Name").is_ok()
    {
        tracing::info!("VDI detected: VMware Horizon environment variables present");
        return true;
    }

    // Generic Remote Desktop (Windows RDP)
    if let Ok(session) = std::env::var("SESSIONNAME") {
        if session.starts_with("RDP-") {
            tracing::info!("VDI detected: RDP session (SESSIONNAME={})", session);
            return true;
        }
    }

    // Amazon WorkSpaces
    if std::env::var("WORKSPACES_BUNDLE_ID").is_ok() {
        tracing::info!("VDI detected: Amazon WorkSpaces");
        return true;
    }

    false
}

/// Resolve the effective VDI mode: check user setting first, fall back to auto-detect.
pub fn is_vdi_mode_active(settings: &Settings) -> bool {
    match settings.get_vdi_mode() {
        Some(explicit) => explicit,       // User explicitly set it
        None => detect_vdi_environment(), // Auto-detect
    }
}
