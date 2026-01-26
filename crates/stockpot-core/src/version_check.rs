//! Version check functionality for checking GitHub releases.
//!
//! This module provides functionality to:
//! - Fetch the latest release from GitHub
//! - Compare versions using semver
//! - Determine if an update is available

use anyhow::{Context, Result};
use serde::Deserialize;

/// GitHub API endpoint for latest release
const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/fed-stew/stockpot/releases/latest";

/// Current application version (from Cargo.toml)
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Information about a GitHub release
#[derive(Debug, Clone)]
pub struct LatestRelease {
    /// Version string without 'v' prefix (e.g., "0.6.0")
    pub version: String,
    /// Original tag name (e.g., "v0.6.0")
    pub tag_name: String,
    /// URL to the release page
    pub html_url: String,
}

/// GitHub API response structure
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

/// Fetches the latest release information from GitHub.
///
/// Returns `None` if the request fails or the response is invalid.
/// This is designed to fail silently - version checks should not
/// disrupt the user experience.
pub async fn fetch_latest_release() -> Result<LatestRelease> {
    let client = reqwest::Client::new();
    let response = client
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", "stockpot-cli")
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("Failed to fetch releases from GitHub")?;

    if !response.status().is_success() {
        anyhow::bail!("GitHub API returned status {}", response.status());
    }

    let release: GitHubRelease = response
        .json()
        .await
        .context("Failed to parse GitHub release response")?;

    // Strip 'v' prefix from tag name if present
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name)
        .to_string();

    Ok(LatestRelease {
        version,
        tag_name: release.tag_name,
        html_url: release.html_url,
    })
}

/// Compares two version strings and returns true if `latest` is newer than `current`.
///
/// Uses semver parsing for accurate comparison. Returns `false` if either
/// version string is invalid (fail-safe behavior).
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (
        semver::Version::parse(current),
        semver::Version::parse(latest),
    ) {
        (Ok(curr), Ok(lat)) => lat > curr,
        _ => {
            tracing::warn!(
                "Failed to parse versions for comparison: current={}, latest={}",
                current,
                latest
            );
            false
        }
    }
}

/// Checks if an update is available and returns the release info if so.
///
/// This function:
/// 1. Fetches the latest release from GitHub
/// 2. Compares it to the current version
/// 3. Returns the release info only if a newer version exists
///
/// Returns `None` if:
/// - The fetch fails (network error, API error, etc.)
/// - The current version is up to date
pub async fn check_for_update() -> Option<LatestRelease> {
    match fetch_latest_release().await {
        Ok(release) => {
            if is_newer_version(CURRENT_VERSION, &release.version) {
                tracing::debug!(
                    "Update available: {} -> {}",
                    CURRENT_VERSION,
                    release.version
                );
                Some(release)
            } else {
                tracing::debug!(
                    "No update available: current={}, latest={}",
                    CURRENT_VERSION,
                    release.version
                );
                None
            }
        }
        Err(e) => {
            tracing::debug!("Failed to check for updates: {}", e);
            None
        }
    }
}

/// Print update notification to terminal if a newer version is available.
///
/// This is a convenience function for CLI usage that handles the async
/// check and prints a formatted message.
pub fn print_update_notification_blocking() {
    // Run the async check in a blocking context
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're already in a tokio runtime, spawn a blocking task
            std::thread::spawn(move || {
                handle.block_on(async {
                    if let Some(release) = check_for_update().await {
                        print_update_message(&release);
                    }
                })
            });
            return;
        }
        Err(_) => {
            // No runtime, create a new one
            match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            }
        }
    };

    rt.block_on(async {
        if let Some(release) = check_for_update().await {
            print_update_message(&release);
        }
    });
}

