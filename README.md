# Zorch

> A production-ready **AI Key Orchestration Platform** built in Rust. Expose a single OpenAI-compatible API that routes across multiple upstream providers (OpenAI, Anthropic, and any OpenAI-compatible backend) with per-key governance, dynamic pricing, billing, analytics, a sandboxed middleware engine, and a full admin dashboard.

Zorch is a **modular monolith**: one Rust workspace, one deployable binary, one Next.js dashboard — no microservices required. It proxies requests transparently to upstream providers (no response translation), encrypts provider keys at rest, and gives you fine-grained control over who can call what, how fast, and at what cost.

---

## Table of Contents

- [Why Zorch](#why-zorch)
- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [Provider Setup](#provider-setup)
- [Middleware Engine](#middleware-engine)
- [Admin Dashboard](#admin-dashboard)
- [Project Structure](#project-structure)
- [Development](#development)
- [Deployment](#deployment)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## Why Zorch

Running multiple AI providers in production raises a pile of operational questions that raw SDK calls don't answer:

- **Key rotation** across multiple keys per provider without downtime
- **Per-client governance**: rate limits, daily caps, spend budgets, model allowlists, access windows
- **Cost attribution**: who called what, how many tokens, what did it cost, on which key
- **Dynamic pricing**: markups per model that hot-reload without restart
- **Request policy**: redact secrets, block prompts, inject system instructions — without touching application code
- **Analytics**: latency percentiles, token trends, tag-level cost attribution
- **Resilience**: circuit breaking per backend, key failover, sticky routing

Zorch solves all of the above behind a single drop-in OpenAI-compatible endpoint.

---

## Features

### Gateway & Proxy
- **OpenAI-compatible** `/v1/chat/completions`, `/v1/embeddings`, `/v1/models`
- **Anthropic-compatible** `/v1/messages`
- **Streaming** via SSE with usage capture (`stream_options.include_usage`)
- **Raw pass-through** — forwards requests transparently, no cross-provider response translation
- **Multi-key providers** with sticky routing + automatic key failover on 5xx/429

### Governance (per API key)
- **Rate limiting**: per-minute (RPM) and per-day (RPD) sliding windows
- **Spend budgets**: hard `max_spend_usd` cap with 80% soft-limit alerting
- **Model allowlists**: restrict which models a key may call
- **Access windows**: time-of-day restrictions with IANA timezone support (e.g. 09:00–17:00 Asia/Bangkok)
- **Kill-switch**: `enforce_per_key_governance` flag to disable enforcement globally

### Billing & Pricing
- **Cost calculation** from `provider_model_config` pricing (input/output cost per 1M tokens + markup %)
- **Hot-reloadable** pricing engine (no restart on admin changes)
- **Latency breakdown**: provider latency vs. gateway latency in every billing record
- **Spend tracking** in Redis with 24h rolling TTL

### Middleware Engine
- **Sandboxed Rhai scripting** — no filesystem, network, `eval`, or provider-key access
- **Two phases**: `request.pre_governance` and `request.pre_upstream`
- **Per-API-key binding** — assign middleware configs to specific keys
- **Configurable limits**: ops, string size, array/map size, call depth
- **Failure modes**: `fail_open` (continue on error) or `fail_closed` (block on error)
- **Full audit log** of every middleware run in `middleware_runs`
- **Built-in starter scripts**: Token Reducer, Sensitive Marker, Request Blocker, Prompt Injector

### Resilience
- **Circuit breaker** per backend (Closed → Open → HalfOpen with probe calls)
- **Key failover** within a provider on retryable errors
- **Sticky routing** with configurable TTL (default 300s)
- **Retry with backoff + jitter** on 429/5xx upstream responses

### Observability
- **Prometheus metrics** at `/metrics`
- **OpenTelemetry tracing** hooks
- **ClickHouse inspector** for request metadata capture (`none` / `metadata_only` / `full`)
- **PostgreSQL `requests_log`** for billing and analytics queries

### Admin Dashboard
- Next.js 16 + React 19 + shadcn/ui dashboard
- API key management with governance, tags, access windows, middleware binding
- Provider CRUD with target models and encrypted API keys
- Model alias management with priority-based routing
- Dynamic pricing editor
- Middleware config editor with Monaco + validate + dry-run
- Analytics with charts (token usage, cost trends, latency percentiles, tag attribution)

---

## Architecture

```
                    ┌─────────────────────────────────────────────┐
                    │              Admin Dashboard                 │
                    │         (Next.js 16 + shadcn/ui)             │
                    └──────────────────┬──────────────────────────┘
                                       │ /api/v1/admin/*
                    ┌──────────────────▼──────────────────────────┐
   Client ──/v1──► │                zorch-api                      │  ──► PostgreSQL
   (OpenAI SDK)    │           (Axum HTTP server:8080)             │  ──► Redis/Dragonfly
                    │                                              │  ──► ClickHouse
                    │  ┌──────────────────────────────────────┐   │
                    │  │  Request Lifecycle                   │   │
                    │  │  1. Auth (SHA-256 key lookup)         │   │
                    │  │  2. Pre-governance middleware (Rhai)  │   │
                    │  │  3. Model resolution + routing        │   │
                    │  │  4. Governance pipeline (RL+budget)   │   │
                    │  │  5. Pre-upstream middleware (Rhai)    │   │
                    │  │  6. Circuit breaker + key failover    │   │
                    │  │  7. Proxy to upstream provider         │   │
                    │  │  8. Billing + spend recording          │   │
                    │  │  9. Inspector (ClickHouse capture)     │   │
                    │  └──────────────────────────────────────┘   │
                    └──────────────────┬──────────────────────────┘
                                       │
                    ┌──────────────────▼──────────────────────────┐
                    │         Upstream Providers                  │
                    │   OpenAI  •  Anthropic  •  Custom            │
                    └─────────────────────────────────────────────┘
```

### Workspace Crates

| Crate | Responsibility |
|---|---|
| `zorch-shared` | Core types, newtype IDs, `AppConfig`, `SecretVault` (AES-256-GCM), `AppError` → HTTP mapping |
| `zorch-db` | PostgreSQL pool, `ApiKey` row model |
| `zorch-cache` | Redis caches: `ModelProviderCache` (6h TTL), `StickyTargetKeyCache` (round-robin + sticky) |
| `zorch-telemetry` | OpenTelemetry tracing init + Prometheus metrics |
| `zorch-providers` | `AuthType` (bearer/anthropic/custom), `ProviderHttpClient` (retry+backoff), `ProxyProvider` (multi-key failover), `ProxyProviderRegistry`, `ModelResolver`, `BackendSelector` |
| `zorch-inspector` | `InspectorHook` trait, `ClickHouseInspector` for metadata capture |
| `zorch-gateway` | `GovernanceEngine`, `KeyLimits`, `RateLimiter`, `CircuitBreaker`, `AccessWindow`, `BillingEngine`, `PricingEngine`, `MiddlewareEngine` (Rhai), `RequestPipeline` |
| `zorch-api` | Axum server, route map, HTTP middleware stack (auth, request-id, timeout, inspector, CORS) |

---

## Quick Start

### Prerequisites

- **Rust 1.78+** (toolchain tested up to 1.96) with `cargo`
- **Docker** + Docker Compose
- **Node.js 20+** (for the admin dashboard)

### 1. Start Infrastructure

```bash
docker compose -f docker/docker-compose.yml up -d
```

This starts:

| Service | Image | Port |
|---|---|---|
| PostgreSQL | `postgres:18.4-alpine` | 5432 |
| ClickHouse | `clickhouse-server:26.4` | 8123 (HTTP), 9000 (native) |
| Dragonfly | `dragonfly:v1.39` (Redis-compatible) | 6379 |

### 2. Configure Environment

```bash
cp .env.example .env
```

Edit `.env` — at minimum set `ZORCH_ENCRYPTION_KEY` (32-byte hex or 32-char string) and `ZORCH_ADMIN_SECRET`.

### 3. Run Migrations

```bash
ZORCH_DATABASE_URL=postgres://postgres:postgres@localhost:5432/zorch \
  cargo sqlx migrate run
```

### 4. Build & Start the Backend

```bash
cargo build --release --bin zorch-api
source .env && ./target/release/zorch-api
```

The API server listens on `0.0.0.0:8080`.

### 5. Start the Admin Dashboard

```bash
cd apps/admin
npm install
npm run build
npm start
```

Dashboard available at `http://localhost:3000`. Set `ZORCH_API_URL=http://localhost:8080` in `apps/admin/.env.local` if the API is on a different host.

### 6. Create Your First API Key

Open the dashboard → **API Keys** → **New**, or via the admin API:

```bash
curl -X POST http://localhost:8080/api/v1/admin/api-keys \
  -H "X-Admin-Secret: $ZORCH_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"name": "My App", "allowed_models": ["gpt-4o", "claude-3-5-sonnet"]}'
```

### 7. Make a Request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-zorch-..." \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello!"}]}'
```

Works with any OpenAI-compatible SDK — just point `base_url` at Zorch.

### Smoke Tests

```bash
# Basic proxy smoke test
BASE_URL=http://localhost:8080 API_KEY=sk-zorch-... MODEL=gpt-4o \
  bash tests/smoke-test.sh

# Middleware smoke test
BASE_URL=http://localhost:8080 API_KEY=sk-zorch-... MODEL=gpt-4o \
  bash tests/middleware-smoke-test.sh
```

---

## Configuration

All configuration is via `ZORCH_`-prefixed environment variables (see `.env.example`):

| Variable | Default | Description |
|---|---|---|
| `ZORCH_DATABASE_URL` | — | PostgreSQL connection string |
| `ZORCH_CLICKHOUSE_URL` | — | ClickHouse HTTP URL (empty → noop inspector) |
| `ZORCH_REDIS_URL` | — | Redis/Dragonfly URL |
| `ZORCH_APP_PORT` | `8080` | HTTP server port |
| `ZORCH_RUST_LOG` | `info` | Tracing filter directive |
| `ZORCH_ENCRYPTION_KEY` | — | AES-256-GCM master key for `SecretVault` (provider key encryption) |
| `ZORCH_ADMIN_SECRET` | — | Shared secret for admin API access (`X-Admin-Secret` header) |
| `ZORCH_INSPECTOR_CAPTURE_LEVEL` | `metadata_only` | `none` / `metadata_only` / `full` |
| `ZORCH_TIMEOUT_SECS` | `60` | Upstream request timeout |
| `ZORCH_CIRCUIT_BREAKER_TIMEOUT_SECS` | `30` | How long a failed backend is excluded before probe requests |
| `ZORCH_STICKY_TARGET_KEY_TTL_SECS` | `300` | Client key → target key sticky mapping TTL |
| `ZORCH_ENFORCE_PER_KEY_GOVERNANCE` | `true` | Kill-switch: `false` skips per-key RPM/RPD/budget/allowlist |
| `ZORCH_DEFAULT_ORG_ID` | — | Default org when creating keys without explicit org |
| `ZORCH_OPENAI_API_KEY` | — | Fallback OpenAI key when no DB providers configured |
| `ZORCH_ANTHROPIC_API_KEY` | — | Fallback Anthropic key when no DB providers configured |
| `ZORCH_CORS_ALLOWED_ORIGINS` | — | Comma-separated admin origins (empty = allow any, dev only) |

---

## API Reference

### Health & Operations

| Endpoint | Method | Description |
|---|---|---|
| `/health` | GET | Liveness probe (200 if process running) |
| `/health/ready` | GET | Readiness probe (pings Postgres, Redis, ClickHouse) |
| `/metrics` | GET | Prometheus metrics |
| `/api-docs` | GET | OpenAPI 3.0 JSON spec |

### Proxy (OpenAI & Anthropic compatible)

| Endpoint | Method | Description |
|---|---|---|
| `/v1/chat/completions` | POST | OpenAI-compatible chat completions (stream when `stream: true`) |
| `/v1/chat/completions/stream` | POST | Explicit streaming chat completions (SSE) |
| `/v1/messages` | POST | Anthropic-compatible messages endpoint |
| `/v1/embeddings` | POST | OpenAI-compatible embeddings |
| `/v1/models` | GET | List available models |
| `/v1/models/:model_id` | GET | Get a specific model |

### Admin (`/api/v1/admin/*`)

Authentication: `X-Admin-Secret` header or a bearer token with `admin` scope.

| Resource | Methods |
|---|---|
| `/config` | GET |
| `/dashboard` | GET |
| `/api-keys` | GET, POST |
| `/api-keys/:id` | PUT, DELETE |
| `/api-keys/:id/tags` | PUT |
| `/api-keys/:id/middleware-configs` | GET, POST |
| `/api-keys/:id/middleware-configs/:config_id` | DELETE |
| `/providers` | GET, POST |
| `/providers/:id` | PUT, DELETE |
| `/providers/:id/active` | POST |
| `/providers/:id/targets` | GET |
| `/providers/:id/target-models` | GET, POST, DELETE (`:tm_id`), POST `/sync` |
| `/providers/:id/api-keys` | GET, POST, DELETE (`:key_id`), PUT `:key_id/active` |
| `/providers/preview-models` | POST |
| `/models` | GET, POST |
| `/models/:id` | GET, PUT, DELETE |
| `/models/:id/targets` | GET, POST, PUT (`:target_id`), DELETE (`:target_id`) |
| `/pricing` | GET, POST |
| `/pricing/:id` | DELETE |
| `/analytics` | GET |
| `/analytics/by-tag` | GET |
| `/middleware/configs` | GET, POST |
| `/middleware/configs/:id` | GET, PUT, DELETE |
| `/middleware/runs` | GET |
| `/middleware/validate` | POST |
| `/middleware/run` | POST |

---

## Provider Setup

A **provider** is an upstream API (OpenAI, Anthropic, or any OpenAI-compatible service). Each provider has:

- `name`, `base_url` (must include the API version root, e.g. `https://api.openai.com/v1`)
- `auth_type`: `bearer`, `anthropic`, or `custom` (with `auth_header_name` + `auth_prefix`)
- **Target models** — upstream model names the provider supports
- **Target API keys** — one or more encrypted keys (AES-256-GCM at rest via `SecretVault`)

| Auth type | Example base URL | Headers sent upstream |
|---|---|---|
| `bearer` | `https://api.openai.com/v1` | `Authorization: Bearer <key>` |
| `anthropic` | `https://api.anthropic.com/v1` | `x-api-key: <key>`, `anthropic-version: 2023-06-01` |
| `custom` | provider-specific `/v1` root | `<auth_header_name>: <auth_prefix> <key>` |

**Models** are public aliases. A model maps to one or more **targets** (`provider_id` + `target_model` + `priority`). When a request comes in for a model, the `ModelResolver` orders targets by priority (shuffling equal-priority groups for load balancing) and the proxy tries each in order with circuit-breaker health checks and key failover.

Zorch strips its own `/v1` prefix and appends the remaining path to the provider's `base_url`. For example, a request to `/v1/chat/completions` with a provider whose `base_url` is `https://api.openai.com/v1` is forwarded to `https://api.openai.com/v1/chat/completions`.

---

## Middleware Engine

Zorch's middleware engine runs [Rhai](https://rhai.rs) scripts against proxied requests. Scripts are stored in the database, bound to specific API keys, and run in priority order.

### Phases

| Phase | When it runs | Typical use |
|---|---|---|
| `request.pre_governance` | After auth, before rate limiting/governance | Token reduction, normalization |
| `request.pre_upstream` | After governance, before sending to provider | Blocking, redaction, prompt injection |

### Script Contract

```rhai
fn run(ctx, input, config) {
    // ctx:     { requestId, orgId, apiKeyId, providerId, modelId, route }
    // input:   { body: object, headers: object }
    // config:  the middleware config's JSON (your custom fields)

    let body = input.body;
    body.model = "gpt-4o-mini";

    return #{
        action: "continue",        // "continue" or "block"
        body: body,                // optional: modified request body
        headers: #{},              // optional: header overrides
        metadata: #{ changed: true },
        status_code: 200,          // optional: for block
        message: ""                // optional: for block
    };
}
```

### Sandbox

- No filesystem, network, `eval`, `import`, or `require` access
- Scripts cannot access provider API keys
- Configurable limits per config: `max_operations` (1M), `max_string_size` (64KB), `max_array_size` (10K), `max_map_size` (10K), `max_call_stack_depth` (64)

### Built-in Starter Scripts

Seeded by migration `0002` (unbound — assign to keys via the dashboard):

| Script | Phase | Failure mode | What it does |
|---|---|---|---|
| Token Reducer | `request.pre_governance` | `fail_open` | Trims whitespace in message content |
| Sensitive Marker | `request.pre_upstream` | `fail_closed` | Replaces configured literal strings |
| Request Blocker | `request.pre_upstream` | `fail_closed` | Blocks requests containing configured patterns |
| Prompt Injector | `request.pre_upstream` | `fail_open` | Injects a system prompt prefix/suffix |

See [`docs/middleware.md`](docs/middleware.md) for the full middleware guide.

---

## Admin Dashboard

Built with **Next.js 16**, **React 19**, **TypeScript 5**, **Tailwind CSS 4**, and **shadcn/ui**.

### Pages

| Page | Route | Purpose |
|---|---|---|
| Dashboard | `/` | RPM/TPM/error-rate metrics, recent activity |
| API Keys | `/api-keys` | List, create, edit, revoke keys |
| API Key Edit | `/api-keys/:id/edit` | Governance limits, tags, access windows, middleware binding |
| Providers | `/providers` | List, create, edit providers |
| Provider Detail | `/providers/:id` | Target models, encrypted API keys, key management |
| Models | `/models` | List, create model aliases |
| Model Targets | `/models/:id/targets` | Manage alias → provider/target mappings |
| Pricing | `/pricing` | Per-model pricing + markup editor |
| Middleware | `/middleware` | Config CRUD with Monaco editor + validate + dry-run |
| Middleware Runs | `/middleware/runs` | Audit log of middleware executions |
| Analytics | `/analytics` | Token usage, cost trends, latency percentiles, tag attribution |
| Settings | `/settings` | Gateway configuration view |

---

## Project Structure

```
zorch/
├── crates/
│   ├── zorch-shared/        # Types, config, crypto (SecretVault), errors
│   ├── zorch-db/            # PostgreSQL pool + row models
│   ├── zorch-cache/         # Redis: model-provider cache, sticky routing
│   ├── zorch-telemetry/     # OpenTelemetry tracing + Prometheus metrics
│   ├── zorch-providers/     # Proxy abstraction: auth, HTTP client, registry, routing
│   ├── zorch-inspector/     # ClickHouse request metadata capture
│   ├── zorch-gateway/       # Governance, billing, pricing, rate limit, circuit breaker, middleware
│   └── zorch-api/           # Axum HTTP server, routes, middleware stack
├── apps/
│   └── admin/               # Next.js 16 admin dashboard
│       ├── app/             # 20 route pages (App Router)
│       ├── components/      # 20 shadcn/ui + 19 feature components
│       └── lib/             # Typed API client, hooks, utils
├── docker/
│   ├── docker-compose.yml   # postgres, clickhouse, dragonfly, app, web
│   ├── docker-compose.prod.yml
│   ├── docker-compose.openwebui.yml
│   ├── Dockerfile           # 3-stage cargo-chef Rust build
│   ├── Dockerfile.web       # 2-stage Next.js standalone build
│   └── init-scripts/        # ClickHouse schema init
├── migrations/             # 5 SQL migrations (PostgreSQL)
├── tests/                   # Smoke test scripts
├── docs/                    # Middleware, provider setup, security guides
├── .cargo/config.toml       # rustflags = ["-D", "warnings"]
├── rustfmt.toml
├── .env.example
└── Cargo.toml               # Workspace root
```

### Data Model (PostgreSQL)

12 tables across 5 migrations:

| Table | Purpose |
|---|---|
| `organizations` | Tenant orgs |
| `api_keys` | Client keys with governance (RPM/RPD/budget/allowlist/access-window/tags) |
| `providers` | Upstream provider configs (base_url, auth_type) |
| `provider_target_models` | Upstream model names per provider |
| `provider_api_keys` | Encrypted upstream keys per provider |
| `models` | Public model aliases |
| `model_targets` | Alias → provider/target mappings with priority |
| `provider_model_config` | Per-model pricing (input/output cost per 1M + markup) |
| `middleware_configs` | Rhai script configs (phase, priority, failure_mode, limits) |
| `middleware_runs` | Audit log of every middleware execution |
| `api_key_middleware_configs` | Per-key middleware binding (join table) |
| `requests_log` | Billing/usage records with latency breakdown + tags |

ClickHouse table `inspector_requests` stores request metadata for the inspector.

---

## Development

### Build & Test

```bash
# Build all crates
cargo build --all

# Run unit tests (255 tests across 40 files)
cargo test --all

# Lint (warnings are denied via .cargo/config.toml)
cargo clippy --all -- -D warnings
cargo fmt --all -- --check

# Format
cargo fmt --all
```

### Admin Dashboard Development

```bash
cd apps/admin
npm install
npm run dev    # hot-reload dev server at :3000
npm run lint
npm run build  # production build
```

### Editor Setup

The repo includes `.vscode/` settings. Rust projects use `rustfmt.toml` (`edition = "2021"`, `max_width = 100`). Warnings are denied project-wide via `.cargo/config.toml`.

### Infrastructure

Start dependencies for local development:

```bash
docker compose -f docker/docker-compose.yml up -d postgres clickhouse dragonfly
```

---

## Deployment

### Docker Compose (full stack)

```bash
docker compose -f docker/docker-compose.yml up -d --build
```

This builds and starts all 5 services:
- `postgres` — PostgreSQL 18.4
- `clickhouse` — ClickHouse 26.4
- `dragonfly` — Dragonfly 1.39 (Redis-compatible)
- `app` — Rust API server (port 8081 → 8080)
- `web` — Next.js dashboard (port 3001 → 3000)

### Production Overlay

```bash
docker compose -f docker/docker-compose.yml -f docker/docker-compose.prod.yml up -d
```

Adds 2 app replicas with 512M memory limits.

### Dockerfiles

- **`docker/Dockerfile`** — 3-stage cargo-chef build (`rust:1.96-bookworm` planner → builder → `debian:bookworm-slim` runtime with non-root `app` user). All SQL queries use runtime `sqlx::query()` (no compile-time macros), so `SQLX_OFFLINE` is not required.
- **`docker/Dockerfile.web`** — 2-stage `node:22-alpine` build using Next.js standalone output.

---

## Roadmap

Items tracked in `docs/project_state.yaml`:

- **Dynamic pricing from DB** — `PricingEngine` currently seeds in code; load from `provider_model_config` table
- **Persist gateway rejections** — record blocked requests in `requests_log` (rate-limit/budget/access-window denials)
- **ClickHouse analytics reads** — currently analytics query Postgres; migrate heavy aggregations to ClickHouse
- **Real-time updates** — WebSocket/SSE for live dashboard metrics
- **Response-phase middleware** — `response.pre_client` and `inspector.pre_capture` phases (defined but not yet invoked)
- **Integration tests** — add Rust integration tests (currently shell smoke tests only)

---

## Contributing

1. Fork the repo and create a feature branch
2. Ensure `cargo clippy --all -- -D warnings` and `cargo fmt --all -- --check` pass
3. Ensure `cargo test --all` passes
4. For dashboard changes, ensure `npm run lint` and `npm run build` pass in `apps/admin`
5. Open a pull request with a clear description

### Project Stats

| Metric | Count |
|---|---|
| Workspace crates | 8 |
| Rust source files | 75 |
| Rust LOC | ~13,700 |
| Unit tests | 255 |
| Next.js pages | 20 |
| shadcn/ui components | 20 |
| Feature components | 19 |
| Admin REST endpoints | ~40 |
| PostgreSQL tables | 12 |
| Migrations | 5 |
| Built-in middleware scripts | 4 |

---