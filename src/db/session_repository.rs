//! SQLite-backed session persistence.
//!
//! This repository encapsulates all SQL operations for:
//! - Creating sessions and appending messages (`sessions`, `messages`)
//! - Tracking per-interface active sessions (`active_sessions`)
//! - Recording sub-agent invocations for observability (`sub_agent_invocations`)

use crate::db::Database;
use rusqlite::OptionalExtension;

/// Session persistence operations.
pub struct SessionRepository<'a> {
    db: &'a Database,
}

impl<'a> SessionRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Create a new session for an agent, returns session_id
    pub fn create_session(&self, agent_name: &str) -> Result<i64, rusqlite::Error> {
        let name = format!("{}-{}", agent_name, uuid::Uuid::new_v4());

        self.db.conn().execute(
            "INSERT INTO sessions (name, agent_name, created_at, updated_at)
             VALUES (?, ?, unixepoch(), unixepoch())",
            rusqlite::params![name, agent_name],
        )?;

        Ok(self.db.conn().last_insert_rowid())
    }

    /// Add a message to a session (stores serialized ModelRequest as JSON)
    pub fn add_message(
        &self,
        session_id: i64,
        role: &str,
        content: &str,
        token_count: Option<i64>,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.db.conn();
        conn.execute_batch("BEGIN")?;

        let result = (|| {
            conn.execute(
                "INSERT INTO messages (session_id, role, content, token_count, created_at)
                 VALUES (?, ?, ?, ?, unixepoch())",
                rusqlite::params![session_id, role, content, token_count],
            )?;
            let message_id = conn.last_insert_rowid();

            conn.execute(
                "UPDATE sessions SET updated_at = unixepoch() WHERE id = ?",
                [session_id],
            )?;

            Ok(message_id)
        })();

        match result {
            Ok(message_id) => {
                conn.execute_batch("COMMIT")?;
                Ok(message_id)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Get all messages for a session
    pub fn get_messages(
        &self,
        session_id: i64,
    ) -> Result<Vec<crate::db::Message>, rusqlite::Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, session_id, role, content, token_count, created_at
             FROM messages
             WHERE session_id = ?
             ORDER BY id",
        )?;

        let rows = stmt.query_map([session_id], |row| {
            Ok(crate::db::Message {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect()
    }

    /// Copy all messages from one session to a new session for a different agent
    /// Returns the new session_id
    pub fn copy_history_to_agent(
        &self,
        from_session_id: i64,
        to_agent: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.db.conn();
        conn.execute_batch("BEGIN")?;

        let result = (|| {
            let to_session_id = self.create_session(to_agent)?;

            conn.execute(
                "INSERT INTO messages (session_id, role, content, token_count, created_at)
                 SELECT ?, role, content, token_count, created_at
                 FROM messages
                 WHERE session_id = ?
                 ORDER BY id",
                rusqlite::params![to_session_id, from_session_id],
            )?;

            conn.execute(
                "UPDATE sessions SET updated_at = unixepoch() WHERE id = ?",
                [to_session_id],
            )?;

            Ok(to_session_id)
        })();

        match result {
            Ok(to_session_id) => {
                conn.execute_batch("COMMIT")?;
                Ok(to_session_id)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Delete a session and its messages (CASCADE handles messages)
    pub fn delete_session(&self, session_id: i64) -> Result<(), rusqlite::Error> {
        self.db
            .conn()
            .execute("DELETE FROM sessions WHERE id = ?", [session_id])?;
        Ok(())
    }

    /// Get or create active session for an interface
    pub fn get_active_session(&self, interface: &str) -> Result<Option<i64>, rusqlite::Error> {
        let session_id: Option<Option<i64>> = self
            .db
            .conn()
            .query_row(
                "SELECT session_id FROM active_sessions WHERE interface = ?",
                [interface],
                |row| row.get(0),
            )
            .optional()?;

        Ok(session_id.flatten())
    }

    /// Set the active session for an interface
    pub fn set_active_session(
        &self,
        interface: &str,
        agent_name: &str,
        session_id: i64,
    ) -> Result<(), rusqlite::Error> {
        self.db.conn().execute(
            "INSERT INTO active_sessions (interface, agent_name, session_id, created_at)
             VALUES (?, ?, ?, unixepoch())
             ON CONFLICT(interface) DO UPDATE SET
                agent_name = excluded.agent_name,
                session_id = excluded.session_id,
                created_at = excluded.created_at",
            rusqlite::params![interface, agent_name, session_id],
        )?;
        Ok(())
    }

    /// Clear all active sessions (called on startup)
    pub fn clear_active_sessions(&self) -> Result<(), rusqlite::Error> {
        self.db.conn().execute("DELETE FROM active_sessions", [])?;
        Ok(())
    }

    /// Record a sub-agent invocation
    pub fn record_sub_agent_invocation(
        &self,
        parent_session_id: Option<i64>,
        agent_name: &str,
        prompt: &str,
        response: Option<&str>,
        duration_ms: Option<i64>,
    ) -> Result<i64, rusqlite::Error> {
        self.db.conn().execute(
            "INSERT INTO sub_agent_invocations (
                parent_session_id,
                agent_name,
                prompt,
                response,
                duration_ms,
                created_at
             ) VALUES (?, ?, ?, ?, ?, unixepoch())",
            rusqlite::params![parent_session_id, agent_name, prompt, response, duration_ms],
        )?;

        Ok(self.db.conn().last_insert_rowid())
    }
}
