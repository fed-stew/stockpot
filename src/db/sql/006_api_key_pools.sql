-- API key pools for multi-key management per provider
-- Allows multiple API keys per provider with rotation support
CREATE TABLE IF NOT EXISTS api_key_pools (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_name TEXT NOT NULL,        -- e.g., "CEREBRAS_API_KEY", "OPENAI_API_KEY"
    api_key TEXT NOT NULL,
    priority INTEGER DEFAULT 0,          -- Lower = higher priority (used first)
    label TEXT,                          -- Optional user-friendly label like "Personal", "Work"
    is_active INTEGER DEFAULT 1,         -- 1=active, 0=disabled without deleting
    last_used_at INTEGER,                -- Unix timestamp of last successful use
    last_error_at INTEGER,               -- Unix timestamp of last error (429, etc.)
    error_count INTEGER DEFAULT 0,       -- Consecutive errors (reset on success)
    created_at INTEGER DEFAULT (unixepoch()),
    updated_at INTEGER DEFAULT (unixepoch()),
    UNIQUE(provider_name, api_key)       -- Prevent duplicate keys per provider
);

-- Index for efficient provider lookups
CREATE INDEX IF NOT EXISTS idx_api_key_pools_provider ON api_key_pools(provider_name, priority, is_active);
