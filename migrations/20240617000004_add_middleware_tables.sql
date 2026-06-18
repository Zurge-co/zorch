CREATE TABLE IF NOT EXISTS middleware_plugins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_key TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    runtime TEXT NOT NULL DEFAULT 'builtin',
    version TEXT NOT NULL DEFAULT '1.0.0',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS middleware_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_key TEXT NOT NULL REFERENCES middleware_plugins(plugin_key),
    enabled BOOLEAN NOT NULL DEFAULT true,
    phase TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    failure_mode TEXT NOT NULL DEFAULT 'fail_closed',
    scope JSONB NOT NULL DEFAULT '{}',
    config JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT middleware_configs_phase_check CHECK (
        phase IN (
            'request.pre_governance',
            'request.pre_upstream',
            'response.pre_client',
            'inspector.pre_capture'
        )
    ),
    CONSTRAINT middleware_configs_failure_mode_check CHECK (
        failure_mode IN ('fail_open', 'fail_closed')
    )
);

CREATE INDEX IF NOT EXISTS idx_middleware_configs_enabled_phase
    ON middleware_configs(enabled, phase, priority, plugin_key, id);

CREATE TABLE IF NOT EXISTS middleware_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id TEXT,
    plugin_key TEXT NOT NULL,
    phase TEXT NOT NULL,
    status TEXT NOT NULL,
    action TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    body_changed BOOLEAN NOT NULL DEFAULT false,
    metadata JSONB NOT NULL DEFAULT '{}',
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_middleware_runs_created_at
    ON middleware_runs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_middleware_runs_plugin_status
    ON middleware_runs(plugin_key, status, created_at DESC);

INSERT INTO middleware_plugins (plugin_key, name, description, runtime, version)
VALUES
    ('token_reducer', 'Token reducer', 'Normalizes whitespace in string message content.', 'builtin', '1.0.0'),
    ('sensitive_marker', 'Sensitive marker', 'Replaces configured sensitive regex matches in string message content.', 'builtin', '1.0.0'),
    ('request_blocker', 'Request blocker', 'Blocks requests containing configured forbidden regex matches.', 'builtin', '1.0.0'),
    ('prompt_injector', 'Prompt injector', 'Injects an organization-level OpenAI-style system prompt.', 'builtin', '1.0.0')
ON CONFLICT (plugin_key) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    runtime = EXCLUDED.runtime,
    version = EXCLUDED.version,
    updated_at = NOW();

CREATE OR REPLACE FUNCTION update_middleware_configs_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER trg_middleware_configs_updated_at
BEFORE UPDATE ON middleware_configs
FOR EACH ROW
EXECUTE FUNCTION update_middleware_configs_updated_at();
