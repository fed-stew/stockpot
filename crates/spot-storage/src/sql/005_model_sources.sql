-- Add source column to track where models came from
-- Values: 'catalog', 'oauth', 'custom'
ALTER TABLE models ADD COLUMN source TEXT DEFAULT 'custom';

-- Update existing rows based on their characteristics
UPDATE models SET source = 'oauth' WHERE model_type IN ('claude_code', 'chatgpt_oauth');
UPDATE models SET source = 'catalog' WHERE is_builtin = 1;

CREATE INDEX IF NOT EXISTS idx_models_source ON models(source);
