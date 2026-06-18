-- Add a human-friendly name to API keys.
-- Existing rows get a deterministic synthetic name from the key hash so
-- the column is safe to make NOT NULL without backfill risk.
ALTER TABLE api_keys
    ADD COLUMN IF NOT EXISTS name TEXT NOT NULL DEFAULT '';

UPDATE api_keys
SET name = 'Legacy key ' || substr(key_hash, 1, 8)
WHERE name = '';

ALTER TABLE api_keys
    ALTER COLUMN name DROP DEFAULT;
