# Zorch — AI Key Orchestration Platform

## Overview

Zorch is a production-ready **Rust modular monolith** for orchestrating AI provider API keys. It exposes a unified OpenAI-compatible REST proxy that routes requests across multiple upstream providers (OpenAI, Anthropic, and more) with per-model governance, cost attribution, dynamic pricing, billing, analytics, and hardening.

## Architecture

Zorch is built as a **Cargo workspace monolith** with 8 crates and a Next.js admin dashboard:

```
├── crates/
│   ├── zorch-shared      # Core types, errors, configuration, SecretVault
│   ├── zorch-db          # PostgreSQL connection pool + migrations
│   ├── zorch-cache       # Redis/Dragonfly model-provider cache
│   ├── zorch-telemetry   # OpenTelemetry tracing + Prometheus metrics
│   ├── zorch-providers   # Proxy provider abstraction + HTTP client
│   ├── zorch-inspector   # Request metadata capture to ClickHouse
│   ├── zorch-gateway     # Governance, billing, pricing, rate limiting, circuit breaker, middleware engine
│   └── zorch-api         # Axum HTTP server, routes, middleware
├── apps/
│   └── admin             # Next.js 14 + shadcn/ui dashboard
├── docker/
│   └── docker-compose.yml # PostgreSQL 16, ClickHouse 24, Dragonfly 1.20
└── docs/
    ├── provider-setup.md
    ├── middleware.md
    └── security.md
```

## Production Feature Set

| Phase | Feature | Status |
|---|---|---|
| 1 | Core Gateway + Inspector Foundation | Complete |
| 2 | Real Provider Integrations (OpenAI, Anthropic) | Complete |
| 3 | Model-Provider Cache + Provider Abstraction | Complete |
| 4+5 | Governance + Billing Engines | Complete |
| 6 | Analytics (ClickHouse aggregations, percentiles) | Complete |
| 7 | Admin Dashboard (Next.js, shadcn/ui) | Complete |
| 8 | Hardening (Rate Limiting, Circuit Breaker, OpenAPI) | Complete |
| 9 | Dynamic Pricing + Multi-Key Providers | Complete |
| 10 | Middleware Engine + Built-in Plugins | Complete |

## Quick Start

### Prerequisites

- Rust 1.78+ with `cargo`
- Docker + Docker Compose
- Node.js 20+ (for admin dashboard)

### 1. Start Infrastructure

```bash
cd docker && docker compose up -d
```

Starts:
- PostgreSQL 16 (port 5432)
- ClickHouse 24 (port 8123)
- Dragonfly 1.20 (port 6379)

### 2. Run Migrations

```bash
ZORCH_DATABASE_URL=postgres://postgres:postgres@localhost:5432/zorch cargo sqlx migrate run
```

### 3. Build + Test Backend

```bash
cargo test --all
cargo clippy --all -- -D warnings
cargo build --release --bin zorch-api
```

### 4. Start Server

```bash
ZORCH_RUST_LOG=info \
ZORCH_ENCRYPTION_KEY=replace-with-32-byte-hex-or-32-char-secret \
ZORCH_DATABASE_URL=postgres://postgres:postgres@localhost:5432/zorch \
ZORCH_CLICKHOUSE_URL=http://localhost:8123 \
ZORCH_REDIS_URL=redis://localhost:6379 \
  ./target/release/zorch-api
```

Server listens on `0.0.0.0:8080`.

### 5. Start Admin Dashboard

```bash
cd apps/admin
npm install
npm run build
npm start
```

Dashboard available at `http://localhost:3000`.

## API Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/health` | GET | Liveness probe (always returns 200 if the process is running) |
| `/health/ready` | GET | Readiness probe (pings PostgreSQL, Redis, ClickHouse) |
| `/api-docs` | GET | OpenAPI 3.0 JSON spec |
| `/metrics` | GET | Prometheus metrics |
| `/v1/chat/completions` | POST | OpenAI-compatible chat completions |
| `/v1/chat/completions/stream` | POST | Streaming chat completions (SSE) |
| `/v1/messages` | POST | Anthropic-compatible messages endpoint |
| `/v1/embeddings` | POST | OpenAI-compatible embeddings |
| `/v1/models` | GET | List available models |
| `/v1/models/:model_id` | GET | Get a specific model |
| `/api/v1/admin/dashboard` | GET | Admin dashboard stats |
| `/api/v1/admin/api-keys` | GET/POST | List / create API keys |
| `/api/v1/admin/api-keys/:id` | DELETE | Revoke an API key |
| `/api/v1/admin/providers` | GET/POST | List / create providers |
| `/api/v1/admin/providers/:id` | PUT/DELETE | Update / delete provider |
| `/api/v1/admin/pricing` | GET/POST | List / set per-model pricing |
| `/api/v1/admin/pricing/:id` | DELETE | Delete a pricing rule |
| `/api/v1/admin/analytics` | GET | Analytics data |
| `/api/v1/admin/middleware/plugins` | GET | List built-in middleware plugins |
| `/api/v1/admin/middleware/configs` | GET/POST | List / create middleware configs |
| `/api/v1/admin/middleware/configs/:id` | PUT/DELETE | Update / delete middleware config |
| `/api/v1/admin/middleware/runs` | GET | Recent middleware audit log |

