//! SQLite database for config, sessions, and OAuth tokens.

mod migrations;
mod schema;

use rusqlite::Connection;
use std::path::PathBuf;

pub use schema::*;

/// Database connection wrapper.
pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    /// Open the database at the default location.
    pub fn open() -> anyhow::Result<Self> {
        let path = Self::default_path()?;
        Self::open_at(path)
    }

    /// Open the database at a specific path.
    pub fn open_at(path: PathBuf) -> anyhow::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        
        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        
        Ok(Self { conn, path })
    }

    /// Get the default database path.
    pub fn default_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        
        Ok(data_dir.join("stockpot").join("spot.db"))
    }

    /// Run database migrations.
    pub fn migrate(&self) -> anyhow::Result<()> {
        migrations::run_migrations(&self.conn)?;
        Ok(())
    }

    /// Get a reference to the connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Get the database path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_and_migrate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let db = Database::open_at(path).unwrap();
        db.migrate().unwrap();
    }
}
