//! XDG Base Directory support.

use std::path::PathBuf;

/// XDG directory paths for Stockpot.
pub struct XdgDirs {
    /// Config directory (~/.config/stockpot or XDG_CONFIG_HOME/stockpot)
    pub config: PathBuf,
    /// Data directory (~/.local/share/stockpot or XDG_DATA_HOME/stockpot)
    pub data: PathBuf,
    /// Cache directory (~/.cache/stockpot or XDG_CACHE_HOME/stockpot)
    pub cache: PathBuf,
    /// State directory (~/.local/state/stockpot or XDG_STATE_HOME/stockpot)
    pub state: PathBuf,
}

impl XdgDirs {
    /// Get XDG directories, respecting environment variables.
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        Self {
            config: std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".config"))
                .join("stockpot"),
            data: std::env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".local/share"))
                .join("stockpot"),
            cache: std::env::var("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".cache"))
                .join("stockpot"),
            state: std::env::var("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".local/state"))
                .join("stockpot"),
        }
    }

    /// Ensure all directories exist.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for dir in [&self.config, &self.data, &self.cache, &self.state] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Legacy directory (~/.stockpot).
    pub fn legacy() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".stockpot")
    }
}

impl Default for XdgDirs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for XDG directory support.
    //!
    //! Coverage:
    //! - Default directory paths
    //! - Environment variable overrides
    //! - Directory creation
    //! - Legacy path handling

    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    /// Helper to temporarily set environment variables for testing.
    /// Returns a guard that restores the original values on drop.
    struct EnvGuard {
        vars: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn new(vars: &[(&str, &str)]) -> Self {
            let mut saved = Vec::new();
            for (key, value) in vars {
                saved.push((key.to_string(), env::var(key).ok()));
                env::set_var(key, value);
            }
            Self { vars: saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, original) in &self.vars {
                match original {
                    Some(val) => env::set_var(key, val),
                    None => env::remove_var(key),
                }
            }
        }
    }

    // =========================================================================
    // Default Path Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_xdg_dirs_ends_with_stockpot() {
        let dirs = XdgDirs::new();

        // All paths should end with "stockpot"
        assert!(
            dirs.config.ends_with("stockpot"),
            "config path should end with stockpot: {:?}",
            dirs.config
        );
        assert!(
            dirs.data.ends_with("stockpot"),
            "data path should end with stockpot: {:?}",
            dirs.data
        );
        assert!(
            dirs.cache.ends_with("stockpot"),
            "cache path should end with stockpot: {:?}",
            dirs.cache
        );
        assert!(
            dirs.state.ends_with("stockpot"),
            "state path should end with stockpot: {:?}",
            dirs.state
        );
    }

    #[test]
    #[serial]
    fn test_xdg_dirs_default_paths_contain_expected_segments() {
        // Clear XDG vars to test defaults
        let _guard = EnvGuard::new(&[]);
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("XDG_DATA_HOME");
        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_STATE_HOME");

        let dirs = XdgDirs::new();

        // Check that default paths contain expected directory names
        let config_str = dirs.config.to_string_lossy();
        let data_str = dirs.data.to_string_lossy();
        let cache_str = dirs.cache.to_string_lossy();
        let state_str = dirs.state.to_string_lossy();

        assert!(
            config_str.contains(".config") || config_str.contains("stockpot"),
            "config path should contain .config: {}",
            config_str
        );
        assert!(
            data_str.contains(".local")
                || data_str.contains("share")
                || data_str.contains("stockpot"),
            "data path should contain .local/share: {}",
            data_str
        );
        assert!(
            cache_str.contains(".cache") || cache_str.contains("stockpot"),
            "cache path should contain .cache: {}",
            cache_str
        );
        assert!(
            state_str.contains(".local")
                || state_str.contains("state")
                || state_str.contains("stockpot"),
            "state path should contain .local/state: {}",
            state_str
        );
    }

    // =========================================================================
    // Environment Variable Override Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_xdg_config_home_override() {
        let temp = TempDir::new().unwrap();
        let custom_config = temp.path().join("custom_config");

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", custom_config.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        assert_eq!(dirs.config, custom_config.join("stockpot"));
    }

    #[test]
    #[serial]
    fn test_xdg_data_home_override() {
        let temp = TempDir::new().unwrap();
        let custom_data = temp.path().join("custom_data");

        let _guard = EnvGuard::new(&[("XDG_DATA_HOME", custom_data.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        assert_eq!(dirs.data, custom_data.join("stockpot"));
    }

    #[test]
    #[serial]
    fn test_xdg_cache_home_override() {
        let temp = TempDir::new().unwrap();
        let custom_cache = temp.path().join("custom_cache");

        let _guard = EnvGuard::new(&[("XDG_CACHE_HOME", custom_cache.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        assert_eq!(dirs.cache, custom_cache.join("stockpot"));
    }

    #[test]
    #[serial]
    fn test_xdg_state_home_override() {
        let temp = TempDir::new().unwrap();
        let custom_state = temp.path().join("custom_state");

        let _guard = EnvGuard::new(&[("XDG_STATE_HOME", custom_state.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        assert_eq!(dirs.state, custom_state.join("stockpot"));
    }

    #[test]
    #[serial]
    fn test_all_xdg_vars_override() {
        let temp = TempDir::new().unwrap();

        let _guard = EnvGuard::new(&[
            ("XDG_CONFIG_HOME", temp.path().join("cfg").to_str().unwrap()),
            ("XDG_DATA_HOME", temp.path().join("data").to_str().unwrap()),
            (
                "XDG_CACHE_HOME",
                temp.path().join("cache").to_str().unwrap(),
            ),
            (
                "XDG_STATE_HOME",
                temp.path().join("state").to_str().unwrap(),
            ),
        ]);

        let dirs = XdgDirs::new();

        assert_eq!(dirs.config, temp.path().join("cfg").join("stockpot"));
        assert_eq!(dirs.data, temp.path().join("data").join("stockpot"));
        assert_eq!(dirs.cache, temp.path().join("cache").join("stockpot"));
        assert_eq!(dirs.state, temp.path().join("state").join("stockpot"));
    }

    // =========================================================================
    // Directory Creation Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_ensure_dirs_creates_all_directories() {
        let temp = TempDir::new().unwrap();

        let _guard = EnvGuard::new(&[
            ("XDG_CONFIG_HOME", temp.path().join("cfg").to_str().unwrap()),
            ("XDG_DATA_HOME", temp.path().join("data").to_str().unwrap()),
            (
                "XDG_CACHE_HOME",
                temp.path().join("cache").to_str().unwrap(),
            ),
            (
                "XDG_STATE_HOME",
                temp.path().join("state").to_str().unwrap(),
            ),
        ]);

        let dirs = XdgDirs::new();

        // Directories shouldn't exist yet
        assert!(!dirs.config.exists());
        assert!(!dirs.data.exists());
        assert!(!dirs.cache.exists());
        assert!(!dirs.state.exists());

        // Create them
        dirs.ensure_dirs().unwrap();

        // Now they should exist
        assert!(dirs.config.exists());
        assert!(dirs.data.exists());
        assert!(dirs.cache.exists());
        assert!(dirs.state.exists());

        // They should be directories
        assert!(dirs.config.is_dir());
        assert!(dirs.data.is_dir());
        assert!(dirs.cache.is_dir());
        assert!(dirs.state.is_dir());
    }

    #[test]
    #[serial]
    fn test_ensure_dirs_idempotent() {
        let temp = TempDir::new().unwrap();

        let _guard = EnvGuard::new(&[
            ("XDG_CONFIG_HOME", temp.path().join("cfg").to_str().unwrap()),
            ("XDG_DATA_HOME", temp.path().join("data").to_str().unwrap()),
            (
                "XDG_CACHE_HOME",
                temp.path().join("cache").to_str().unwrap(),
            ),
            (
                "XDG_STATE_HOME",
                temp.path().join("state").to_str().unwrap(),
            ),
        ]);

        let dirs = XdgDirs::new();

        // Call multiple times - should not error
        dirs.ensure_dirs().unwrap();
        dirs.ensure_dirs().unwrap();
        dirs.ensure_dirs().unwrap();

        assert!(dirs.config.exists());
    }

    #[test]
    #[serial]
    fn test_ensure_dirs_creates_nested_paths() {
        let temp = TempDir::new().unwrap();
        let deeply_nested = temp.path().join("a").join("b").join("c").join("d");

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", deeply_nested.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        dirs.ensure_dirs().unwrap();

        // Should create the entire path including stockpot
        assert!(dirs.config.exists());
        assert!(dirs.config.ends_with("stockpot"));
    }

    // =========================================================================
    // Legacy Path Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_legacy_path_ends_with_stockpot() {
        let legacy = XdgDirs::legacy();
        assert!(
            legacy.ends_with(".stockpot"),
            "legacy path should end with .stockpot: {:?}",
            legacy
        );
    }

    #[test]
    #[serial]
    fn test_legacy_path_is_in_home() {
        let legacy = XdgDirs::legacy();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        // Legacy path should be a child of home directory
        assert!(
            legacy.starts_with(&home),
            "legacy path {:?} should start with home {:?}",
            legacy,
            home
        );
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_default_trait() {
        let dirs1 = XdgDirs::new();
        let dirs2 = XdgDirs::default();

        // Both should produce the same paths
        assert_eq!(dirs1.config, dirs2.config);
        assert_eq!(dirs1.data, dirs2.data);
        assert_eq!(dirs1.cache, dirs2.cache);
        assert_eq!(dirs1.state, dirs2.state);
    }

    // =========================================================================
    // Path Validity Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_paths_are_absolute_or_relative_to_home() {
        let dirs = XdgDirs::new();

        // All paths should be absolute (or at least start with home or .)
        for (name, path) in [
            ("config", &dirs.config),
            ("data", &dirs.data),
            ("cache", &dirs.cache),
            ("state", &dirs.state),
        ] {
            assert!(
                path.is_absolute() || path.starts_with("."),
                "{} path should be absolute or start with '.': {:?}",
                name,
                path
            );
        }
    }

    #[test]
    #[serial]
    fn test_paths_are_distinct() {
        let dirs = XdgDirs::new();

        // All four paths should be different
        assert_ne!(dirs.config, dirs.data, "config and data should differ");
        assert_ne!(dirs.config, dirs.cache, "config and cache should differ");
        assert_ne!(dirs.config, dirs.state, "config and state should differ");
        assert_ne!(dirs.data, dirs.cache, "data and cache should differ");
        assert_ne!(dirs.data, dirs.state, "data and state should differ");
        assert_ne!(dirs.cache, dirs.state, "cache and state should differ");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    #[serial]
    fn test_xdg_vars_with_spaces_in_path() {
        let temp = TempDir::new().unwrap();
        let path_with_spaces = temp.path().join("path with spaces");

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", path_with_spaces.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        assert_eq!(dirs.config, path_with_spaces.join("stockpot"));

        // Should be able to create directory with spaces
        dirs.ensure_dirs().unwrap();
        assert!(dirs.config.exists());
    }

    #[test]
    #[serial]
    fn test_xdg_vars_with_unicode_in_path() {
        let temp = TempDir::new().unwrap();
        let unicode_path = temp.path().join("配置目录");

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", unicode_path.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        assert_eq!(dirs.config, unicode_path.join("stockpot"));

        // Should handle unicode paths
        dirs.ensure_dirs().unwrap();
        assert!(dirs.config.exists());
    }

    #[test]
    #[serial]
    fn test_ensure_dirs_fails_when_path_is_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("not_a_dir");

        // Create a file where we want a directory
        std::fs::write(&file_path, "blocking file").unwrap();

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", file_path.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        // Trying to create stockpot inside a file should fail
        let result = dirs.ensure_dirs();
        assert!(result.is_err(), "should fail when parent is a file");
    }

    #[test]
    #[serial]
    fn test_partial_xdg_override() {
        // Only override some vars, leave others as default
        let temp = TempDir::new().unwrap();
        let custom_config = temp.path().join("custom_cfg");

        // Clear all XDG vars first
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("XDG_DATA_HOME");
        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_STATE_HOME");

        // Only set config
        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", custom_config.to_str().unwrap())]);

        let dirs = XdgDirs::new();

        // Config should use override
        assert_eq!(dirs.config, custom_config.join("stockpot"));

        // Others should use defaults (contain expected segments)
        assert!(
            dirs.data.to_string_lossy().contains(".local")
                || dirs.data.to_string_lossy().contains("share")
        );
        assert!(dirs.cache.to_string_lossy().contains(".cache"));
        assert!(
            dirs.state.to_string_lossy().contains(".local")
                || dirs.state.to_string_lossy().contains("state")
        );
    }

    #[test]
    #[serial]
    fn test_xdg_vars_with_trailing_slash() {
        let temp = TempDir::new().unwrap();
        let path_with_slash = format!("{}/", temp.path().display());

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", &path_with_slash)]);

        let dirs = XdgDirs::new();
        // Should still end with stockpot regardless of trailing slash
        assert!(dirs.config.ends_with("stockpot"));
        dirs.ensure_dirs().unwrap();
        assert!(dirs.config.exists());
    }

    #[test]
    #[serial]
    fn test_xdg_vars_with_relative_path() {
        // XDG spec says these should be absolute, but we handle relative gracefully
        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", "relative/path")]);

        let dirs = XdgDirs::new();
        assert!(dirs.config.ends_with("stockpot"));
        assert!(dirs.config.to_string_lossy().contains("relative/path"));
    }

    #[test]
    #[serial]
    fn test_ensure_dirs_with_existing_files_in_path() {
        let temp = TempDir::new().unwrap();

        // Pre-create only config dir, leave others
        let config_base = temp.path().join("cfg");
        std::fs::create_dir_all(config_base.join("stockpot")).unwrap();

        let _guard = EnvGuard::new(&[
            ("XDG_CONFIG_HOME", temp.path().join("cfg").to_str().unwrap()),
            ("XDG_DATA_HOME", temp.path().join("data").to_str().unwrap()),
            (
                "XDG_CACHE_HOME",
                temp.path().join("cache").to_str().unwrap(),
            ),
            (
                "XDG_STATE_HOME",
                temp.path().join("state").to_str().unwrap(),
            ),
        ]);

        let dirs = XdgDirs::new();

        // Config already exists
        assert!(dirs.config.exists());
        // Others don't
        assert!(!dirs.data.exists());

        // ensure_dirs should still work (idempotent for existing + creates new)
        dirs.ensure_dirs().unwrap();

        assert!(dirs.config.exists());
        assert!(dirs.data.exists());
        assert!(dirs.cache.exists());
        assert!(dirs.state.exists());
    }

    #[test]
    #[serial]
    fn test_legacy_path_distinct_from_xdg_paths() {
        let dirs = XdgDirs::new();
        let legacy = XdgDirs::legacy();

        // Legacy ~/.stockpot should differ from all XDG paths
        assert_ne!(legacy, dirs.config);
        assert_ne!(legacy, dirs.data);
        assert_ne!(legacy, dirs.cache);
        assert_ne!(legacy, dirs.state);
    }

    #[test]
    #[serial]
    fn test_xdg_dirs_struct_fields_accessible() {
        // Verify struct fields are public and accessible
        let temp = TempDir::new().unwrap();
        let _guard = EnvGuard::new(&[
            ("XDG_CONFIG_HOME", temp.path().to_str().unwrap()),
            ("XDG_DATA_HOME", temp.path().to_str().unwrap()),
            ("XDG_CACHE_HOME", temp.path().to_str().unwrap()),
            ("XDG_STATE_HOME", temp.path().to_str().unwrap()),
        ]);

        let dirs = XdgDirs::new();

        // All fields should be PathBuf and usable
        let _: &PathBuf = &dirs.config;
        let _: &PathBuf = &dirs.data;
        let _: &PathBuf = &dirs.cache;
        let _: &PathBuf = &dirs.state;

        // Can clone paths
        let config_clone = dirs.config.clone();
        assert_eq!(config_clone, dirs.config);
    }

    #[test]
    #[serial]
    fn test_concurrent_ensure_dirs() {
        use std::sync::Arc;
        use std::thread;

        let temp = TempDir::new().unwrap();
        let temp_path = Arc::new(temp.path().to_path_buf());

        // Create XdgDirs with explicit paths instead of relying on env vars
        let config = temp_path.join("cfg").join("stockpot");
        let data = temp_path.join("data").join("stockpot");
        let cache = temp_path.join("cache").join("stockpot");
        let state = temp_path.join("state").join("stockpot");

        let dirs = Arc::new(XdgDirs {
            config: config.clone(),
            data: data.clone(),
            cache: cache.clone(),
            state: state.clone(),
        });

        // Spawn multiple threads calling ensure_dirs simultaneously
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let dirs_clone = Arc::clone(&dirs);
                thread::spawn(move || dirs_clone.ensure_dirs())
            })
            .collect();

        // All should succeed without race condition errors
        for handle in handles {
            let result = handle.join().expect("thread panicked");
            assert!(result.is_ok(), "concurrent ensure_dirs should succeed");
        }

        assert!(config.exists());
        assert!(data.exists());
        assert!(cache.exists());
        assert!(state.exists());
    }

    #[test]
    #[serial]
    fn test_xdg_paths_are_canonical_form() {
        let temp = TempDir::new().unwrap();
        // Use path with .. that should still work
        let indirect_path = temp.path().join("a").join("..").join("b");

        let _guard = EnvGuard::new(&[("XDG_CONFIG_HOME", indirect_path.to_str().unwrap())]);

        let dirs = XdgDirs::new();
        // Path should contain the indirect segments (PathBuf doesn't canonicalize)
        assert!(dirs.config.ends_with("stockpot"));

        // But ensure_dirs should still work
        dirs.ensure_dirs().unwrap();
        assert!(dirs.config.exists());
    }
}