/// Print the update message to stderr
pub fn print_update_message(release: &LatestRelease) {
    use nu_ansi_term::{Color, Style};

    let yellow = Style::new().fg(Color::Yellow).bold();
    let cyan = Style::new().fg(Color::Cyan);
    let dim = Style::new().dimmed();

    eprintln!();
    eprintln!(
        "{} A new version of stockpot is available: {} → {}",
        yellow.paint("⬆"),
        dim.paint(CURRENT_VERSION),
        cyan.paint(&release.version)
    );
    eprintln!("  {}", dim.paint(&release.html_url));
    eprintln!();
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // is_newer_version Tests
    // =========================================================================

    #[test]
    fn test_is_newer_version_patch_bump() {
        assert!(is_newer_version("0.5.0", "0.5.1"));
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("2.3.4", "2.3.5"));
    }

    #[test]
    fn test_is_newer_version_minor_bump() {
        assert!(is_newer_version("0.5.0", "0.6.0"));
        assert!(is_newer_version("1.0.0", "1.1.0"));
        assert!(is_newer_version("2.3.4", "2.4.0"));
    }

    #[test]
    fn test_is_newer_version_major_bump() {
        assert!(is_newer_version("0.5.0", "1.0.0"));
        assert!(is_newer_version("1.9.9", "2.0.0"));
        assert!(is_newer_version("0.99.99", "1.0.0"));
    }

    #[test]
    fn test_is_newer_version_same_version() {
        assert!(!is_newer_version("0.5.0", "0.5.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("2.3.4", "2.3.4"));
    }

    #[test]
    fn test_is_newer_version_older_version() {
        assert!(!is_newer_version("0.6.0", "0.5.0"));
        assert!(!is_newer_version("1.0.0", "0.99.99"));
        assert!(!is_newer_version("2.0.0", "1.9.9"));
    }

    #[test]
    fn test_is_newer_version_complex_comparison() {
        // Major > minor > patch
        assert!(is_newer_version("0.9.9", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));

        // Same major, different minor
        assert!(is_newer_version("1.5.9", "1.6.0"));
        assert!(!is_newer_version("1.6.0", "1.5.9"));
    }

    #[test]
    fn test_is_newer_version_invalid_current() {
        assert!(!is_newer_version("invalid", "0.6.0"));
        assert!(!is_newer_version("not.a.version", "1.0.0"));
        assert!(!is_newer_version("", "0.6.0"));
    }

    #[test]
    fn test_is_newer_version_invalid_latest() {
        assert!(!is_newer_version("0.5.0", "invalid"));
        assert!(!is_newer_version("1.0.0", "not.a.version"));
        assert!(!is_newer_version("0.5.0", ""));
    }

    #[test]
    fn test_is_newer_version_both_invalid() {
        assert!(!is_newer_version("invalid", "also-invalid"));
        assert!(!is_newer_version("", ""));
        assert!(!is_newer_version("x.y.z", "a.b.c"));
    }

    #[test]
    fn test_is_newer_version_prerelease() {
        // Pre-release versions are less than release versions
        assert!(is_newer_version("1.0.0-alpha", "1.0.0"));
        assert!(is_newer_version("1.0.0-beta", "1.0.0"));
        assert!(is_newer_version("1.0.0-rc.1", "1.0.0"));

        // Pre-release ordering
        assert!(is_newer_version("1.0.0-alpha", "1.0.0-beta"));
        assert!(is_newer_version("1.0.0-alpha.1", "1.0.0-alpha.2"));
    }

    // =========================================================================
    // CURRENT_VERSION Tests
    // =========================================================================

    #[test]
    fn test_current_version_is_valid() {
        let version = semver::Version::parse(CURRENT_VERSION);
        assert!(
            version.is_ok(),
            "CURRENT_VERSION '{}' should be valid semver",
            CURRENT_VERSION
        );
    }

    #[test]
    fn test_current_version_not_empty() {
        assert!(!CURRENT_VERSION.is_empty());
    }

    #[test]
    fn test_current_version_format() {
        // Should be in format X.Y.Z
        let parts: Vec<&str> = CURRENT_VERSION.split('.').collect();
        assert!(
            parts.len() >= 3,
            "Version should have at least 3 parts: {}",
            CURRENT_VERSION
        );

        // First part (major) should be a number
        assert!(
            parts[0].parse::<u32>().is_ok(),
            "Major version should be a number"
        );
    }

    // =========================================================================
    // LatestRelease Tests
    // =========================================================================

    #[test]
    fn test_latest_release_clone() {
        let release = LatestRelease {
            version: "1.0.0".to_string(),
            tag_name: "v1.0.0".to_string(),
            html_url: "https://github.com/test/repo/releases/tag/v1.0.0".to_string(),
        };

        let cloned = release.clone();
        assert_eq!(cloned.version, release.version);
        assert_eq!(cloned.tag_name, release.tag_name);
        assert_eq!(cloned.html_url, release.html_url);
    }

    #[test]
    fn test_latest_release_debug() {
        let release = LatestRelease {
            version: "1.0.0".to_string(),
            tag_name: "v1.0.0".to_string(),
            html_url: "https://example.com".to_string(),
        };

        let debug_str = format!("{:?}", release);
        assert!(debug_str.contains("LatestRelease"));
        assert!(debug_str.contains("1.0.0"));
    }

    // =========================================================================
    // GitHub URL Tests
    // =========================================================================

    #[test]
    fn test_github_releases_url_format() {
        assert!(GITHUB_RELEASES_URL.starts_with("https://"));
        assert!(GITHUB_RELEASES_URL.contains("api.github.com"));
        assert!(GITHUB_RELEASES_URL.contains("releases/latest"));
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_version_with_build_metadata() {
        // Note: The semver crate compares build metadata lexicographically,
        // even though the semver spec says it should be ignored for precedence.
        // This test documents the actual behavior.
        // "build2" > "build1" lexicographically, so this returns true.
        assert!(is_newer_version("1.0.0+build1", "1.0.0+build2"));

        // Different version numbers still work correctly
        assert!(is_newer_version("1.0.0+build1", "1.0.1"));
        assert!(is_newer_version("1.0.0+build1", "1.0.1+build1"));
    }

    #[test]
    fn test_version_large_numbers() {
        assert!(is_newer_version("0.0.1", "999.999.999"));
        assert!(!is_newer_version("999.999.999", "0.0.1"));
    }

    // =========================================================================
    // Version Prefix Stripping (simulating fetch_latest_release logic)
    // =========================================================================

    #[test]
    fn test_version_prefix_stripping() {
        // Simulate the strip_prefix logic from fetch_latest_release
        let tag_with_v = "v1.2.3";
        let tag_without_v = "1.2.3";

        let stripped_with_v = tag_with_v.strip_prefix('v').unwrap_or(tag_with_v);
        let stripped_without_v = tag_without_v.strip_prefix('v').unwrap_or(tag_without_v);

        assert_eq!(stripped_with_v, "1.2.3");
        assert_eq!(stripped_without_v, "1.2.3");
    }

    #[test]
    fn test_version_prefix_edge_cases() {
        // Edge case: tag starts with 'v' but is not a version prefix
        let tag_version = "version-1.0.0";
        let stripped = tag_version.strip_prefix('v').unwrap_or(tag_version);
        // Should strip the 'v' even though it's part of 'version'
        assert_eq!(stripped, "ersion-1.0.0");

        // Edge case: empty string
        let empty = "";
        let stripped_empty = empty.strip_prefix('v').unwrap_or(empty);
        assert_eq!(stripped_empty, "");

        // Edge case: just 'v'
        let just_v = "v";
        let stripped_just_v = just_v.strip_prefix('v').unwrap_or(just_v);
        assert_eq!(stripped_just_v, "");
    }

    // =========================================================================
    // Prerelease Version Ordering Tests
    // =========================================================================

    #[test]
    fn test_prerelease_vs_release_ordering() {
        // A release version is always newer than any prerelease of the same version
        assert!(is_newer_version("1.0.0-alpha", "1.0.0"));
        assert!(is_newer_version("1.0.0-alpha.1", "1.0.0"));
        assert!(is_newer_version("1.0.0-beta", "1.0.0"));
        assert!(is_newer_version("1.0.0-beta.2", "1.0.0"));
        assert!(is_newer_version("1.0.0-rc.1", "1.0.0"));

        // But a prerelease of next version is still newer than current release
        assert!(is_newer_version("1.0.0", "1.0.1-alpha"));
        assert!(is_newer_version("1.0.0", "1.1.0-beta"));
        assert!(is_newer_version("1.0.0", "2.0.0-rc.1"));
    }

    #[test]
    fn test_prerelease_identifier_ordering() {
        // Numeric identifiers are compared as integers
        assert!(is_newer_version("1.0.0-alpha.1", "1.0.0-alpha.2"));
        assert!(is_newer_version("1.0.0-alpha.9", "1.0.0-alpha.10"));
        assert!(is_newer_version("1.0.0-rc.1", "1.0.0-rc.2"));

        // Alphabetic ordering for string identifiers
        assert!(is_newer_version("1.0.0-alpha", "1.0.0-beta"));
        assert!(is_newer_version("1.0.0-beta", "1.0.0-rc"));

        // Mixed identifiers
        assert!(is_newer_version("1.0.0-alpha.1.beta", "1.0.0-alpha.2"));
    }

    // =========================================================================
    // Invalid Version Format Tests
    // =========================================================================

    #[test]
    fn test_invalid_version_formats() {
        // Missing parts
        assert!(!is_newer_version("1", "2"));
        assert!(!is_newer_version("1.0", "2.0"));

        // Non-numeric parts
        assert!(!is_newer_version("a.b.c", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "x.y.z"));

        // Negative numbers (not valid semver)
        assert!(!is_newer_version("-1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.-1.0", "1.0.0"));

        // Spaces
        assert!(!is_newer_version(" 1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0 ", "1.0.0"));
        assert!(!is_newer_version("1. 0.0", "1.0.0"));

        // Special characters
        assert!(!is_newer_version("1.0.0!", "1.0.0"));
        assert!(!is_newer_version("v1.0.0", "1.0.0")); // v prefix not valid semver
    }

    #[test]
    fn test_version_with_leading_zeros() {
        // Leading zeros are not allowed in semver
        assert!(!is_newer_version("01.0.0", "1.0.0"));
        assert!(!is_newer_version("1.00.0", "1.0.0"));
        assert!(!is_newer_version("1.0.00", "1.0.0"));
    }

    // =========================================================================
    // LatestRelease Struct Tests
    // =========================================================================

    #[test]
    fn test_latest_release_field_access() {
        let release = LatestRelease {
            version: "2.5.3".to_string(),
            tag_name: "v2.5.3".to_string(),
            html_url: "https://github.com/fed-stew/stockpot/releases/tag/v2.5.3".to_string(),
        };

        assert_eq!(release.version, "2.5.3");
        assert_eq!(release.tag_name, "v2.5.3");
        assert!(release.html_url.contains("releases/tag"));
    }

    #[test]
    fn test_latest_release_with_various_urls() {
        let release = LatestRelease {
            version: "1.0.0".to_string(),
            tag_name: "v1.0.0".to_string(),
            html_url: "https://github.com/user/repo/releases/tag/v1.0.0".to_string(),
        };

        assert!(release.html_url.starts_with("https://"));
        assert!(release.html_url.contains("github.com"));
    }

    #[test]
    fn test_latest_release_empty_fields() {
        // Edge case: empty fields (shouldn't happen in practice but tests struct flexibility)
        let release = LatestRelease {
            version: "".to_string(),
            tag_name: "".to_string(),
            html_url: "".to_string(),
        };

        assert!(release.version.is_empty());
        assert!(release.tag_name.is_empty());
        assert!(release.html_url.is_empty());
    }

    // =========================================================================
    // Boundary Version Tests
    // =========================================================================

    #[test]
    fn test_zero_versions() {
        assert!(is_newer_version("0.0.0", "0.0.1"));
        assert!(is_newer_version("0.0.0", "0.1.0"));
        assert!(is_newer_version("0.0.0", "1.0.0"));

        assert!(!is_newer_version("0.0.1", "0.0.0"));
        assert!(!is_newer_version("0.0.0", "0.0.0"));
    }

    #[test]
    fn test_version_overflow_prevention() {
        // Very large version numbers should still work
        let large_version = "9999999.9999999.9999999";
        let small_version = "0.0.1";

        assert!(!is_newer_version(large_version, small_version));
        assert!(is_newer_version(small_version, large_version));
    }

    // =========================================================================
    // Real-world Version Scenarios
    // =========================================================================

    #[test]
    fn test_realistic_upgrade_scenarios() {
        // Typical patch upgrade
        assert!(is_newer_version("0.16.0", "0.16.1"));

        // Minor version upgrade
        assert!(is_newer_version("0.16.1", "0.17.0"));

        // Major version upgrade (breaking changes)
        assert!(is_newer_version("0.99.99", "1.0.0"));

        // Downgrade detection
        assert!(!is_newer_version("1.0.0", "0.99.99"));
    }

    #[test]
    fn test_current_version_upgrade_scenarios() {
        // Test against actual current version
        let current = CURRENT_VERSION;

        // Parse current version to generate test cases
        if let Ok(version) = semver::Version::parse(current) {
            // Next patch should be newer
            let next_patch = format!("{}.{}.{}", version.major, version.minor, version.patch + 1);
            assert!(is_newer_version(current, &next_patch));

            // Previous patch should be older (if patch > 0)
            if version.patch > 0 {
                let prev_patch =
                    format!("{}.{}.{}", version.major, version.minor, version.patch - 1);
                assert!(!is_newer_version(current, &prev_patch));
            }

            // Same version should return false
            assert!(!is_newer_version(current, current));
        }
    }

    // =========================================================================
    // GitHubRelease Deserialization Tests
    // =========================================================================

    #[test]
    fn test_github_release_deserialization() {
        let json = r#"{
            "tag_name": "v1.2.3",
            "html_url": "https://github.com/test/repo/releases/tag/v1.2.3"
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.2.3");
        assert_eq!(
            release.html_url,
            "https://github.com/test/repo/releases/tag/v1.2.3"
        );
    }

    #[test]
    fn test_github_release_deserialization_extra_fields() {
        // GitHub API returns many more fields - ensure we ignore extras
        let json = r#"{
            "tag_name": "v2.0.0",
            "html_url": "https://github.com/test/repo/releases/tag/v2.0.0",
            "id": 12345,
            "name": "Release 2.0.0",
            "draft": false,
            "prerelease": false,
            "created_at": "2024-01-01T00:00:00Z",
            "published_at": "2024-01-01T00:00:00Z",
            "body": "Release notes here"
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v2.0.0");
        assert_eq!(
            release.html_url,
            "https://github.com/test/repo/releases/tag/v2.0.0"
        );
    }

    #[test]
    fn test_github_release_deserialization_missing_fields() {
        // Missing required field should fail
        let json_missing_tag = r#"{
            "html_url": "https://github.com/test/repo/releases/tag/v1.0.0"
        }"#;

        let result: Result<GitHubRelease, _> = serde_json::from_str(json_missing_tag);
        assert!(result.is_err());

        let json_missing_url = r#"{
            "tag_name": "v1.0.0"
        }"#;

        let result: Result<GitHubRelease, _> = serde_json::from_str(json_missing_url);
        assert!(result.is_err());
    }

    #[test]
    fn test_github_release_debug_format() {
        let json = r#"{
            "tag_name": "v3.0.0",
            "html_url": "https://example.com"
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", release);
        assert!(debug_str.contains("GitHubRelease"));
        assert!(debug_str.contains("v3.0.0"));
    }

    // =========================================================================
    // Additional Edge Cases - Unicode and Special Characters
    // =========================================================================

    #[test]
    fn test_version_with_unicode_characters() {
        // Unicode should fail to parse as semver
        assert!(!is_newer_version("1.0.0", "1.0.0\u{200B}")); // zero-width space
        assert!(!is_newer_version("1.0.0", "١.٠.٠")); // Arabic numerals
        assert!(!is_newer_version("1.0.0", "1.0.0α")); // Greek alpha
        assert!(!is_newer_version("1.0.0", "1\u{2024}0\u{2024}0")); // one-dot leader
    }

    #[test]
    fn test_version_with_control_characters() {
        assert!(!is_newer_version("1.0.0\t", "1.0.0"));
        assert!(!is_newer_version("1.0.0\n", "1.0.0"));
        assert!(!is_newer_version("1.0.0\r", "1.0.0"));
        assert!(!is_newer_version("1.0.0\0", "1.0.0")); // null byte
    }

    // =========================================================================
    // Version String Length Edge Cases
    // =========================================================================

    #[test]
    fn test_very_long_version_parts() {
        // u64::MAX equivalent version parts should still work
        let max_part = u64::MAX.to_string();
        let huge_version = format!("{}.0.0", max_part);

        // Should parse successfully and compare correctly
        assert!(is_newer_version("0.0.1", &huge_version));
        assert!(!is_newer_version(&huge_version, "0.0.1"));
    }

    #[test]
    fn test_version_with_many_prerelease_identifiers() {
        // Multiple prerelease identifiers
        assert!(is_newer_version(
            "1.0.0-alpha.beta.gamma.delta.1",
            "1.0.0-alpha.beta.gamma.delta.2"
        ));
        assert!(is_newer_version(
            "1.0.0-alpha.1.beta.2",
            "1.0.0-alpha.1.beta.3"
        ));
    }

    #[test]
    fn test_prerelease_numeric_vs_string_comparison() {
        // Per semver spec: numeric identifiers have lower precedence than alphanumeric
        // Actually: numeric < alphanumeric when comparing mixed types
        assert!(is_newer_version("1.0.0-1", "1.0.0-alpha"));
        assert!(is_newer_version("1.0.0-999", "1.0.0-a"));
    }

    // =========================================================================
    // Build Metadata Edge Cases
    // =========================================================================

    #[test]
    fn test_build_metadata_complex() {
        // Build metadata with special allowed characters
        assert!(is_newer_version(
            "1.0.0+20130313144700",
            "1.0.0+20140313144700"
        ));
        assert!(is_newer_version("1.0.0+build.1", "1.0.0+build.2"));

        // Build metadata should be compared lexicographically by semver crate
        assert!(is_newer_version("1.0.0+aaa", "1.0.0+bbb"));
    }

    #[test]
    fn test_prerelease_with_build_metadata() {
        // Combined prerelease and build metadata
        assert!(is_newer_version("1.0.0-alpha+001", "1.0.0-alpha+002"));
        assert!(is_newer_version("1.0.0-alpha+build", "1.0.0-beta+build"));
        assert!(is_newer_version("1.0.0-alpha+build", "1.0.0"));
    }

    // =========================================================================
    // Tag Prefix Edge Cases
    // =========================================================================

    #[test]
    fn test_uppercase_v_prefix() {
        // Uppercase 'V' should NOT be stripped (only lowercase 'v' is handled)
        let tag = "V1.2.3";
        let stripped = tag.strip_prefix('v').unwrap_or(tag);
        assert_eq!(stripped, "V1.2.3"); // Unchanged
    }

    #[test]
    fn test_multiple_v_prefixes() {
        let tag = "vv1.2.3";
        let stripped = tag.strip_prefix('v').unwrap_or(tag);
        assert_eq!(stripped, "v1.2.3"); // Only first 'v' stripped
    }

    #[test]
    fn test_v_in_middle_of_tag() {
        let tag = "release-v1.2.3";
        let stripped = tag.strip_prefix('v').unwrap_or(tag);
        assert_eq!(stripped, "release-v1.2.3"); // Unchanged (no 'v' at start)
    }

    // =========================================================================
    // GitHubRelease Deserialization Edge Cases
    // =========================================================================

    #[test]
    fn test_github_release_with_null_values() {
        // Null for required string fields should fail
        let json = r#"{
            "tag_name": null,
            "html_url": "https://example.com"
        }"#;

        let result: Result<GitHubRelease, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_github_release_with_empty_strings() {
        // Empty strings are valid JSON strings
        let json = r#"{
            "tag_name": "",
            "html_url": ""
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "");
        assert_eq!(release.html_url, "");
    }

    #[test]
    fn test_github_release_with_unicode_in_tag() {
        let json = r#"{
            "tag_name": "v1.0.0-日本語",
            "html_url": "https://example.com"
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.0.0-日本語");
    }

    #[test]
    fn test_github_release_with_escaped_characters() {
        let json = r#"{
            "tag_name": "v1.0.0-test\nline",
            "html_url": "https://example.com/path?a=1&b=2"
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert!(release.tag_name.contains('\n'));
        assert!(release.html_url.contains('&'));
    }

    #[test]
    fn test_github_release_malformed_json() {
        let malformed_inputs = [
            r#"{"tag_name": "v1.0.0", "html_url": "https://example.com""#, // missing closing brace
            r#"{"tag_name": "v1.0.0" "html_url": "https://example.com"}"#, // missing comma
            r#"{tag_name: "v1.0.0", html_url: "https://example.com"}"#,    // unquoted keys
            r#"null"#,                                                     // null instead of object
            r#"[]"#, // array instead of object
        ];

        for input in malformed_inputs {
            let result: Result<GitHubRelease, _> = serde_json::from_str(input);
            assert!(result.is_err(), "Expected error for input: {}", input);
        }
    }

    // =========================================================================
    // Version Comparison Transitivity Tests
    // =========================================================================

    #[test]
    fn test_version_comparison_transitivity() {
        // If A < B and B < C, then A < C
        let versions = ["0.1.0", "0.2.0", "0.3.0", "1.0.0", "1.1.0", "2.0.0"];

        for i in 0..versions.len() {
            for j in (i + 1)..versions.len() {
                assert!(
                    is_newer_version(versions[i], versions[j]),
                    "{} should be older than {}",
                    versions[i],
                    versions[j]
                );
                assert!(
                    !is_newer_version(versions[j], versions[i]),
                    "{} should not be older than {}",
                    versions[j],
                    versions[i]
                );
            }
        }
    }

    #[test]
    fn test_version_comparison_reflexivity() {
        // A version is never newer than itself
        let versions = [
            "0.0.0",
            "0.1.0",
            "1.0.0",
            "1.0.0-alpha",
            "1.0.0-beta.1",
            "1.0.0+build",
            CURRENT_VERSION,
        ];

        for v in versions {
            assert!(
                !is_newer_version(v, v),
                "Version {} should not be newer than itself",
                v
            );
        }
    }

    // =========================================================================
    // Semver Spec Compliance Tests
    // =========================================================================

    #[test]
    fn test_semver_precedence_rules() {
        // From semver.org precedence rules:
        // 1.0.0-alpha < 1.0.0-alpha.1 < 1.0.0-alpha.beta < 1.0.0-beta <
        // 1.0.0-beta.2 < 1.0.0-beta.11 < 1.0.0-rc.1 < 1.0.0
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];

        for i in 0..ordered.len() - 1 {
            assert!(
                is_newer_version(ordered[i], ordered[i + 1]),
                "{} should be older than {}",
                ordered[i],
                ordered[i + 1]
            );
        }
    }

    #[test]
    fn test_numeric_identifier_comparison() {
        // Numeric identifiers always have lower precedence than alphanumeric
        assert!(is_newer_version("1.0.0-1", "1.0.0-2"));
        assert!(is_newer_version("1.0.0-2", "1.0.0-10")); // numeric comparison, not string
        assert!(is_newer_version("1.0.0-10", "1.0.0-100"));
    }

    // =========================================================================
    // LatestRelease Construction Edge Cases
    // =========================================================================

    #[test]
    fn test_latest_release_version_without_tag_prefix() {
        // Simulate what happens when tag_name doesn't have 'v' prefix
        let tag_name = "1.0.0";
        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

        let release = LatestRelease {
            version: version.to_string(),
            tag_name: tag_name.to_string(),
            html_url: "https://example.com".to_string(),
        };

        assert_eq!(release.version, "1.0.0");
        assert_eq!(release.tag_name, "1.0.0");
    }

    #[test]
    fn test_latest_release_with_prerelease_tag() {
        let tag_name = "v2.0.0-beta.1";
        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

        let release = LatestRelease {
            version: version.to_string(),
            tag_name: tag_name.to_string(),
            html_url: "https://github.com/test/repo/releases/tag/v2.0.0-beta.1".to_string(),
        };

        assert_eq!(release.version, "2.0.0-beta.1");
        assert!(is_newer_version("1.9.0", &release.version));
        assert!(!is_newer_version("2.0.0", &release.version)); // release > prerelease
    }

    // =========================================================================
    // Error Message and Logging Tests
    // =========================================================================

    #[test]
    fn test_invalid_version_returns_false_silently() {
        // These should return false without panicking
        // (the actual warning is logged, but we can't easily test that)
        let invalid_pairs = [
            ("garbage", "1.0.0"),
            ("1.0.0", "garbage"),
            ("1.0", "2.0"),
            ("", ""),
            ("   ", "1.0.0"),
            ("1.0.0.0", "1.0.0"),   // 4 parts
            ("1.0.0.0.0", "1.0.0"), // 5 parts
        ];

        for (current, latest) in invalid_pairs {
            // Should not panic
            let result = is_newer_version(current, latest);
            assert!(
                !result,
                "Invalid versions should return false: ({}, {})",
                current, latest
            );
        }
    }

    // =========================================================================
    // URL Validation Tests for GitHub Releases
    // =========================================================================

    #[test]
    fn test_github_url_components() {
        assert!(GITHUB_RELEASES_URL.contains("fed-stew"));
        assert!(GITHUB_RELEASES_URL.contains("stockpot"));
        assert!(GITHUB_RELEASES_URL.ends_with("/releases/latest"));
    }

    #[test]
    fn test_github_release_url_parsing() {
        let release = LatestRelease {
            version: "1.0.0".to_string(),
            tag_name: "v1.0.0".to_string(),
            html_url: "https://github.com/fed-stew/stockpot/releases/tag/v1.0.0".to_string(),
        };

        // Verify URL structure
        assert!(release.html_url.starts_with("https://github.com/"));
        assert!(release.html_url.contains("/releases/tag/"));
        assert!(release.html_url.ends_with(&release.tag_name));
    }
}
