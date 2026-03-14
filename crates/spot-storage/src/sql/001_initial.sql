-- Settings table (replaces puppy.cfg)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER DEFAULT (unixepoch())
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    agent_name TEXT NOT NULL,
    created_at INTEGER DEFAULT (unixepoch()),
    updated_at INTEGER DEFAULT (unixepoch())
);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    token_count INTEGER,
    created_at INTEGER DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);

-- OAuth tokens table
CREATE TABLE IF NOT EXISTS oauth_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at INTEGER,
    account_id TEXT,
    extra_data TEXT,
    updated_at INTEGER DEFAULT (unixepoch())
);

-- Command history
CREATE TABLE IF NOT EXISTS command_history (
    id INTEGER PRIMARY KEY,
    command TEXT NOT NULL,
    created_at INTEGER DEFAULT (unixepoch())
);

-- Agent terminal sessions (which agent is active in which terminal)
CREATE TABLE IF NOT EXISTS terminal_sessions (
    session_id TEXT PRIMARY KEY,
    agent_name TEXT NOT NULL,
    updated_at INTEGER DEFAULT (unixepoch())
);
