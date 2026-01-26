-- API keys table for provider authentication
-- Keys are stored with the provider/env_var name as the key
CREATE TABLE IF NOT EXISTS api_keys (
    name TEXT PRIMARY KEY,           -- e.g., "ZHIPU_API_KEY", "OPENAI_API_KEY"
    api_key TEXT NOT NULL,           -- The actual API key value
    created_at INTEGER DEFAULT (unixepoch()),
    updated_at INTEGER DEFAULT (unixepoch())
);
