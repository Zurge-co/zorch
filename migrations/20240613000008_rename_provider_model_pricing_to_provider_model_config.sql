-- Rationale: table now stores pricing + max_context_tokens (and future per-model
-- metadata), so "pricing" is no longer an accurate name. RENAME preserves all
-- data, indexes, constraints, and triggers. The trigger
-- trg_provider_model_pricing_updated_at is intentionally NOT renamed: it
-- automatically follows the table rename (bound by OID, not name), and renaming
-- it would be cosmetic-only risk with no functional benefit.

ALTER TABLE provider_model_pricing RENAME TO provider_model_config;

ALTER TABLE provider_model_config
    ADD COLUMN max_context_tokens BIGINT NOT NULL DEFAULT 0;
