# Security Notice

## Middleware Can Change Prompts

Middleware plugins run on your infrastructure and can modify request bodies before they leave your company. Only enable plugins and configurations you trust and understand.

## Provider API Keys Are Never Exposed to Middleware

Provider API keys are:
- Encrypted at rest in PostgreSQL using AES-256-GCM
- Decrypted only when building upstream request headers
- Never passed to middleware plugins
- Never logged in middleware audit metadata

## Raw Sensitive Prompt Logging Is Disabled by Default

The inspector captures metadata-only by default:
- Token counts
- Model and provider IDs
- Status codes and latency
- Middleware metadata (redaction counts, etc.)

It does **not** capture raw request bodies. If you need full request capture, change `ZORCH_INSPECTOR_CAPTURE_LEVEL` to `full`, but be aware this stores complete prompts including any sensitive data that middleware may have redacted.

## Recommended Practices

1. **sensitive_marker**: Use `fail_closed` so invalid regex configs cannot silently fail open
2. **request_blocker**: Use `fail_closed` so pattern match failures block rather than leak
3. **token_reducer**: Use `fail_open` since whitespace normalization is non-critical
4. **prompt_injector**: Use `fail_open` since prompt injection is advisory

## Audit Everything

Review the **Recent Runs** tab in the Middleware admin page regularly to verify plugins are behaving as expected.

## Pre-Launch Hardening Checklist

Before exposing Zorch to any non-trusted network, verify every item below:

- `ZORCH_ENCRYPTION_KEY` is set to a strong, unique 32-byte secret (not the example value).
- `ZORCH_ADMIN_SECRET` is set to a strong, unique random string (not the example value).
- `ZORCH_CORS_ALLOWED_ORIGINS` is set to the exact origin(s) of the admin dashboard.
- `ZORCH_INSPECTOR_CAPTURE_LEVEL` is `metadata_only` (default). Avoid `full` in production.
- `.env` is NOT committed to version control (the project ships a `.gitignore`).
- PostgreSQL, ClickHouse, and Redis are not exposed to the public internet.
- TLS termination is in front of the API (load balancer or reverse proxy).
- Database backups are configured and tested.
- At least one admin API key has been created and its raw value saved securely.
- All provider API keys are stored via the admin UI (encrypted at rest), not only in env vars.
- The `/health/ready` endpoint returns 200 from your orchestration platform.
