use rusqlite::Connection;

/// Repository for raw settings storage (the `settings` table).
pub struct SettingsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get a setting value by key.
    pub fn get(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let result: Result<String, _> =
            self.conn
                .query_row("SELECT value FROM settings WHERE key = ?", [key], |row| {
                    row.get(0)
                });

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Set a setting value (insert or update).
    pub fn set(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            [key, value],
        )?;
        Ok(())
    }

    /// Delete a setting by key.
    pub fn delete(&self, key: &str) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?", [key])?;
        Ok(())
    }

    /// List all settings, ordered by key.
    pub fn list(&self) -> Result<Vec<(String, String)>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = Vec::new();
        for row in rows {
            settings.push(row?);
        }
        Ok(settings)
    }

    /// List settings whose key starts with the given prefix, ordered by key.
    pub fn list_with_prefix(&self, prefix: &str) -> Result<Vec<(String, String)>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings WHERE key LIKE ? ORDER BY key")?;
        let pattern = format!("{}%", prefix);
        let rows = stmt.query_map([pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = Vec::new();
        for row in rows {
            settings.push(row?);
        }
        Ok(settings)
    }
}
