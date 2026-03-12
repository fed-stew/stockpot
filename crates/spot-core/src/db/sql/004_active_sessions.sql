-- Active session tracking (cleared on startup for fresh start)
-- Tracks which session is currently active for each interface type
CREATE TABLE IF NOT EXISTS active_sessions (
    id INTEGER PRIMARY KEY,
    interface TEXT NOT NULL CHECK(interface IN ('cli', 'tui', 'gui')),
    agent_name TEXT NOT NULL,
    session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
    created_at INTEGER DEFAULT (unixepoch()),
    UNIQUE(interface)  -- Only one active session per interface
);

-- Track sub-agent invocations for debugging/observability
CREATE TABLE IF NOT EXISTS sub_agent_invocations (
    id INTEGER PRIMARY KEY,
    parent_session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
    agent_name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    response TEXT,
    duration_ms INTEGER,
    created_at INTEGER DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_sub_agent_parent ON sub_agent_invocations(parent_session_id);
