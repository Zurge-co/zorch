# Zorch: Dynamic Pricing Engine — Builder-Ready Execution Plan

## 1. State of the Union (with Deltas)

**Canonical source:** `docs/project_state.yaml` (updated 2026-06-18, last commit `f3a47b2`).

### Completed (per YAML)
- **Proxy:** pass-through, multi-key rotation, streaming, retry, middleware engine (request phases), circuit breaker, access windows, per-key governance (RPM/RPD/budget/model-allowlist).
- **Admin Dashboard:** API key CRUD, provider CRUD, pricing CRUD, middleware admin, analytics charts, dashboard metrics.
- **Analytics:** Postgres `requests_log`, Prometheus `/metrics`, cost attribution by tag.
- **Security:** AES-256-GCM key encryption, SHA-256 client key hashing, admin auth, constant-time equality.

### Partial / Missing (per YAML)
- `clickhouse-inspector` — partial (schema present, reads not wired).
- `middleware-response-phases` — missing (types/DB exist, never invoked).
- `real-time-updates` — missing (dashboard has no WebSocket/SSE).

### Spot-Check Deltas
1. **dynamic-pricing-engine** (`next_recommended` #1, high priority):  
   `PricingEngine::new()` in `crates/zorch-gateway/src/pricing.rs` still hardcodes 4 models. Server startup (`crates/zorch-api/src/server/mod.rs`) initializes the engine with `PricingEngine::new()` and never loads `provider_model_config` from the DB. Admin endpoints (`crates/zorch-api/src/routes/admin/pricing.rs`) already have `reload_pricing()` that builds a fresh engine from the DB table, but it is only invoked on admin mutation, not on boot. **Gap confirmed.**

2. **persist-gateway-rejections** (`next_recommended` #2, high priority):  
   `proxy_handler` in `crates/zorch-api/src/routes/v1/proxy/mod.rs` returns early when `run_governance_pipeline` errors (rate-limit, budget, circuit-breaker, model-allowlist) without writing a `BillingRecord`. By contrast, access-window rejections in `auth.rs` are already persisted via `BillingRecord::with_error`. **Gap confirmed.**

3. **integration-tests** (`next_recommended` #3, high priority):  
   `glob **/tests/**/*.rs` returned zero files across the workspace. **Zero integration tests confirmed.**

4. **Regressions:** No `TODO`/`FIXME` markers found in completed feature files.

---

## 2. Candidate Features with Scoring

| Rank | Feature | User/Biz Value | Complexity | Foundation | Risk | Time-to-Value |
|------|---------|----------------|------------|------------|------|---------------|
| 1 | **dynamic-pricing-engine** | 5/5 — billing accuracy is core; ops must add models without code deploys | 2/5 — schema & UI exist; only need boot-time load + seed migration | 4/5 — unlocks per-org pricing, model-specific governance | 1/5 — read-only cache, additive migration | 5/5 — one PR, immediate on restart |
| 2 | **persist-gateway-rejections** | 5/5 — invisible blocks make analytics useless for debugging | 2/5 — `BillingRecord::with_error` exists; insert before `?` | 3/5 — establishes rejection-telemetry pattern | 1/5 — additive inserts only | 5/5 — one PR |
| 3 | **add-integration-tests** | 4/5 — prevents regressions in proxy and admin | 4/5 — needs Postgres+Redis testcontainers, mock upstream | 5/5 — unlocks safe CI for all future work | 2/5 — test-only, but can slow CI | 2/5 — multi-PR before payoff |
| 4 | **wire-response-middleware** | 3/5 — enables response transformation / inspection plugins | 3/5 — must buffer streaming & non-streaming responses safely | 4/5 — completes middleware lifecycle | 3/5 — touches hot streaming path | 3/5 — needs streaming + non-streaming tests |
| 5 | **clickhouse-analytics-read** | 3/5 — fast analytics at scale, but Postgres works today | 4/5 — schema drift (missing `middleware_metadata`), needs CH in dev | 3/5 — enables real-time large-scale analytics | 3/5 — optional dependency drift | 2/5 — blocked by dev-env friction |

### Justification for #1 Choice
**Dynamic Pricing Engine** is the highest-ROI next feature because it fixes a live billing inaccuracy: any new model added via the admin dashboard currently costs `$0` after a server restart until an admin mutates pricing again. The schema, admin UI, and hot-reload logic are already built; the missing piece is a seed migration and a single startup load call. It ships in one PR, requires no new dependencies, and removes the last hardcoded operational constant from the proxy path.

---

## 3. Deep-Dive Technical Plan for #1 — Dynamic Pricing Engine

### 3.1 Goal & Definition of Done
When the Zorch server boots, it loads every row from `provider_model_config` into the in-memory `PricingEngine`. If the table is empty, the engine starts empty and cost calculations safely return `(0.0, 0.0)`. The four legacy hardcoded prices are removed from Rust code and instead seeded into the database via a migration so existing deployments retain backward compatibility. Admin pricing mutations (`POST /api/v1/admin/pricing` and `DELETE /api/v1/admin/pricing/:id`) continue to hot-reload the engine without restart. The admin dashboard requires zero changes; the existing **Providers → Model Pricing** editor and the cross-provider **Pricing** audit page already read and write the same table.

### 3.2 Schema Changes

**No DDL changes are required.** The existing `provider_model_config` table (created by migrations `20240613000006`, renamed in `20240613000008`, and FK-enhanced in `20240617000002`) already stores:
- `provider_id UUID NOT NULL` (FK to `providers`)
- `provider TEXT NOT NULL` (denormalized read cache)
- `model TEXT NOT NULL`
- `input_cost_per_1m DOUBLE PRECISION NOT NULL`
- `output_cost_per_1m DOUBLE PRECISION NOT NULL`
- `markup_percent DOUBLE PRECISION DEFAULT 0.0`
- `max_context_tokens BIGINT NOT NULL DEFAULT 0`
- `UNIQUE (provider_id, model)`
- `INDEX idx_provider_model_config_lookup (provider_id, model)`

**One data-only seed migration** is added to move the legacy hardcoded prices into the database, preserving backward compatibility for deployments that have never inserted pricing rows manually.

```sql
-- migrations/20240622000001_seed_hardcoded_pricing.sql
-- Idempotent seed: inserts the 4 legacy hardcoded prices only when the provider exists.

INSERT INTO provider_model_config (provider_id, provider, model, input_cost_per_1m, output_cost_per_1m, markup_percent, max_context_tokens)
SELECT p.id, p.name, 'gpt-4o', 2500.0, 10000.0, 0.0, 0
FROM providers p WHERE p.name = 'openai'
ON CONFLICT (provider_id, model) DO NOTHING;

INSERT INTO provider_model_config (provider_id, provider, model, input_cost_per_1m, output_cost_per_1m, markup_percent, max_context_tokens)
SELECT p.id, p.name, 'gpt-4o-mini', 150.0, 600.0, 0.0, 0
FROM providers p WHERE p.name = 'openai'
ON CONFLICT (provider_id, model) DO NOTHING;

INSERT INTO provider_model_config (provider_id, provider, model, input_cost_per_1m, output_cost_per_1m, markup_percent, max_context_tokens)
SELECT p.id, p.name, 'claude-3-5-sonnet', 3000.0, 15000.0, 0.0, 0
FROM providers p WHERE p.name = 'anthropic'
ON CONFLICT (provider_id, model) DO NOTHING;

INSERT INTO provider_model_config (provider_id, provider, model, input_cost_per_1m, output_cost_per_1m, markup_percent, max_context_tokens)
SELECT p.id, p.name, 'claude-3-opus', 15000.0, 75000.0, 0.0, 0
FROM providers p WHERE p.name = 'anthropic'
ON CONFLICT (provider_id, model) DO NOTHING;
```

**Justification:**
- `ON CONFLICT DO NOTHING` makes the migration idempotent and safe to re-run.
- Using `SELECT FROM providers` ensures FK integrity; if a provider does not yet exist, nothing is inserted (the admin can still add pricing later via the UI).
- Seeding data rather than keeping hardcoded Rust values means the source of truth is exclusively the DB, while existing users who never created pricing rows still see the same costs after upgrading.

### 3.3 Data Model

**Modified struct / impl:**
- `crates/zorch-gateway/src/pricing.rs` — `PricingEngine::new()`

**Current behavior:**
```rust
pub fn new() -> Self {
    let mut engine = Self { prices: HashMap::new() };
    engine.register(ModelPricing { ... }); // 4 hardcoded rows
    engine
}
```

**New behavior:**
```rust
pub fn new() -> Self {
    Self { prices: HashMap::new() }
}

pub fn from_rows(rows: &[sqlx::postgres::PgRow]) -> Self {
    let mut engine = Self::new();
    for row in rows {
        engine.register(row_to_engine_pricing(row));
    }
    engine
}
```

- `ModelPricing` requires no changes; its fields already map 1-to-1 to the DB columns.
- `row_to_engine_pricing` already exists in `crates/zorch-api/src/routes/admin/pricing.rs` (line 210). For crate hygiene, a copy or re-export should live in `zorch-gateway` so the engine can be built from DB rows without depending on `zorch-api`. **Decision:** move `row_to_engine_pricing` (and its `sqlx` import) into `zorch-gateway/src/pricing.rs` as a `pub(crate)` helper, or keep the startup loader in `zorch-api` and call `engine.register(...)` manually. The simplest path is to let `zorch-api` run the query and call `engine.register` in a loop, exactly as `reload_pricing` already does.

**Validation rules:**
- Costs must be finite and non-negative (already enforced by `BillingRecord::new`).
- `max_context_tokens` is stored as `u64` in `ModelPricing` but read as `i64` from Postgres; negative DB values are clamped to `0` (already handled by `unwrap_or(0)`).

**Error types:** No new error types. `AppError::Database` and `AppError::Internal` are sufficient.

### 3.4 API Contract

**No new endpoints. No request/response changes.**

Existing endpoints (already implemented and documented in `crates/zorch-api/src/routes/admin/mod.rs`):
- `GET /api/v1/admin/pricing` — list all rows.
- `POST /api/v1/admin/pricing` — upsert a row; side-effect triggers `reload_pricing`.
- `DELETE /api/v1/admin/pricing/:id` — remove a row; side-effect triggers `reload_pricing`.

**OpenAPI / utoipa annotations:** None required; the project currently does not use `utoipa` macro annotations on admin routes (only a static `docs.rs` endpoint exists). The existing JSON contracts remain unchanged.

**Backward compatibility:**
- Old admin clients continue to work; the request/response shape of `SetPricingRequest` and `PricingResponse` is unchanged.
- Old proxy binaries (pre-this-change) running against the new schema are compatible because the schema is unchanged.
- New binaries running against an empty `provider_model_config` table safely return `(0.0, 0.0)` for all models, which is the same behavior as today for unknown models.

### 3.5 Proxy Pipeline Integration

**Exact files/functions to modify:**
1. `crates/zorch-gateway/src/pricing.rs` — `PricingEngine::new()`
2. `crates/zorch-api/src/server/mod.rs` — `run()` (startup sequence)
3. `crates/zorch-api/src/routes/admin/pricing.rs` — `reload_pricing()` (minor refactor to reuse startup logic)

**Insertion point in request lifecycle:**
- **Startup**, immediately after `sqlx::migrate!()` completes and before `axum::serve()` begins. At this point `db_pool` is live and `provider_model_config` is guaranteed to exist.

**Performance:**
- Adds **one** full-table `SELECT` at boot time. The table is expected to hold hundreds of rows; query latency is sub-millisecond.
- No per-request DB query is added. The engine is read via `arc_swap::ArcSwap<PricingEngine>` (lock-free) on every proxy request and analytics request, which is the existing pattern.
- No Redis caching required; the entire engine fits in a few KiB of RAM.

**Request context extension:** None. The engine is accessed globally through `state.pricing.load()` in:
- `crates/zorch-api/src/routes/v1/proxy/usage.rs` — `record_usage_async`
- `crates/zorch-api/src/routes/admin/analytics.rs` — `fetch_analytics`

**Code sketch for `run()`:**
```rust
let initial_engine = load_pricing_from_db(&db_pool).await.unwrap_or_else(|e| {
    tracing::warn!("Failed to load pricing from DB at startup: {}. Starting with empty engine.", e);
    zorch_gateway::PricingEngine::new()
});
let pricing = Arc::new(arc_swap::ArcSwap::new(Arc::new(initial_engine)));
```

### 3.6 Admin Dashboard Changes

**No changes required.** The feature is backend-only.

- **Page(s) modified or created:** None.
- **Data fetching pattern:** Existing SWR-like `useFetchData` hook in `apps/admin/lib/useFetchData.ts` already fetches `/api/v1/admin/pricing`.
- **Form validation schema:** `ModelPricingRow` in `apps/admin/components/ModelPricingSection.tsx` already validates numeric, non-negative inputs client-side and server-side (`validate_set_pricing`).
- **UX flow:** Admin adds/edits pricing on **Providers** page → clicks **Save** → backend calls `reload_pricing` → in-memory engine updates instantly. This flow already works; after this feature, it will also survive server restarts.

**Optional future enhancement (out of scope):** A "Reload pricing" button on the Pricing audit page that calls `POST /api/v1/admin/pricing` with no-op data or a dedicated reload endpoint. Not needed because every save/delete already triggers hot reload.

### 3.7 Analytics & Observability

**New queries:** None. The existing `fetch_analytics` in `crates/zorch-api/src/routes/admin/analytics.rs` uses `state.pricing.load().calculate_cost(...)` to compute `cost_trends`. Once the engine is DB-backed, these trends automatically reflect live pricing.

**New admin endpoint:** None.

**New Prometheus metric (optional, recommended):**
- Name: `zorch_pricing_engine_entries_total`
- Type: Gauge
- Labels: none
- Value: `engine.prices.len() as i64`
- Exported in the existing `/metrics` handler (already wired in `zorch_telemetry`).
- **Purpose:** allows operators to alert if the engine is unexpectedly empty (e.g., DB connection failure at startup).

### 3.8 Testing Plan

**Unit tests** (`crates/zorch-gateway/src/pricing.rs`):
1. `test_empty_engine` — `PricingEngine::new()` returns empty `prices` map.
2. `test_register_and_calculate` — register a row, then `calculate_cost` matches expected math.
3. `test_unknown_model_returns_zero` — empty engine returns `(0.0, 0.0)` for any provider/model.
4. `test_duplicate_registration_overwrites` — registering two prices for the same key keeps the last one.
5. `test_zero_cost_values` — register `$0` costs; total cost is `0.0`.
6. `test_large_cost_no_panic` — use max safe `f64` values; assert result is finite (no panic).

**Integration tests** (to be added in a new `tests/` directory as part of this PR, laying groundwork for the `add-integration-tests` feature):
1. `test_startup_loads_pricing_from_db` — spin up a test server with a populated `provider_model_config` row; proxy a request and assert `requests_log.total_cost` equals the DB-configured price.
2. `test_empty_db_falls_back_to_zero` — start with empty pricing table; proxy a request; assert `total_cost == 0.0`.

**Migration safety:**
- Seed migration is data-only and idempotent (`ON CONFLICT DO NOTHING`).
- Rollback: revert the code changes; the hardcoded prices are gone, but if the migration has already run, the rows remain in the DB. The old binary cannot read them (it never loaded from DB), so the net effect is that pricing still works because the DB rows are ignored by old code. Reverting the migration is unnecessary for rollback because the schema is unchanged.

**Admin manual checklist (since no E2E suite exists yet):**
1. Seed migration runs cleanly: `sqlx migrate run` reports success.
2. Open admin **Providers** page → expand **Model Pricing** for OpenAI → confirm `gpt-4o` row exists with `$2500.0 / $10000.0`.
3. Edit the input cost to `9999.0`, save, restart server.
4. Send a proxy request for `gpt-4o`.
5. Check **Analytics** → cost trend for today reflects the new price.
6. Delete the `gpt-4o` pricing row, restart server, send another request → cost is `0.0`.

### 3.9 Security & Safety

- **SQL injection:** The startup loader will use `sqlx::query` with **zero** dynamic string interpolation; the query is a static `SELECT` (identical to the one in `reload_pricing`). All user-provided pricing values are bound via SQLx in the admin endpoints.
- **JSONB injection:** Not applicable; no JSONB mutation in this feature.
- **Regex injection:** Not applicable.
- **Secret exposure:** No secrets are read or logged during pricing load.
- **Enumeration / DoS:** Startup performs a single unbounded `SELECT` on a table with expected cardinality < 1,000 rows. No risk.
- **Timezone / DST:** Not applicable; `provider_model_config` has no time-based columns.

### 3.10 Rollback & Compatibility

- **Schema:** No breaking DDL. The new seed migration is forward-compatible with old binaries (old binaries simply ignore the new rows).
- **Old binaries against new schema:** Yes — old binaries never read `provider_model_config`, so extra rows are harmless.
- **New binaries against old schema:** Yes — the table and columns already exist in every schema version that includes migration `20240617000002`.
- **Feature flag:** Not required. The change is deterministic: load from DB at startup. If the load query fails, the server logs a warning and starts with an empty engine (fail-safe). For operators who want the legacy hardcoded fallback even when the DB is empty, a one-line config flag `pricing_fallback_to_hardcoded: bool` could be added in `AppConfig`, but this is discouraged because it reintroduces a dual source of truth.

### 3.11 Out-of-Scope

- **Periodic background refresh** (e.g., polling DB every 60s). Hot-reload on admin mutation is sufficient for the current architecture; background refresh can be added later if needed.
- **Per-organization or per-key pricing overrides.** The scope is global provider/model pricing only.
- **Pricing validation against upstream APIs** (e.g., fetching OpenAI’s official price list).
- **ClickHouse pricing analytics.** This feature only affects the in-memory engine and Postgres source of truth.
- **Admin dashboard UI changes.** The existing pages already support CRUD; no new pages, buttons, or notifications are included.
- **Integration test framework beyond a single smoke test.** Full E2E coverage is tracked as the separate `add-integration-tests` feature.

---

## 4. Sequenced Task List

| # | Task | File(s) | Nature | Depends On | Acceptance Criteria |
|---|------|---------|--------|------------|---------------------|
| 1 | Create seed migration | `migrations/20240622000001_seed_hardcoded_pricing.sql` | New file | — | `sqlx migrate run` succeeds idempotently; `provider_model_config` contains 4 legacy rows when `openai`/`anthropic` providers exist |
| 2 | Empty `PricingEngine::new()` | `crates/zorch-gateway/src/pricing.rs` | Modify function | — | `PricingEngine::new()` produces empty `prices` HashMap; all unit tests in the file still pass after updating expectations |
| 3 | Add startup pricing loader | `crates/zorch-api/src/server/mod.rs` | Modify function | 1, 2 | `run()` loads `provider_model_config` into `ArcSwap` after migrations; engine is non-empty when DB has rows; warning logged on DB error |
| 4 | Refactor `reload_pricing` to reuse loader | `crates/zorch-api/src/routes/admin/pricing.rs` | Modify function | 2 | `reload_pricing` calls the same DB-loading helper as startup (DRY); hot-reload still works after admin save/delete |
| 5 | Add startup-load unit test | `crates/zorch-gateway/src/pricing.rs` | Add test | 2 | `test_empty_engine` and `test_register_and_calculate` pass |
| 6 | Add edge-case unit tests | `crates/zorch-gateway/src/pricing.rs` | Add tests | 2 | Tests for unknown model, duplicate overwrite, zero cost, large finite cost pass |
| 7 | Add integration smoke test | `tests/proxy_pricing_integration.rs` (new) | New file | 3 | Test server boots, loads a pricing row, proxies a mock request, and asserts `requests_log.total_cost > 0` |
| 8 | Add Prometheus gauge | `crates/zorch-gateway/src/pricing.rs` + `crates/zorch-telemetry/src/lib.rs` | Modify / add metric | 2 | `zorch_pricing_engine_entries_total` exposed on `/metrics`; value matches row count after startup |
| 9 | Run full workspace tests | `cargo test --workspace` | Command | 5, 6, 7 | All tests green; no compiler warnings |
| 10 | Admin manual validation | — | Manual checklist | 3, 4 | Edit pricing → restart → verify analytics cost reflects new value; delete row → restart → verify zero cost |

---

## 5. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | **Empty pricing table after deploy** → all costs become `$0` | Medium (new deployments without seed migration run) | High (silent revenue loss) | Seed migration auto-inserts legacy prices; additionally, log a `WARN` at startup if `provider_model_config` is empty so operators are alerted |
| 2 | **DB connection failure at startup** → engine empty until admin mutation | Low | Medium (cost calculations zero until hot-reload) | Startup catches DB errors, logs warning, and continues with empty engine; the server stays alive and admin can still trigger reload later |
| 3 | **Concurrent admin mutation + proxy request** reads stale engine for one request | Low | Very Low (one request uses old price) | `arc_swap::ArcSwap` provides atomic pointer swaps; readers see either the old or new engine, never a partially constructed one |
| 4 | **Seed migration fails because provider row missing** | Medium (if providers table is empty) | Low (migration still succeeds, just inserts nothing) | `ON CONFLICT DO NOTHING` and `SELECT FROM providers` ensure the migration never crashes; empty result is acceptable |
| 5 | **Analyst confusion** after hardcoded removal — “Where did my prices go?” | Low | Low | Update `docs/provider-setup.md` to note that pricing lives in the DB and is seeded automatically; admin dashboard already shows the rows |
