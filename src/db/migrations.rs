//! Database migrations.

use rusqlite::Connection;

/// Run all migrations.
pub fn run_migrations(conn: &Connection) -> anyhow::Result<()> {
    // Create migrations table if it doesn't exist
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at INTEGER DEFAULT (unixepoch())
        );",
    )?;

    let migrations = [
        ("001_initial", include_str!("sql/001_initial.sql")),
        ("002_models", include_str!("sql/002_models.sql")),
        ("003_api_keys", include_str!("sql/003_api_keys.sql")),
        (
            "004_active_sessions",
            include_str!("sql/004_active_sessions.sql"),
        ),
        (
            "005_model_sources",
            include_str!("sql/005_model_sources.sql"),
        ),
    ];

    for (name, sql) in migrations {
        let applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM migrations WHERE name = ?)",
            [name],
            |row| row.get(0),
        )?;

        if !applied {
            tracing::info!("Running migration: {}", name);
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO migrations (name) VALUES (?)", [name])?;
        }
    }

    Ok(())
}
