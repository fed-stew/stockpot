use rusqlite::Connection;

use crate::PoolKey;

/// Repository for API key pool storage (the `api_key_pools` table).
pub struct KeyPoolRepository<'a> {
    conn: &'a Connection,
}

impl<'a> KeyPoolRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Shared row mapping to eliminate duplication between `get_all` and `get_active`.
    fn map_pool_key_row(row: &rusqlite::Row) -> Result<PoolKey, rusqlite::Error> {
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
    }

    /// Save a new API key to the pool for a provider.
    /// Returns the ID of the inserted key.
    pub fn save(
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
    pub fn get_all(&self, provider: &str) -> Result<Vec<PoolKey>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_name, api_key, priority, label, is_active,
                    last_used_at, last_error_at, error_count
             FROM api_key_pools
             WHERE provider_name = ?
             ORDER BY is_active DESC, priority ASC, id ASC",
        )?;
        let rows = stmt.query_map([provider], Self::map_pool_key_row)?;
        rows.collect()
    }

    /// Get only active keys for a provider, ordered by priority.
    pub fn get_active(&self, provider: &str) -> Result<Vec<PoolKey>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_name, api_key, priority, label, is_active,
                    last_used_at, last_error_at, error_count
             FROM api_key_pools
             WHERE provider_name = ? AND is_active = 1
             ORDER BY priority ASC, id ASC",
        )?;
        let rows = stmt.query_map([provider], Self::map_pool_key_row)?;
        rows.collect()
    }

    /// Update key usage statistics (call after successful use).
    pub fn mark_used(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET last_used_at = unixepoch(), updated_at = unixepoch()
             WHERE id = ?",
            [key_id],
        )?;
        Ok(())
    }

    /// Update key error statistics (call after 429 or other error).
    pub fn mark_error(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET last_error_at = unixepoch(), error_count = error_count + 1, updated_at = unixepoch()
             WHERE id = ?",
            [key_id],
        )?;
        Ok(())
    }

    /// Reset error count for a key (call after successful use following errors).
    pub fn reset_errors(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET error_count = 0, updated_at = unixepoch()
             WHERE id = ?",
            [key_id],
        )?;
        Ok(())
    }

    /// Toggle key active status.
    pub fn set_active(&self, key_id: i64, is_active: bool) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET is_active = ?, updated_at = unixepoch()
             WHERE id = ?",
            rusqlite::params![if is_active { 1 } else { 0 }, key_id],
        )?;
        Ok(())
    }

    /// Delete a key from the pool.
    pub fn delete(&self, key_id: i64) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM api_key_pools WHERE id = ?", [key_id])?;
        Ok(())
    }

    /// Update key priority (for reordering).
    pub fn update_priority(&self, key_id: i64, new_priority: i32) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE api_key_pools
             SET priority = ?, updated_at = unixepoch()
             WHERE id = ?",
            rusqlite::params![new_priority, key_id],
        )?;
        Ok(())
    }

    /// Check if a provider has any pool keys configured.
    pub fn has_keys(&self, provider: &str) -> bool {
        let result: Result<i32, _> = self.conn.query_row(
            "SELECT 1 FROM api_key_pools WHERE provider_name = ? LIMIT 1",
            [provider],
            |row| row.get(0),
        );
        result.is_ok()
    }

    /// Count active keys for a provider.
    pub fn count_active(&self, provider: &str) -> Result<usize, rusqlite::Error> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM api_key_pools WHERE provider_name = ? AND is_active = 1",
            [provider],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}
