-- Custom models table (replaces extra_models.json, claude_models.json, chatgpt_models.json)
CREATE TABLE IF NOT EXISTS models (
    name TEXT PRIMARY KEY,
    model_type TEXT NOT NULL DEFAULT 'custom_openai',
    model_id TEXT,
    context_length INTEGER DEFAULT 128000,
    supports_thinking INTEGER DEFAULT 0,
    supports_vision INTEGER DEFAULT 0,
    supports_tools INTEGER DEFAULT 1,
    description TEXT,
    api_endpoint TEXT,
    api_key_env TEXT,
    headers TEXT,  -- JSON object for custom headers
    azure_deployment TEXT,
    azure_api_version TEXT,
    is_builtin INTEGER DEFAULT 0,  -- 1 for defaults, 0 for user-added
    created_at INTEGER DEFAULT (unixepoch()),
    updated_at INTEGER DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_models_type ON models(model_type);
