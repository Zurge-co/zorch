# Middleware

Zorch's middleware engine allows you to transform, inspect, and block AI requests before they reach upstream providers.

## What is Middleware

Middleware plugins run at specific phases of the request lifecycle and can:
- Modify request bodies (e.g., normalize whitespace, redact sensitive data)
- Block requests (e.g., prevent secret keys from leaving the company)
- Inject instructions (e.g., add system prompts)
- Add audit metadata

## Available Phases

| Phase | When it runs | Typical use |
|-------|-------------|-------------|
| `request.pre_governance` | After auth, before rate limiting/governance | `token_reducer` |
| `request.pre_upstream` | After governance, before sending to provider | `sensitive_marker`, `prompt_injector`, `request_blocker` |
| `response.pre_client` | After provider response, before sending to client | Reserved for future use |
| `inspector.pre_capture` | Before request metadata is captured | Reserved for future use |

## Built-in Plugins

### token_reducer

Normalizes whitespace in message string content to reduce token usage.

**Config:**
```json
{
  "collapse_spaces": true,
  "trim_lines": true,
  "max_consecutive_newlines": 2
}
```

**Behavior:**
- Traverses `messages[].content` when content is a string
- Trims leading/trailing whitespace per line
- Collapses repeated spaces
- Limits consecutive newlines
- Leaves model name, tool definitions, and non-string content untouched

**Recommended failure mode:** `fail_open`

### sensitive_marker

Replaces configured sensitive regex patterns in message content.

**Config:**
```json
{
  "patterns": [
    {
      "name": "company_email",
      "regex": "[a-zA-Z0-9._%+-]+@company.com",
      "replacement": "[COMPANY_EMAIL]"
    }
  ]
}
```

**Behavior:**
- Applies regex patterns to `messages[].content` strings
- Replaces matches with configured replacement
- Reports redaction counts per pattern in metadata

**Recommended failure mode:** `fail_closed`

### request_blocker

Blocks requests containing forbidden patterns.

**Config:**
```json
{
  "patterns": [
    {
      "name": "secret_key",
      "regex": "sk-[a-zA-Z0-9]{20,}",
      "message": "Request appears to contain a secret key."
    }
  ]
}
```

**Behavior:**
- If any pattern matches, returns HTTP 403
- Does not send request upstream
- Audit log records pattern name, not raw matched value

**Recommended failure mode:** `fail_closed`

### prompt_injector

Injects an organization-level system prompt into OpenAI-style requests.

**Config:**
```json
{
  "position": "system_prefix",
  "text": "You are using company AI infrastructure. Do not reveal confidential data."
}
```

**Behavior:**
- `system_prefix`: Prepends to existing system message, or inserts new system message at index 0
- `system_suffix`: Appends to existing system message

**Recommended failure mode:** `fail_open`

## Failure Modes

| Mode | Behavior |
|------|----------|
| `fail_open` | Log error, continue request |
| `fail_closed` | Block request with middleware error |

## Scopes

Middleware can be scoped to specific requests. Empty scope means global (applies to all).

```json
{
  "organizations": ["org_123"],
  "api_keys": ["key_123"],
  "providers": ["openai"],
  "models": ["gpt-4o-mini"],
  "routes": ["/v1/chat/completions"]
}
```

A request must match ALL non-empty scope fields to trigger the middleware.

## Audit Logs

Every middleware run is recorded in the `middleware_runs` table:
- `request_id`: Links to the original request
- `plugin_key`, `phase`, `status`, `action`
- `duration_ms`: How long the plugin took
- `body_changed`: Whether the request body was modified
- `metadata`: Plugin-specific metadata (redaction counts, bytes saved, etc.)
- `error`: Error message if the plugin failed

View recent runs in the **Middleware** admin page under the **Recent Runs** tab.

## Configuring Middleware

1. Go to **Middleware** in the admin dashboard
2. Click **Add Config**
3. Select a built-in plugin
4. Choose the phase
5. Set priority (lower numbers run first)
6. Choose failure mode
7. Paste config JSON (example configs are provided)
8. Optionally add scope JSON to limit when the plugin runs
9. Save

## Security Model

- Middleware plugins cannot access provider API keys
- Middleware runs after authentication but before upstream requests
- Audit logs record metadata, not full request bodies
- The inspector captures metadata-only by default, never raw sensitive request bodies
