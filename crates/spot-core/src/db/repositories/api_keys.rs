use rusqlite::Connection;

/// Repository for legacy API key storage (the `api_keys` table).
pub struct ApiKeyRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ApiKeyRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Save an API key to the database.
    pub fn save(&self, name: &str, api_key: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO api_keys (name, api_key, updated_at) VALUES (?, ?, unixepoch())
             ON CONFLICT(name) DO UPDATE SET api_key = excluded.api_key, updated_at = excluded.updated_at",
            [name, api_key],
        )?;
        Ok(())
    }

    /// Get an API key from the database.
    pub fn get(&self, name: &str) -> Result<Option<String>, rusqlite::Error> {
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
    pub fn has(&self, name: &str) -> bool {
        self.get(name).ok().flatten().is_some()
    }

    /// List all stored API key names.
    pub fn list(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM api_keys ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    /// Delete an API key.
    pub fn delete(&self, name: &str) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM api_keys WHERE name = ?", [name])?;
        Ok(())
    }
}
