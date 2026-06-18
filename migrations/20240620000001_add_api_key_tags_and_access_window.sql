ALTER TABLE api_keys
    ADD COLUMN IF NOT EXISTS tags JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE requests_log
    ADD COLUMN IF NOT EXISTS tags JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_requests_log_tags_gin
    ON requests_log USING GIN (tags);

ALTER TABLE api_keys
    ADD COLUMN IF NOT EXISTS allowed_hours_start SMALLINT NULL
        CHECK (allowed_hours_start IS NULL OR (allowed_hours_start >= 0 AND allowed_hours_start <= 23)),
    ADD COLUMN IF NOT EXISTS allowed_hours_end SMALLINT NULL
        CHECK (allowed_hours_end IS NULL OR (allowed_hours_end >= 0 AND allowed_hours_end <= 23)),
    ADD COLUMN IF NOT EXISTS window_timezone TEXT NULL;

-- Start/end must both be set or both be NULL (enforced at application layer,
-- columns allow independent NULL for migration safety).
