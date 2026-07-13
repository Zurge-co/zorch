# Middleware

Zorch's middleware engine runs user-defined [Rhai](https://rhai.rs) scripts against proxied requests. Each script can transform the request body, inspect headers, block the request, or attach audit metadata.

## What is Middleware

Middleware configs are Rhai scripts stored in the database. Each config is bound to one or more API keys. When a request arrives using a key that has bound middleware, the engine runs those scripts in priority order.

Scripts can:

- Modify request bodies (e.g., normalize whitespace, redact sensitive data)
- Block requests (e.g., prevent secret keys from leaving the company)
- Inject instructions (e.g., add system prompts)
- Add audit metadata

## Available Phases

| Phase | When it runs | Typical use |
|-------|-------------|-------------|
| `request.pre_governance` | After auth, before rate limiting/governance | Token reduction, normalization |
| `request.pre_upstream` | After governance, before sending to provider | Blocking, redaction, prompt injection |

## Rhai Script Contract

Each middleware config stores a Rhai source string. The script must define a function named `run`:

```rust
fn run(ctx, input, config) {
    // ctx: { requestId, orgId, apiKeyId, providerId, modelId, route }
    // input: { body: object, headers: object }
    // config: object (the middleware_configs.config JSON minus runtime bookkeeping)

    return #{
        action: "continue",
        body: input.body,
        metadata: #{}
    };
}
```

### Return value

The script must return an object map with one of the following shapes:

**Continue without changes:**

```rust
#{ action: "continue", metadata: #{ ... } }
```

**Continue with a modified body:**

```rust
let body = input.body;
body.model = "gpt-4o-mini";

return #{
    action: "continue",
    body: body,
    metadata: #{ changed: true }
};
```

**Block the request:**

```rust
return #{
    action: "block",
    status_code: 403,
    message: "Request contains blocked content.",
    metadata: #{ reason: "secret_key" }
};
```

### Sandbox limits

Each config can set the following limits in `config`:

| Field | Default | Description |
|-------|---------|-------------|
| `max_operations` | 1,000,000 | Maximum script operations before termination |
| `max_string_size` | 65,536 | Maximum length of any string |
| `max_array_size` | 10,000 | Maximum array length |
| `max_map_size` | 10,000 | Maximum object map entries |
| `max_call_stack_depth` | 64 | Maximum function call depth |

The Rhai engine is configured without filesystem, network, or `eval` access.

## Built-in Starter Scripts

The migration seeds four example configs as unbound starting points:

- **Token Reducer** (`request.pre_governance`) — trims whitespace in message content
- **Sensitive Marker** (`request.pre_upstream`) — replaces configured literal strings
- **Request Blocker** (`request.pre_upstream`) — blocks requests containing configured literal strings
- **Prompt Injector** (`request.pre_upstream`) — injects a system prompt

These seed configs are not assigned to any API key. Open the API key edit page and assign them to a key to activate them.

## Per-API-Key Binding

Middleware configs are global. To make a config run for a request, assign it to the request's API key:

```
GET    /api/v1/admin/api-keys/:id/middleware-configs
POST   /api/v1/admin/api-keys/:id/middleware-configs/:config_id
DELETE /api/v1/admin/api-keys/:id/middleware-configs/:config_id
```

In the admin dashboard, open an API key's edit page and use the **Middleware Scripts** section to assign or unassign configs.

## Failure Modes

| Mode | Behavior |
|------|----------|
| `fail_open` | Log error, continue request |
| `fail_closed` | Block request with middleware error |

## Audit Logs

Every middleware run is recorded in the `middleware_runs` table:

- `request_id`: Links to the original request
- `middleware_config_id`: Links to the config that ran
- `phase`, `status`, `action`
- `duration_ms`: How long the script took
- `body_changed`: Whether the request body was modified
- `metadata`: Script-specific metadata
- `error`: Error message if the script failed

View recent runs in the **Middleware** admin page under the **Runs** tab.

## Configuring Middleware

1. Go to **Middleware** in the admin dashboard
2. Click **Add Config**
3. Enter a name and choose a phase
4. Write or paste a Rhai script in the editor
5. Set runtime limits (defaults are shown)
6. Choose failure mode and priority
7. Save the config
8. Open the API key edit page and assign the config to the desired keys

## Security Model

- Middleware scripts cannot access the filesystem, network, or environment
- Scripts cannot use `eval`, `import`, `require`, or the `Function` constructor
- Scripts operate on request JSON only and cannot access provider API keys
- Audit logs record metadata, not full request bodies
