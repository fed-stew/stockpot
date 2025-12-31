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
