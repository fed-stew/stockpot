//! SQLite database for config, sessions, and OAuth tokens.

mod migrations;
mod schema;
mod session_repository;

use rusqlite::Connection;
use std::path::PathBuf;

pub use schema::*;
pub use session_repository::SessionRepository;

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

        // Clear active session state on startup so we always start fresh.
        self.conn.execute("DELETE FROM active_sessions", [])?;

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

    // =========================================================================
    // API Key Storage
    // =========================================================================

    /// Save an API key to the database.
    pub fn save_api_key(&self, name: &str, api_key: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO api_keys (name, api_key, updated_at) VALUES (?, ?, unixepoch())
             ON CONFLICT(name) DO UPDATE SET api_key = excluded.api_key, updated_at = excluded.updated_at",
            [name, api_key],
        )?;
        Ok(())
    }

    /// Get an API key from the database.
    pub fn get_api_key(&self, name: &str) -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT api_key FROM api_keys WHERE name = ?")?;
        let result = stmt.query_row([name], |row| row.get(0));
        match result {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Check if an API key exists in the database.
    pub fn has_api_key(&self, name: &str) -> bool {
        self.get_api_key(name).ok().flatten().is_some()
    }

    /// List all stored API key names.
    pub fn list_api_keys(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM api_keys ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    /// Delete an API key.
    pub fn delete_api_key(&self, name: &str) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM api_keys WHERE name = ?", [name])?;
        Ok(())
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
