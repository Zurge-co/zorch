-- Migration: Convert middleware to Rhai scripts with per-API-key binding.
-- Removes built-in plugins, middleware_plugins table, scope column, and
-- response/inspector phases. Adds api_key_middleware_configs join table.

-- Drop index that references plugin_key.
DROP INDEX IF EXISTS idx_middleware_configs_enabled_phase;

-- Remove plugin_key column and its foreign key from middleware_configs.
ALTER TABLE middleware_configs DROP CONSTRAINT IF EXISTS middleware_configs_plugin_key_fkey;
ALTER TABLE middleware_configs DROP COLUMN IF EXISTS plugin_key;

-- Remove scope column.
ALTER TABLE middleware_configs DROP COLUMN IF EXISTS scope;

-- Add friendly name column.
ALTER TABLE middleware_configs ADD COLUMN name text NOT NULL DEFAULT '';

-- Restrict phases to the two request phases we support.
ALTER TABLE middleware_configs DROP CONSTRAINT IF EXISTS middleware_configs_phase_check;
ALTER TABLE middleware_configs ADD CONSTRAINT middleware_configs_phase_check CHECK (
    phase = ANY (ARRAY['request.pre_governance'::text, 'request.pre_upstream'::text])
);

-- Recreate index without plugin_key.
CREATE INDEX idx_middleware_configs_enabled_phase ON middleware_configs USING btree (enabled, phase, priority, id);

-- Join table binding API keys to middleware configs.
CREATE TABLE api_key_middleware_configs (
    api_key_id uuid NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    middleware_config_id uuid NOT NULL REFERENCES middleware_configs(id) ON DELETE CASCADE,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    PRIMARY KEY (api_key_id, middleware_config_id)
);

CREATE INDEX idx_akmc_middleware_config_id ON api_key_middleware_configs USING btree (middleware_config_id);

-- Drop plugin registry table; all middleware is now Rhai code stored in configs.
DROP TABLE IF EXISTS middleware_plugins;

-- Replace plugin_key in middleware_runs with a link to the originating config.
DROP INDEX IF EXISTS idx_middleware_runs_plugin_status;
ALTER TABLE middleware_runs DROP COLUMN IF EXISTS plugin_key;
ALTER TABLE middleware_runs ADD COLUMN middleware_config_id uuid REFERENCES middleware_configs(id) ON DELETE SET NULL;

CREATE INDEX idx_middleware_runs_config_status ON middleware_runs USING btree (middleware_config_id, status, created_at DESC);

-- Seed default Rhai middleware configs (unbound; admins assign them to API keys in the dashboard).
INSERT INTO middleware_configs (name, enabled, phase, priority, failure_mode, config) VALUES
(
    'Token Reducer',
    true,
    'request.pre_governance',
    10,
    'fail_open',
    '{
  "source": "fn run(ctx, input, config) {\n    let body = input.body;\n    if body.contains(\"messages\") {\n        for msg in body.messages {\n            if msg.contains(\"content\") && type_of(msg.content) == \"string\" {\n                let s = msg.content;\n                let lines = s.split(\"\n\");\n                let trimmed = [];\n                for line in lines {\n                    trimmed.push(line.trim());\n                }\n                s = trimmed.join(\"\n\");\n                let parts = s.split(\" \");\n                let non_empty = [];\n                for p in parts {\n                    if len(p) > 0 {\n                        non_empty.push(p);\n                    }\n                }\n                s = non_empty.join(\" \");\n                msg.content = s;\n            }\n        }\n    }\n    return #{ action: \"continue\", body: body, metadata: #{ trimmed: true } };\n}",
  "max_operations": 1000000,
  "max_string_size": 65536,
  "max_array_size": 10000,
  "max_map_size": 10000,
  "max_call_stack_depth": 64
}'::jsonb
),
(
    'Sensitive Marker',
    true,
    'request.pre_upstream',
    20,
    'fail_closed',
    '{
  "source": "fn run(ctx, input, config) {\n    let patterns = config.patterns;\n    let body = input.body;\n    let total = 0;\n    if body.contains(\"messages\") {\n        for msg in body.messages {\n            if msg.contains(\"content\") && type_of(msg.content) == \"string\" {\n                let s = msg.content;\n                for p in patterns {\n                    let target = p.target;\n                    let replacement = p.replacement;\n                    let count = 0;\n                    while s.contains(target) {\n                        s = s.replace(target, replacement);\n                        count += 1;\n                    }\n                    total += count;\n                }\n                msg.content = s;\n            }\n        }\n    }\n    return #{ action: \"continue\", body: body, metadata: #{ redactions: total } };\n}",
  "max_operations": 1000000,
  "max_string_size": 65536,
  "max_array_size": 10000,
  "max_map_size": 10000,
  "max_call_stack_depth": 64
}'::jsonb
),
(
    'Request Blocker',
    true,
    'request.pre_upstream',
    30,
    'fail_closed',
    '{
  "source": "fn run(ctx, input, config) {\n    let patterns = config.patterns;\n    let check_text = \"\";\n    if input.body.contains(\"messages\") {\n        for msg in input.body.messages {\n            if msg.contains(\"content\") && type_of(msg.content) == \"string\" {\n                check_text += msg.content;\n                check_text += \" \";\n            }\n        }\n    }\n    for p in patterns {\n        if check_text.contains(p.target) {\n            return #{\n                action: \"block\",\n                status_code: 403,\n                message: p.message,\n                metadata: #{ blocked_pattern: p.name }\n            };\n        }\n    }\n    return #{ action: \"continue\", metadata: #{ checked: true } };\n}",
  "max_operations": 1000000,
  "max_string_size": 65536,
  "max_array_size": 10000,
  "max_map_size": 10000,
  "max_call_stack_depth": 64
}'::jsonb
),
(
    'Prompt Injector',
    true,
    'request.pre_upstream',
    40,
    'fail_open',
    '{
  "source": "fn run(ctx, input, config) {\n    let text = config.text;\n    let position = config.position;\n    let body = input.body;\n    if !body.contains(\"messages\") {\n        body.messages = [];\n    }\n    let messages = body.messages;\n    if position == \"system_prefix\" {\n        if len(messages) > 0 && messages[0].role == \"system\" && type_of(messages[0].content) == \"string\" {\n            messages[0].content = text + \"\\n\\n\" + messages[0].content;\n        } else {\n            messages.insert(0, #{ role: \"system\", content: text });\n        }\n        return #{ action: \"continue\", body: body, metadata: #{ injected: true, position: position } };\n    }\n    if position == \"system_suffix\" {\n        let found = false;\n        for msg in messages {\n            if msg.role == \"system\" && type_of(msg.content) == \"string\" {\n                msg.content = msg.content + \"\\n\\n\" + text;\n                found = true;\n                break;\n            }\n        }\n        if !found {\n            messages.push(#{ role: \"system\", content: text });\n        }\n        return #{ action: \"continue\", body: body, metadata: #{ injected: true, position: position } };\n    }\n    return #{ action: \"continue\", metadata: #{ error: \"unknown position\" } };\n}",
  "max_operations": 1000000,
  "max_string_size": 65536,
  "max_array_size": 10000,
  "max_map_size": 10000,
  "max_call_stack_depth": 64
}'::jsonb
);
