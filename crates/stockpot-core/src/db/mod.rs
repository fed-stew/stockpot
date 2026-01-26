//! SQLite database for config, sessions, and OAuth tokens.

mod migrations;
mod schema;

use rusqlite::Connection;
use std::path::PathBuf;

pub use schema::*;

// =========================================================================
// Types for API Key Pool Management
// =========================================================================

/// Represents an API key in the pool for multi-key rotation support.
#[derive(Debug, Clone)]
pub struct PoolKey {
    pub id: i64,
    pub provider_name: String,
    pub api_key: String,
    pub priority: i32,
    pub label: Option<String>,
    pub is_active: bool,
    pub last_used_at: Option<i64>,
    pub last_error_at: Option<i64>,
    pub error_count: i32,
}

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

        // Migrate legacy API keys from api_keys table to api_key_pools table
        // This ensures all keys are in the unified pool system
        self.migrate_legacy_api_keys()?;

        Ok(())
    }

    /// Migrate existing API keys from the legacy `api_keys` table to the new `api_key_pools` table.
    /// This preserves backward compatibility while moving to the unified pool system.
    /// Keys are only migrated if they don't already exist in the pool.
    fn migrate_legacy_api_keys(&self) -> anyhow::Result<()> {
        // Get all legacy keys
        let legacy_keys = self.list_api_keys().unwrap_or_default();

        for key_name in legacy_keys {
            // Check if this key already exists in the pool
            if self.has_pool_keys(&key_name) {
                continue; // Already migrated or manually added to pool
            }

            // Get the actual key value
            if let Ok(Some(api_key)) = self.get_api_key(&key_name) {
                // Add to pool with label indicating it was migrated
                match self.save_pool_key(&key_name, &api_key, Some("Migrated from legacy"), Some(0))
                {
                    Ok(_) => {
                        tracing::info!(key_name = %key_name, "Migrated API key to pool");
                    }
                    Err(e) => {
                        // Log but don't fail - the key might already exist with same value
                        tracing::debug!(key_name = %key_name, error = %e, "Failed to migrate API key (may already exist)");
                    }
                }
            }
        }

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

    // =========================================================================
    // API Key Pool Storage (Multi-key support)
    // =========================================================================

    /// Save a new API key to the pool for a provider.
    /// Returns the ID of the inserted key.
    pub fn save_pool_key(
        &self,
        provider: &str,
        api_key: &str,
        label: Option<&str>,
        priority: Option<i32>,
    ) -> Result<i64, rusqlite::Error> {
        let priority = priority.unwrap_or(0);
        self.conn.execute(
            "INSERT INTO api_key_pools (provider_name, api_key, label, priority, updated_at)
             VALUES (?, ?, ?, ?, unixepoch())",
            rusqlite::params![provider, api_key, label, priority],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all keys for a provider, ordered by priority (active keys first).
    pub fn get_pool_keys(&self, provider: &str) -> Result<Vec<PoolKey>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_name, api_key, priority, label, is_active,
                    last_used_at, last_error_at, error_count
             FROM api_key_pools
             WHERE provider_name = ?
             ORDER BY is_active DESC, priority ASC, id ASC",
        )?;
        let rows = stmt.query_map([provider], |row| {
            Ok(PoolKey {
                id: row.get(0)?,
                provider_name: row.get(1)?,
                api_key: row.get(2)?,
                priority: row.get(3)?,
                label: row.get(4)?,
                is_active: row.get::<_, i32>(5)? == 1,
                last_used_at: row.get(6)?,
                last_error_at: row.get(7)?,
                error_count: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    /// Get only active keys for a provider, ordered by priority.
    pub fn get_active_pool_keys(&self, provider: &str) -> Result<Vec<PoolKey>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_name, api_key, priority, label, is_active,
                    last_used_at, last_error_at, error_count
             FROM api_key_pools
             WHERE provider_name = ? AND is_active = 1
             ORDER BY priority ASC, id ASC",
        )?;
        let rows = stmt.query_map([provider], |row| {
            Ok(PoolKey {
                id: row.get(0)?,
                provider_name: row.get(1)?,
                api_key: row.get(2)?,
                priority: row.get(3)?,
                label: row.get(4)?,
                is_active: row.get::<_, i32>(5)? == 1,
                last_used_at: row.get(6)?,
                last_error_at: row.get(7)?,
                error_count: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    /// Update key usage statistics (call after successful use).
    pub fn mark_key_used(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET last_used_at = unixepoch(), updated_at = unixepoch()
             WHERE id = ?",
            [key_id],
        )?;
        Ok(())
    }

    /// Update key error statistics (call after 429 or other error).
    pub fn mark_key_error(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET last_error_at = unixepoch(), error_count = error_count + 1, updated_at = unixepoch()
             WHERE id = ?",
            [key_id],
        )?;
        Ok(())
    }

    /// Reset error count for a key (call after successful use following errors).
    pub fn reset_key_errors(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET error_count = 0, updated_at = unixepoch()
             WHERE id = ?",
            [key_id],
        )?;
        Ok(())
    }

    /// Toggle key active status.
    pub fn set_key_active(&self, key_id: i64, is_active: bool) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET is_active = ?, updated_at = unixepoch()
             WHERE id = ?",
            rusqlite::params![if is_active { 1 } else { 0 }, key_id],
        )?;
        Ok(())
    }

    /// Delete a key from the pool.
    pub fn delete_pool_key(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM api_key_pools WHERE id = ?", [key_id])?;
        Ok(())
    }

    /// Update key priority (for reordering).
    pub fn update_key_priority(
        &self,
        key_id: i64,
        new_priority: i32,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET priority = ?, updated_at = unixepoch()
             WHERE id = ?",
            rusqlite::params![new_priority, key_id],
        )?;
        Ok(())
    }

    /// Check if a provider has any pool keys configured.
    pub fn has_pool_keys(&self, provider: &str) -> bool {
        let result: Result<i32, _> = self.conn.query_row(
            "SELECT 1 FROM api_key_pools WHERE provider_name = ? LIMIT 1",
            [provider],
            |row| row.get(0),
        );
        result.is_ok()
    }

    /// Count active keys for a provider.
    pub fn count_active_pool_keys(&self, provider: &str) -> Result<usize, rusqlite::Error> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM api_key_pools WHERE provider_name = ? AND is_active = 1",
            [provider],
            |row| row.get(0),
        )?;
        Ok(count as usize)
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

    // =========================================================================
    // API Key Pool Tests
    // =========================================================================

    #[test]
    fn test_save_pool_key_inserts_new_key() {
        let (_temp, db) = setup_test_db();

        let id = db
            .save_pool_key("OPENAI_API_KEY", "sk-test123", Some("Personal"), None)
            .unwrap();

        assert!(id > 0);
        let keys = db.get_pool_keys("OPENAI_API_KEY").unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].api_key, "sk-test123");
        assert_eq!(keys[0].label, Some("Personal".to_string()));
        assert!(keys[0].is_active);
    }

    #[test]
    fn test_save_pool_key_with_priority() {
        let (_temp, db) = setup_test_db();

        let id = db
            .save_pool_key("OPENAI_API_KEY", "sk-test", None, Some(5))
            .unwrap();

        let keys = db.get_pool_keys("OPENAI_API_KEY").unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].id, id);
        assert_eq!(keys[0].priority, 5);
    }

    #[test]
    fn test_save_pool_key_default_priority_is_zero() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("OPENAI_API_KEY", "sk-test", None, None)
            .unwrap();

        let keys = db.get_pool_keys("OPENAI_API_KEY").unwrap();
        assert_eq!(keys[0].priority, 0);
    }

    #[test]
    fn test_save_pool_key_prevents_duplicates() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("OPENAI_API_KEY", "sk-same-key", None, None)
            .unwrap();

        // Trying to insert the same key again should fail due to UNIQUE constraint
        let result = db.save_pool_key("OPENAI_API_KEY", "sk-same-key", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_pool_key_allows_same_key_different_provider() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("OPENAI_API_KEY", "sk-shared", None, None)
            .unwrap();
        let result = db.save_pool_key("ANTHROPIC_API_KEY", "sk-shared", None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_pool_keys_returns_empty_for_unknown_provider() {
        let (_temp, db) = setup_test_db();

        let keys = db.get_pool_keys("NONEXISTENT_PROVIDER").unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_get_pool_keys_orders_by_active_then_priority() {
        let (_temp, db) = setup_test_db();

        // Insert keys with various priorities
        let id1 = db
            .save_pool_key("TEST", "key-low-priority", None, Some(10))
            .unwrap();
        let id2 = db
            .save_pool_key("TEST", "key-high-priority", None, Some(1))
            .unwrap();
        let id3 = db
            .save_pool_key("TEST", "key-medium-priority", None, Some(5))
            .unwrap();

        // Disable one active key
        db.set_key_active(id2, false).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys.len(), 3);

        // Active keys should come first, ordered by priority
        assert!(keys[0].is_active); // key-medium (priority 5, active)
        assert!(keys[1].is_active); // key-low (priority 10, active)
        assert!(!keys[2].is_active); // key-high (priority 1, but inactive)
        assert_eq!(keys[0].id, id3);
        assert_eq!(keys[1].id, id1);
        assert_eq!(keys[2].id, id2);
    }

    #[test]
    fn test_get_active_pool_keys_filters_inactive() {
        let (_temp, db) = setup_test_db();

        let id1 = db.save_pool_key("TEST", "key-1", None, None).unwrap();
        db.save_pool_key("TEST", "key-2", None, None).unwrap();

        // Disable one key
        db.set_key_active(id1, false).unwrap();

        let active_keys = db.get_active_pool_keys("TEST").unwrap();
        assert_eq!(active_keys.len(), 1);
        assert_eq!(active_keys[0].api_key, "key-2");
    }

    #[test]
    fn test_get_active_pool_keys_orders_by_priority() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("TEST", "key-3", None, Some(3)).unwrap();
        db.save_pool_key("TEST", "key-1", None, Some(1)).unwrap();
        db.save_pool_key("TEST", "key-2", None, Some(2)).unwrap();

        let keys = db.get_active_pool_keys("TEST").unwrap();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].api_key, "key-1");
        assert_eq!(keys[1].api_key, "key-2");
        assert_eq!(keys[2].api_key, "key-3");
    }

    #[test]
    fn test_mark_key_used_updates_timestamp() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();

        // Initially last_used_at should be None
        let keys = db.get_pool_keys("TEST").unwrap();
        assert!(keys[0].last_used_at.is_none());

        db.mark_key_used(id).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert!(keys[0].last_used_at.is_some());
    }

    #[test]
    fn test_mark_key_error_increments_count() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();

        // Initially error_count should be 0
        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 0);
        assert!(keys[0].last_error_at.is_none());

        db.mark_key_error(id).unwrap();
        db.mark_key_error(id).unwrap();
        db.mark_key_error(id).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 3);
        assert!(keys[0].last_error_at.is_some());
    }

    #[test]
    fn test_reset_key_errors_clears_count() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();

        db.mark_key_error(id).unwrap();
        db.mark_key_error(id).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 2);

        db.reset_key_errors(id).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 0);
    }

    #[test]
    fn test_set_key_active_toggles_status() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();

        // Should start active
        let keys = db.get_pool_keys("TEST").unwrap();
        assert!(keys[0].is_active);

        // Deactivate
        db.set_key_active(id, false).unwrap();
        let keys = db.get_pool_keys("TEST").unwrap();
        assert!(!keys[0].is_active);

        // Reactivate
        db.set_key_active(id, true).unwrap();
        let keys = db.get_pool_keys("TEST").unwrap();
        assert!(keys[0].is_active);
    }

    #[test]
    fn test_delete_pool_key_removes_key() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys.len(), 1);

        db.delete_pool_key(id).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_delete_pool_key_nonexistent_succeeds() {
        let (_temp, db) = setup_test_db();

        // Should not error when deleting non-existent key
        db.delete_pool_key(99999).unwrap();
    }

    #[test]
    fn test_update_key_priority() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, Some(5)).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].priority, 5);

        db.update_key_priority(id, 10).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].priority, 10);
    }

    #[test]
    fn test_has_pool_keys_returns_true_when_exists() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("TEST", "key-1", None, None).unwrap();

        assert!(db.has_pool_keys("TEST"));
    }

    #[test]
    fn test_has_pool_keys_returns_false_when_missing() {
        let (_temp, db) = setup_test_db();

        assert!(!db.has_pool_keys("NONEXISTENT"));
    }

    #[test]
    fn test_has_pool_keys_returns_true_even_for_inactive() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();
        db.set_key_active(id, false).unwrap();

        // Should still return true - key exists even if inactive
        assert!(db.has_pool_keys("TEST"));
    }

    #[test]
    fn test_count_active_pool_keys() {
        let (_temp, db) = setup_test_db();

        let id1 = db.save_pool_key("TEST", "key-1", None, None).unwrap();
        db.save_pool_key("TEST", "key-2", None, None).unwrap();
        db.save_pool_key("TEST", "key-3", None, None).unwrap();

        assert_eq!(db.count_active_pool_keys("TEST").unwrap(), 3);

        db.set_key_active(id1, false).unwrap();

        assert_eq!(db.count_active_pool_keys("TEST").unwrap(), 2);
    }

    #[test]
    fn test_count_active_pool_keys_returns_zero_when_none() {
        let (_temp, db) = setup_test_db();

        assert_eq!(db.count_active_pool_keys("NONEXISTENT").unwrap(), 0);
    }

    #[test]
    fn test_pool_key_multiple_providers() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("OPENAI_API_KEY", "sk-openai-1", None, None)
            .unwrap();
        db.save_pool_key("OPENAI_API_KEY", "sk-openai-2", None, None)
            .unwrap();
        db.save_pool_key("ANTHROPIC_API_KEY", "sk-anthropic-1", None, None)
            .unwrap();

        let openai_keys = db.get_pool_keys("OPENAI_API_KEY").unwrap();
        let anthropic_keys = db.get_pool_keys("ANTHROPIC_API_KEY").unwrap();

        assert_eq!(openai_keys.len(), 2);
        assert_eq!(anthropic_keys.len(), 1);
        assert_eq!(anthropic_keys[0].api_key, "sk-anthropic-1");
    }

    #[test]
    fn test_pool_key_with_label() {
        let (_temp, db) = setup_test_db();

        db.save_pool_key("TEST", "key-1", Some("Work Account"), None)
            .unwrap();
        db.save_pool_key("TEST", "key-2", None, None).unwrap();

        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].label, Some("Work Account".to_string()));
        assert_eq!(keys[1].label, None);
    }

    #[test]
    fn test_pool_key_error_count_persists() {
        let (_temp, db) = setup_test_db();

        let id = db.save_pool_key("TEST", "key-1", None, None).unwrap();

        db.mark_key_error(id).unwrap();

        // Verify error state persists
        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 1);

        // Mark used (but don't reset errors explicitly)
        db.mark_key_used(id).unwrap();

        // Error count should NOT be reset by mark_key_used
        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 1);

        // Explicitly reset
        db.reset_key_errors(id).unwrap();
        let keys = db.get_pool_keys("TEST").unwrap();
        assert_eq!(keys[0].error_count, 0);
    }
}
