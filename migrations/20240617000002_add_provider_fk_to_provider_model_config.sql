-- Add FK from provider_model_config to providers.
--
-- The `provider` text column is kept as a denormalized cache so the read path
-- (engine reload + JSON listing) doesn't have to JOIN. Inserts now resolve a
-- provider UUID by name and stamp both columns.
--
-- Uniqueness moves from (provider_name, model) to (provider_id, model) so a
-- provider renaming itself doesn't accidentally merge two distinct rows.

DO $$
DECLARE
    orphaned_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO orphaned_count
    FROM provider_model_config pmc
    LEFT JOIN providers p ON p.name = pmc.provider
    WHERE p.id IS NULL;

    IF orphaned_count > 0 THEN
        RAISE EXCEPTION
            'Cannot add FK: % provider_model_config row(s) reference a provider name not present in the providers table. Reconcile naming before re-running.',
            orphaned_count;
    END IF;
END $$;

ALTER TABLE provider_model_config
    ADD COLUMN provider_id UUID;

UPDATE provider_model_config pmc
SET provider_id = p.id
FROM providers p
WHERE p.name = pmc.provider;

ALTER TABLE provider_model_config
    ALTER COLUMN provider_id SET NOT NULL;

-- Original UNIQUE was created as a table-level constraint and auto-named with
-- the old table; PG renames it on RENAME TABLE so the name is now derived
-- from provider_model_config.
ALTER TABLE provider_model_config
    DROP CONSTRAINT IF EXISTS provider_model_config_provider_model_key;

ALTER TABLE provider_model_config
    ADD CONSTRAINT provider_model_config_provider_id_model_key
    UNIQUE (provider_id, model);

ALTER TABLE provider_model_config
    ADD CONSTRAINT provider_model_config_provider_id_fkey
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE;

DROP INDEX IF EXISTS idx_provider_model_pricing_lookup;

CREATE INDEX idx_provider_model_config_lookup
    ON provider_model_config(provider_id, model);