## Direct Provider Setup

Provider `base_url` values must point at the provider API version root. Zorch strips its own `/v1` prefix from incoming gateway paths and appends the remaining path to the configured base URL:

| Protocol | Example base URL | Zorch route | Upstream path |
|---|---|---|---|
| `openai_compatible` | `https://api.openai.com/v1` | `/v1/chat/completions` | `/chat/completions` |
| `anthropic` | `https://api.anthropic.com/v1` | `/v1/messages` | `/messages` |
| Either | provider-specific `/v1` root | `/v1/models` | `/models` |

Provider config supports an explicit `protocol` field. Existing provider rows without a protocol are treated as `openai_compatible`.

```json
{
  "protocol": "openai_compatible",
  "models": ["gpt-4o-mini", "gpt-4o"],
  "api_key_encrypted": "..."
}
```

```json
{
  "protocol": "anthropic",
  "models": ["claude-3-5-sonnet-latest"],
  "api_key_encrypted": "..."
}
```

Auth headers are protocol-aware: OpenAI-compatible providers receive `Authorization: Bearer <key>`, while Anthropic providers receive `x-api-key: <key>` plus `anthropic-version: 2023-06-01`. Provider API keys are encrypted at rest and are not exposed in admin responses.

## Configuration

Environment variables (see `.env.example`):

| Variable | Default | Description |
|---|---|---|
| `ZORCH_DATABASE_URL` | — | PostgreSQL connection |
| `ZORCH_CLICKHOUSE_URL` | — | ClickHouse HTTP URL |
| `ZORCH_REDIS_URL` | — | Redis/Dragonfly URL |
| `ZORCH_APP_PORT` | 8080 | HTTP server port |
| `ZORCH_RUST_LOG` | info | Tracing filter |
| `ZORCH_ENCRYPTION_KEY` | — | AES-256-GCM master key for `SecretVault` |
| `ZORCH_INSPECTOR_CAPTURE_LEVEL` | metadata_only | `none` / `metadata_only` / `full` |
| `ZORCH_TIMEOUT_SECS` | 60 | Upstream request timeout |
| `ZORCH_OPENAI_API_KEY` | — | Fallback OpenAI key when no DB provider |
| `ZORCH_ANTHROPIC_API_KEY` | — | Fallback Anthropic key when no DB provider |
| `ZORCH_CORS_ALLOWED_ORIGINS` | — | Comma-separated list of admin origins allowed by CORS (empty = any origin, dev only) |

## Key Design Decisions

- **Raw proxy**: Zorch forwards requests transparently to upstream providers; it does not synthesize or translate responses across providers.
- **Provider secrets**: Encrypted at rest in PostgreSQL via `SecretVault` (`ZORCH_ENCRYPTION_KEY`). Environment keys are used only as a fallback when no DB providers exist.
- **Multi-key providers**: A provider can store multiple encrypted keys in `providers.config["api_keys_encrypted"]`. Requests try keys in order.
- **Dynamic pricing**: `provider_model_pricing` table drives `PricingEngine`. Admin pricing changes hot-reload without restart.
- **Streaming billing**: SSE streams are wrapped to capture `stream_options.include_usage` usage chunks and record billing asynchronously.
- **Failover**: Per-provider key failover is supported. Cross-provider failover is intentionally not implemented because Zorch is a proxy and cannot fake responses from a different provider.

## Project Stats

- 50+ Rust source files
- 170+ unit tests passing
- 8 workspace crates
- 7 Next.js pages (dashboard, api-keys, providers, pricing, middleware, analytics, root)
- 19 shadcn/ui components
- `-D warnings` enforced via `.cargo/config.toml`

## License

MIT
