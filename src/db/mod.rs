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

        // Set restrictive file permissions (0600) on Unix systems.
        // The database contains sensitive data like API keys and OAuth tokens.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            {
                tracing::warn!("Failed to set database file permissions: {}", e);
            }
        }

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
    //! Unit tests for Database struct.
    //!
    //! Coverage:
    //! - Database opening/creation
    //! - Migration logic (including idempotency)
    //! - API key storage/retrieval/deletion
    //! - Helper methods (conn, path, default_path)

    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        (temp_dir, db)
    }

    // =========================================================================
    // Database Opening/Creation Tests
    // =========================================================================

    #[test]
    fn test_open_and_migrate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let db = Database::open_at(path).unwrap();
        db.migrate().unwrap();
    }

    #[test]
    fn test_open_at_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let nested_path = tmp
            .path()
            .join("deep")
            .join("nested")
            .join("dir")
            .join("test.db");

        // Parent dirs don't exist yet
        assert!(!nested_path.parent().unwrap().exists());

        let db = Database::open_at(nested_path.clone()).unwrap();

        // File should exist after open
        assert!(nested_path.exists());
        drop(db);
    }

    #[test]
    fn test_open_at_reuses_existing_database() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");

        // First open - create and populate
        {
            let db = Database::open_at(path.clone()).unwrap();
            db.migrate().unwrap();
            db.save_api_key("TEST_KEY", "secret123").unwrap();
        }

        // Second open - should see existing data
        {
            let db = Database::open_at(path).unwrap();
            // Don't need to migrate again for data to persist
            let key = db.get_api_key("TEST_KEY").unwrap();
            assert_eq!(key, Some("secret123".to_string()));
        }
    }

    #[test]
    fn test_default_path_returns_valid_path() {
        // This test depends on having a home/data directory, which should exist
        // in any normal environment
        let result = Database::default_path();

        // Should succeed on any system with a home directory
        if let Ok(path) = result {
            assert!(path.ends_with("stockpot/spot.db"));
            // Path should have a parent
            assert!(path.parent().is_some());
        }
        // If it fails (unusual env), that's acceptable for this test
    }

    #[test]
    fn test_conn_returns_valid_connection() {
        let (_temp, db) = setup_test_db();

        // Should be able to execute a simple query
        let result: i32 = db
            .conn()
            .query_row("SELECT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_path_returns_correct_path() {
        let tmp = TempDir::new().unwrap();
        let expected_path = tmp.path().join("my_database.db");
        let db = Database::open_at(expected_path.clone()).unwrap();

        assert_eq!(db.path(), &expected_path);
    }

    #[cfg(unix)]
    #[test]
    fn test_open_at_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("secure.db");

        let _db = Database::open_at(path.clone()).unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Database should have 0600 permissions");
    }

    // =========================================================================
    // Migration Tests
    // =========================================================================

    #[test]
    fn test_migrate_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let db = Database::open_at(path).unwrap();

        // Run migrations multiple times - should not error
        db.migrate().unwrap();
        db.migrate().unwrap();
        db.migrate().unwrap();
    }

    #[test]
    fn test_migrate_clears_active_sessions() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");

        // First session - create active session
        {
            let db = Database::open_at(path.clone()).unwrap();
            db.migrate().unwrap();

            // Create a valid session first (foreign key constraint)
            db.conn()
                .execute(
                    "INSERT INTO sessions (name, agent_name, created_at, updated_at)
                     VALUES ('test-session', 'stockpot', unixepoch(), unixepoch())",
                    [],
                )
                .unwrap();
            let session_id = db.conn().last_insert_rowid();

            // Insert an active session referencing the valid session
            db.conn()
                .execute(
                    "INSERT INTO active_sessions (interface, agent_name, session_id, created_at)
                     VALUES ('cli', 'stockpot', ?, unixepoch())",
                    [session_id],
                )
                .unwrap();

            // Verify it exists
            let count: i32 = db
                .conn()
                .query_row("SELECT COUNT(*) FROM active_sessions", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 1);
        }

        // Second session - migrate should clear active sessions
        {
            let db = Database::open_at(path).unwrap();
            db.migrate().unwrap();

            let count: i32 = db
                .conn()
                .query_row("SELECT COUNT(*) FROM active_sessions", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0, "active_sessions should be cleared on startup");
        }
    }

    #[test]
    fn test_migrate_creates_required_tables() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let db = Database::open_at(path).unwrap();
        db.migrate().unwrap();

        // Check that expected tables exist by querying sqlite_master
        let tables: Vec<String> = {
            let mut stmt = db
                .conn()
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            let rows = stmt.query_map([], |row| row.get(0)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };

        assert!(tables.contains(&"api_keys".to_string()));
        assert!(tables.contains(&"sessions".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"migrations".to_string()));
        assert!(tables.contains(&"active_sessions".to_string()));
    }

    // =========================================================================
    // API Key Storage Tests
    // =========================================================================

    #[test]
    fn test_save_api_key_inserts_new_key() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("OPENAI_API_KEY", "sk-test123").unwrap();

        let key = db.get_api_key("OPENAI_API_KEY").unwrap();
        assert_eq!(key, Some("sk-test123".to_string()));
    }

    #[test]
    fn test_save_api_key_upserts_existing_key() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("OPENAI_API_KEY", "old_value").unwrap();
        db.save_api_key("OPENAI_API_KEY", "new_value").unwrap();

        let key = db.get_api_key("OPENAI_API_KEY").unwrap();
        assert_eq!(key, Some("new_value".to_string()));
    }

    #[test]
    fn test_save_api_key_multiple_providers() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("OPENAI_API_KEY", "openai-key").unwrap();
        db.save_api_key("ANTHROPIC_API_KEY", "anthropic-key")
            .unwrap();
        db.save_api_key("ZHIPU_API_KEY", "zhipu-key").unwrap();

        assert_eq!(
            db.get_api_key("OPENAI_API_KEY").unwrap(),
            Some("openai-key".to_string())
        );
        assert_eq!(
            db.get_api_key("ANTHROPIC_API_KEY").unwrap(),
            Some("anthropic-key".to_string())
        );
        assert_eq!(
            db.get_api_key("ZHIPU_API_KEY").unwrap(),
            Some("zhipu-key".to_string())
        );
    }

    #[test]
    fn test_get_api_key_returns_none_for_missing() {
        let (_temp, db) = setup_test_db();

        let key = db.get_api_key("NONEXISTENT_KEY").unwrap();
        assert!(key.is_none());
    }

    #[test]
    fn test_has_api_key_returns_true_when_exists() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("TEST_KEY", "value").unwrap();

        assert!(db.has_api_key("TEST_KEY"));
    }

    #[test]
    fn test_has_api_key_returns_false_when_missing() {
        let (_temp, db) = setup_test_db();

        assert!(!db.has_api_key("NONEXISTENT_KEY"));
    }

    #[test]
    fn test_has_api_key_returns_false_after_delete() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("TEST_KEY", "value").unwrap();
        assert!(db.has_api_key("TEST_KEY"));

        db.delete_api_key("TEST_KEY").unwrap();
        assert!(!db.has_api_key("TEST_KEY"));
    }

    #[test]
    fn test_list_api_keys_empty() {
        let (_temp, db) = setup_test_db();

        let keys = db.list_api_keys().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_list_api_keys_returns_all_names() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("ZEBRA_KEY", "z").unwrap();
        db.save_api_key("ALPHA_KEY", "a").unwrap();
        db.save_api_key("MIDDLE_KEY", "m").unwrap();

        let keys = db.list_api_keys().unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_list_api_keys_sorted_alphabetically() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("ZEBRA_KEY", "z").unwrap();
        db.save_api_key("ALPHA_KEY", "a").unwrap();
        db.save_api_key("MIDDLE_KEY", "m").unwrap();

        let keys = db.list_api_keys().unwrap();
        assert_eq!(keys[0], "ALPHA_KEY");
        assert_eq!(keys[1], "MIDDLE_KEY");
        assert_eq!(keys[2], "ZEBRA_KEY");
    }

    #[test]
    fn test_delete_api_key_removes_key() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("DELETE_ME", "value").unwrap();
        assert!(db.get_api_key("DELETE_ME").unwrap().is_some());

        db.delete_api_key("DELETE_ME").unwrap();
        assert!(db.get_api_key("DELETE_ME").unwrap().is_none());
    }

    #[test]
    fn test_delete_api_key_nonexistent_succeeds() {
        let (_temp, db) = setup_test_db();

        // Should not error when deleting non-existent key
        db.delete_api_key("NEVER_EXISTED").unwrap();
    }

    #[test]
    fn test_delete_api_key_only_affects_target() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("KEEP_THIS", "keep").unwrap();
        db.save_api_key("DELETE_THIS", "delete").unwrap();

        db.delete_api_key("DELETE_THIS").unwrap();

        assert!(db.get_api_key("KEEP_THIS").unwrap().is_some());
        assert!(db.get_api_key("DELETE_THIS").unwrap().is_none());
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_api_key_with_special_characters() {
        let (_temp, db) = setup_test_db();

        let special_key = "sk-test_123!@#$%^&*()_+-=[]{}|;':\",./<>?";
        db.save_api_key("SPECIAL_KEY", special_key).unwrap();

        let retrieved = db.get_api_key("SPECIAL_KEY").unwrap();
        assert_eq!(retrieved, Some(special_key.to_string()));
    }

    #[test]
    fn test_api_key_with_unicode() {
        let (_temp, db) = setup_test_db();

        let unicode_key = "密钥-тест-🔑-キー";
        db.save_api_key("UNICODE_KEY", unicode_key).unwrap();

        let retrieved = db.get_api_key("UNICODE_KEY").unwrap();
        assert_eq!(retrieved, Some(unicode_key.to_string()));
    }

    #[test]
    fn test_api_key_empty_string() {
        let (_temp, db) = setup_test_db();

        db.save_api_key("EMPTY_KEY", "").unwrap();

        let retrieved = db.get_api_key("EMPTY_KEY").unwrap();
        assert_eq!(retrieved, Some("".to_string()));
    }

    #[test]
    fn test_api_key_very_long_value() {
        let (_temp, db) = setup_test_db();

        let long_key = "x".repeat(10000);
        db.save_api_key("LONG_KEY", &long_key).unwrap();

        let retrieved = db.get_api_key("LONG_KEY").unwrap();
        assert_eq!(retrieved, Some(long_key));
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let (_temp, db) = setup_test_db();

        // Verify foreign keys are enabled
        let fk_status: i32 = db
            .conn()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_status, 1, "Foreign keys should be enabled");
    }
}
