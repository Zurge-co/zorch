# Zorch: Persist Gateway Rejections — Builder-Ready Execution Plan

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

### Existing Plans
- `docs/plans/dynamic-pricing-engine.md` — fully specified plan for boot-time pricing load (high priority, unexecuted).
- `docs/plans/js-sandbox-middleware.md` — fully specified plan for QuickJS sandbox plugin (medium priority, unexecuted).

### Spot-Check Deltas
1. **persist-gateway-rejections** (`next_recommended` #2, high priority):  
   `proxy_handler` in `crates/zorch-api/src/routes/v1/proxy/mod.rs` returns early on middleware block (lines 148-156, 174-182) and on governance pipeline failure (line 165 via `?`) without writing a `BillingRecord`. By contrast, access-window rejections in `crates/zorch-api/src/middleware/auth.rs` (lines 119-136) already persist via `BillingRecord::with_error`. **Gap confirmed.**

2. **dynamic-pricing-engine** (`next_recommended` #1, high priority):  
   `PricingEngine::new()` hardcodes 4 models; server startup never loads `provider_model_config`. **Gap confirmed.** A complete builder-ready plan already exists at `docs/plans/dynamic-pricing-engine.md`.

3. **integration-tests** (`next_recommended` #3, high priority):  
   `glob **/tests/**/*.rs` returned zero files. **Zero integration tests confirmed.**

4. **Regressions:** No `TODO`/`FIXME` markers found in completed feature files.

---

## 2. Candidate Features with Scoring

| Rank | Feature | User/Biz Value | Complexity | Foundation | Risk | Time-to-Value |
|------|---------|----------------|------------|------------|------|---------------|
| 1 | **persist-gateway-rejections** | 5/5 — invisible blocks make analytics useless for debugging; admins cannot tune rate limits or budgets without visibility | 2/5 — `BillingRecord::with_error` exists; need helper + 3 call sites + analytics endpoint | 4/5 — establishes rejection-telemetry pattern for alerting and forecasting | 1/5 — additive inserts only, no schema breakage | 5/5 — one PR, immediate visibility |
| 2 | **dynamic-pricing-engine** | 5/5 — billing accuracy is core; any new model currently costs $0 after restart | 2/5 — schema & UI exist; only need boot-time load + seed migration | 4/5 | 1/5 | 5/5 |
| 3 | **add-integration-tests** | 4/5 — prevents regressions in proxy and admin | 4/5 — needs Postgres+Redis testcontainers, mock upstream | 5/5 | 2/5 | 2/5 |
| 4 | **wire-response-middleware** | 3/5 — enables response transformation / inspection plugins | 3/5 — must buffer streaming & non-streaming responses safely | 4/5 | 3/5 | 3/5 |
| 5 | **clickhouse-analytics-read** | 3/5 — fast analytics at scale, but Postgres works today | 4/5 — schema drift, CH in dev | 3/5 | 3/5 | 2/5 |

### Justification for #1 Choice
**Persist Gateway Rejections** is the highest-ROI *unplanned* next feature because it closes the largest operational blind spot in the platform. Today, when an API key hits its RPM limit, exhausts its budget, or is blocked by a middleware rule, the admin dashboard shows "0 requests" or a mysterious drop in traffic with no explanation. The only rejection that *is* logged is the access-window check in `auth.rs`, which established the exact pattern (`BillingRecord::with_error`) that we will reuse. Unlike `dynamic-pricing-engine` — which is already fully specified in an existing plan and is essentially a one-line startup fix — rejection persistence requires careful coordination across the proxy handler, governance pipeline, analytics queries, and dashboard UI. Implementing it unlocks an entire category of operational workflows: tuning rate limits, identifying abusive keys, forecasting budget burn, and alerting on circuit-breaker trips.

---

## 3. Deep-Dive Technical Plan for #1 — Persist Gateway Rejections

### 3.1 Goal & Definition of Done
When a proxied request is rejected by the gateway — whether by middleware block (`request.pre_governance` or `request.pre_upstream`), rate-limit exhaustion, budget exhaustion, model-allowlist violation, or circuit-breaker open state — a `BillingRecord` is written to `requests_log` with the correct HTTP status code, descriptive error message, zero cost, and the actual elapsed latency. Admins can view these rejections in the **Dashboard → Recent Activity** feed (status = "error") and see a dedicated **Rejection Breakdown** on the **Analytics** page. The existing `access_window` rejection pattern in `auth.rs` continues to work unchanged. Rejection counts are also exposed as a Prometheus counter for alerting.

### 3.2 Schema Changes

One new partial index on `requests_log` to keep rejection analytics queries fast as the table grows.

```sql
-- migrations/20240622000003_add_requests_log_error_index.sql
CREATE INDEX IF NOT EXISTS idx_requests_log_errors
    ON requests_log(created_at DESC, status_code)
    WHERE status_code >= 400;
```

**Justification:**
- `WHERE status_code >= 400` makes this a **partial index**. The vast majority of rows are `200` successes, so the index remains small (~5–10% of table size).
- `created_at DESC` supports time-range filtering (`created_at > NOW() - INTERVAL '24 hours'`).
- `status_code` supports the existing `error_requests` aggregation pattern.
- No table rewrite is required; PostgreSQL creates the index concurrently in the background.

### 3.3 Data Model

No new structs are required. We reuse `zorch_gateway::BillingRecord::with_error`.

**New helper function** (to be added in `crates/zorch-api/src/routes/v1/proxy/mod.rs`, immediately after the `record_request_metrics` function at line 28):

```rust
async fn record_gateway_rejection(
    state: &AppState,
    ctx: &RequestContext,
    request_id_str: &str,
    status_code: i32,
    error_message: &str,
    start: std::time::Instant,
) {
    let latency_ms = start.elapsed().as_millis() as i32;
    let request_id = uuid::Uuid::parse_str(request_id_str)
        .map(zorch_shared::RequestId::from_uuid)
        .unwrap_or_else(|_| zorch_shared::RequestId::new());

    let record = zorch_gateway::BillingRecord::with_error(
        request_id,
        ctx.api_key_id.clone(),
        *ctx.org_id,
        ctx.provider_id.clone(),
        ctx.model_id.clone(),
        0,
        0,
        0.0,
        0.0,
        status_code,
        latency_ms,
        ctx.api_key.tags.clone(),
        Some(error_message.to_string()),
    );

    if let Ok(record) = record {
        if let Err(e) = state.billing.record_request(&state.db_pool, record).await {
            tracing::warn!("Failed to record rejection billing record: {}", e);
        }
    }

    // Also increment the existing HTTP request counter so Prometheus sees the rejection.
    zorch_telemetry::record_http_request("POST", status_code as u16);
}
```

**Validation rules:**
- `status_code` is clamped to valid HTTP ranges by the callers (400, 429, 502). No additional validation needed.
- `error_message` is passed as-is; length is unbounded because Postgres `TEXT` has no limit.
- `latency_ms` is derived from `Instant::now()`, so it is always non-negative.

**Error types:** No new error types. The helper is fire-and-forget: billing-insert failures are logged as `WARN` but never propagated to the caller, ensuring that a DB outage cannot turn a gateway rejection into an internal server error.

### 3.4 API Contract

**New endpoint:**
- `GET /api/v1/admin/analytics/rejections`
- Query params (same pattern as existing analytics):
  - `range` — optional; values `"24h"`, `"7d"`, `"30d"`; default `"24h"`.
  - `tag` — optional; same `"key:value"` filter format used by `GET /api/v1/admin/analytics`.

**Response JSON:**
```json
{
  "summary": {
    "totalRejections": 150,
    "byType": {
      "rateLimit": 80,
      "budget": 20,
      "modelAllowlist": 15,
      "circuitBreaker": 10,
      "middleware": 20,
      "accessWindow": 5
    }
  },
  "trend": [
    {
      "hour": "14:00",
      "count": 12,
      "rateLimit": 8,
      "budget": 1,
      "modelAllowlist": 0,
      "circuitBreaker": 2,
      "middleware": 1,
      "accessWindow": 0
    }
  ]
}
```

**OpenAPI / utoipa annotations:** None required; the project currently does not use `utoipa` macro annotations on admin routes.

**Backward compatibility:**
- Fully additive. Existing `GET /api/v1/admin/analytics` and `GET /api/v1/admin/dashboard` endpoints automatically include rejection rows in `error_requests` counts because they filter on `status_code >= 400`.
- Old admin dashboards that do not call the new endpoint simply do not display the rejection breakdown; the rest of the UI works unchanged.

### 3.5 Proxy Pipeline Integration

**Exact files/functions to modify:**
1. `crates/zorch-api/src/routes/v1/proxy/mod.rs` — add `record_gateway_rejection` helper (after line 30).
2. `crates/zorch-api/src/routes/v1/proxy/mod.rs` — `proxy_handler`, lines 148-156 (middleware pre_governance block).
3. `crates/zorch-api/src/routes/v1/proxy/mod.rs` — `proxy_handler`, lines 174-182 (middleware pre_upstream block).
4. `crates/zorch-api/src/routes/v1/proxy/mod.rs` — `proxy_handler`, line 165 (governance pipeline error).

**Insertion points in request lifecycle:**

1. **Middleware `request.pre_governance` block** (line 148-156):  
   After `Err(e)` is matched and before the `return Err(...)`:
   ```rust
   Err(e) => {
       let msg = format!("Middleware blocked request: {}", e.message);
       record_gateway_rejection(&state, &ctx, &request_id, 400, &msg, start).await;
       return Err(AppError::BadRequest(msg));
   }
   ```

2. **Middleware `request.pre_upstream` block** (line 174-182):  
   Same pattern:
   ```rust
   Err(e) => {
       let msg = format!("Middleware blocked request: {}", e.message);
       record_gateway_rejection(&state, &ctx, &request_id, 400, &msg, start).await;
       return Err(AppError::BadRequest(msg));
   }
   ```

3. **Governance pipeline rejection** (line 165):  
   Replace the bare `?` with an explicit error match:
   ```rust
   if let Err(e) = run_governance_pipeline(
       &state,
       &ctx,
       &axum::body::Bytes::from(modified_body.clone()),
   )
   .await
   {
       let status_code = match &e {
           AppError::RateLimit(_) => 429,
           AppError::BadRequest(_) => 400,
           AppError::Provider(_) => 502,
           _ => 500,
       };
       record_gateway_rejection(&state, &ctx, &request_id, status_code, &e.to_string(), start).await;
       return Err(e);
   }
   ```

**Performance:**
- Adds **one** asynchronous `INSERT` per rejected request. Rejections are orders of magnitude less frequent than successful requests.
- No per-request overhead for successful requests (the helper is only called on error paths).
- The insert is fire-and-forget: if Postgres is unreachable, the rejection is still returned to the client immediately; the failed insert is logged as `WARN`.
- The existing `zorch_telemetry::record_http_request` counter call adds zero latency (in-memory atomic increment).

**Request context extension:** None. The helper consumes the existing `RequestContext` and `AppState`.

### 3.6 Admin Dashboard Changes

**Page modified:** `apps/admin/app/analytics/page.tsx`

**Changes:**
1. Add a new section **"Rejection Breakdown"** below the existing metric cards (after line 177).
2. Import the new `fetchRejectionAnalytics` function from `@/lib/api`.
3. Add a new `useFetchData` hook call:
   ```typescript
   const { data: rejections } = useFetchData<RejectionAnalyticsData>(
     () => fetchRejectionAnalytics(range || undefined)
   );
   ```
4. Render a horizontal bar chart (using `recharts` `BarChart` with `layout="vertical"`) showing `byType` counts:
   - `rateLimit` — color `hsl(var(--destructive))`
   - `budget` — color `orange`
   - `modelAllowlist` — color `purple`
   - `circuitBreaker` — color `gray`
   - `middleware` — color `blue`
   - `accessWindow` — color `green`
5. Render a small summary table (3 columns: Type, Count, % of Total) using the existing `@/components/ui/table` components.

**Data fetching pattern:** Match existing — add `fetchRejectionAnalytics` to `apps/admin/lib/api.ts`:
```typescript
export interface RejectionSummary {
  totalRejections: number;
  byType: Record<string, number>;
}
export interface RejectionTrendPoint {
  hour: string;
  count: number;
  rateLimit: number;
  budget: number;
  modelAllowlist: number;
  circuitBreaker: number;
  middleware: number;
  accessWindow: number;
}
export interface RejectionAnalyticsData {
  summary: RejectionSummary;
  trend: RejectionTrendPoint[];
}

export async function fetchRejectionAnalytics(range?: string): Promise<RejectionAnalyticsData> {
  const params = new URLSearchParams();
  if (range) params.set("range", range);
  const qs = params.toString();
  const path = qs ? `/api/v1/admin/analytics/rejections?${qs}` : "/api/v1/admin/analytics/rejections";
  return fetchObject<RejectionAnalyticsData>(path, {
    fallback: {
      summary: { totalRejections: 0, byType: {} },
      trend: [],
    },
  });
}
```

**Form validation schema:** Not applicable (read-only analytics).

**UX flow step-by-step:**
1. Admin navigates to **Analytics** page.
2. The new **Rejection Breakdown** card loads automatically alongside existing charts.
3. Admin selects a time range (e.g., "Last 7d") from the existing selector.
4. The breakdown updates to show which rejection type dominates (e.g., "Rate Limit: 80").
5. Admin identifies a key that is constantly rate-limited, switches to **API Keys**, and raises its RPM limit.

**Page modified:** `apps/admin/app/dashboard/page.tsx`
6. Enhance `RecentActivity` rendering (lines 138-154) to show the `error_message` in a tooltip on hover when `status === "error"`. This requires adding `errorMessage?: string` to the `RecentActivity` interface in `apps/admin/lib/api.ts` and updating the backend `fetch_recent_activity` query to select `error_message`.

### 3.7 Analytics & Observability

**New PostgreSQL query** (to be added to `crates/zorch-api/src/routes/admin/analytics.rs`):

```sql
SELECT
    CASE
        WHEN error_message LIKE 'outside_allowed_hours%' THEN 'accessWindow'
        WHEN error_message LIKE 'Rate limit exceeded%' THEN 'rateLimit'
        WHEN error_message LIKE 'Budget exceeded%' THEN 'budget'
        WHEN error_message LIKE 'Model %is not in the allowed models list%' THEN 'modelAllowlist'
        WHEN error_message LIKE 'Model not allowed for this API key%' THEN 'modelAllowlist'
        WHEN error_message LIKE 'Provider %is currently unavailable%' THEN 'circuitBreaker'
        WHEN error_message LIKE 'Middleware blocked request%' THEN 'middleware'
        ELSE 'other'
    END AS rejection_type,
    COUNT(*)::bigint AS count
FROM requests_log
WHERE created_at > NOW() - INTERVAL '24 hours'
    AND status_code >= 400
    AND error_message IS NOT NULL
GROUP BY rejection_type
ORDER BY count DESC;
```

**Trend query** (same file, grouped by hour):
```sql
SELECT
    DATE_TRUNC('hour', created_at) AS hour,
    COUNT(*)::bigint AS count,
    COUNT(*) FILTER (WHERE error_message LIKE 'Rate limit exceeded%')::bigint AS rate_limit,
    COUNT(*) FILTER (WHERE error_message LIKE 'Budget exceeded%')::bigint AS budget,
    COUNT(*) FILTER (WHERE error_message LIKE 'Model %is not in the allowed models list%' OR error_message LIKE 'Model not allowed for this API key%')::bigint AS model_allowlist,
    COUNT(*) FILTER (WHERE error_message LIKE 'Provider %is currently unavailable%')::bigint AS circuit_breaker,
    COUNT(*) FILTER (WHERE error_message LIKE 'Middleware blocked request%')::bigint AS middleware,
    COUNT(*) FILTER (WHERE error_message LIKE 'outside_allowed_hours%')::bigint AS access_window
FROM requests_log
WHERE created_at > NOW() - INTERVAL '24 hours'
    AND status_code >= 400
    AND error_message IS NOT NULL
GROUP BY DATE_TRUNC('hour', created_at)
ORDER BY hour;
```

**New admin endpoint:** `GET /api/v1/admin/analytics/rejections` (new file `crates/zorch-api/src/routes/admin/rejections.rs`).

**New Prometheus metric:**
- Name: `zorch_gateway_rejections_total`
- Type: Counter
- Labels: `reason` (`rate_limit`, `budget`, `model_allowlist`, `circuit_breaker`, `middleware`, `access_window`, `other`)
- Exported via existing `/metrics` handler (`zorch_telemetry::metrics_snapshot`).
- Incremented inside `record_gateway_rejection` using:
  ```rust
  metrics::counter!("zorch_gateway_rejections_total", "reason" => reason_label).increment(1);
  ```

**New log line:**
- `INFO` in `record_gateway_rejection`: `recorded gateway rejection request_id=... status=... reason=... latency_ms=...`

### 3.8 Testing Plan

**Unit tests** (`crates/zorch-api/src/routes/v1/proxy/mod.rs` — add a `#[cfg(test)]` mod at the bottom of the file):
1. `test_record_rejection_maps_status_codes` — verify that `AppError::RateLimit` maps to 429, `AppError::BadRequest` to 400, `AppError::Provider` to 502.
2. `test_record_rejection_invalid_request_id_fallback` — pass a malformed `request_id_str`; assert the helper does not panic and falls back to a fresh UUID.
3. `test_record_rejection_long_error_message` — pass a 10,000-character error message; assert the record is created successfully.
4. `test_record_rejection_zero_latency` — pass an `Instant` created just before the call; assert `latency_ms >= 0`.

**Integration tests** (new file `tests/gateway_rejections.rs`):
1. `test_rate_limit_rejection_persisted` — create an API key with `requests_per_minute = 1`; send 2 rapid proxy requests; assert HTTP 429 on the second; query `requests_log` and assert `status_code = 429`, `error_message LIKE 'Rate limit%'`.
2. `test_budget_rejection_persisted` — create a key with `max_spend_usd = 0.01`; seed Redis spend to exceed it; proxy a request; assert HTTP 429 (budget returns `RateLimit` error in current code); assert `requests_log` row exists.
3. `test_model_allowlist_rejection_persisted` — create a key with `allowed_models = ["gpt-4o-mini"]`; proxy with `"gpt-4o"`; assert HTTP 400; assert `requests_log` row with `error_message LIKE 'Model%allowed%'`.
4. `test_middleware_block_persisted` — create a `request_blocker` middleware config for `request.pre_governance` that blocks all requests; proxy any request; assert HTTP 400; assert `requests_log` row with `error_message LIKE 'Middleware blocked%'`.
5. `test_circuit_breaker_rejection_persisted` — force the circuit breaker open for a provider (via repeated upstream failures or direct Redis manipulation if exposed); proxy a request; assert HTTP 502; assert `requests_log` row with `error_message LIKE 'Provider%unavailable%'`.
6. `test_access_window_still_works` — create a key with `allowed_hours_start = 9`, `allowed_hours_end = 18`; proxy at 03:00 UTC; assert HTTP 403; assert `requests_log` row exists (verifying no regression of existing behavior).
7. `test_rejection_analytics_endpoint` — after running tests 1-5, call `GET /api/v1/admin/analytics/rejections?range=24h`; assert `summary.totalRejections >= 5` and `byType.rateLimit > 0`.

**Migration safety:**
- The new partial index is `IF NOT EXISTS`; re-running the migration is a no-op.
- Rollback: `DROP INDEX IF EXISTS idx_requests_log_errors;`. Old binaries ignore the index.

**Admin manual checklist (since no E2E suite exists yet):**
1. Configure an API key with `RPM = 1`.
2. Send 2 proxy requests within 1 minute.
3. Open **Dashboard** → confirm the second request appears in Recent Activity with status "error".
4. Open **Analytics** → confirm the **Rejection Breakdown** shows "rateLimit: 1".
5. Open **Analytics** → confirm the **Error Rate** metric is > 0%.
6. Run `curl http://localhost:8080/metrics | grep zorch_gateway_rejections_total` → counter incremented with `reason="rate_limit"`.
7. Configure a `request_blocker` middleware that blocks all requests; send a proxy request; confirm "middleware" count increments in the breakdown.

### 3.9 Security & Safety

- **SQL injection:** The new analytics query in `rejections.rs` uses **zero** dynamic string interpolation for user input. The `range` parameter is resolved to a static interval string (`"24 hours"`, `"7 days"`, `"30 days"`) via a whitelist function identical to `analytics.rs::resolve_interval`. The `tag` parameter is bound via SQLx as a JSONB value using the existing `build_where_clause` helper. The `CASE` expression is static SQL.
- **Secret exposure:** Rejection records contain `api_key_id` (UUID), `tags` (same JSONB copied from the `api_keys` table), and `error_message` (a gateway-generated string). No raw API key tokens or upstream provider secrets are ever persisted.
- **Enumeration / DoS:** The new `GET /api/v1/admin/analytics/rejections` endpoint returns an aggregate summary (at most 7 rows) plus a time-series (at most 24–720 rows depending on range). It is not a list endpoint and does not expose individual request IDs. The partial index ensures the query remains fast even at millions of rows.
- **JSONB injection:** The `tags` field is copied from `ctx.api_key.tags`, which was already validated when the API key was created (`validate_tags` enforces key/value length and character limits).
- **Timezone / DST:** Not applicable; all timestamps are stored in UTC (`TIMESTAMPTZ`).

### 3.10 Rollback & Compatibility

- **Schema:** The new partial index is backward-compatible. Old binaries can run against the new schema (they do not reference the index).
- **New binaries against old schema:** Yes — the index creation is idempotent (`IF NOT EXISTS`), and the code works without it (query performance degrades gracefully on very large tables).
- **Old binaries against new schema:** Yes — extra indexes do not affect write semantics for old binaries.
- **Feature flag:** Not required. The behavior is purely additive.

### 3.11 Out-of-Scope

- **Real-time alerting on rejection spikes** (e.g., PagerDuty or Slack webhook). The Prometheus counter provides the foundation; alerting rules are operator-side configuration.
- **Per-key rejection throttling or adaptive rate limits.** This feature is observability-only; it does not change when or how rejections occur.
- **ClickHouse rejection analytics.** v1 reads from PostgreSQL only.
- **Streaming response rejection recording.** Gateway-level blocks happen before the upstream request is made, so they apply equally to streaming and non-streaming paths. The existing `UsageCapturingStream` already handles post-upstream failures.
- **Redaction or transformation of error messages before persistence.** Messages are stored as-is.
- **Admin email/Slack notifications for rejections.**
- **Integration test framework beyond the single `tests/gateway_rejections.rs` smoke file.** Full E2E coverage is tracked as the separate `add-integration-tests` feature.

---

## 4. Sequenced Task List

| # | Task | File(s) | Nature | Depends On | Acceptance Criteria |
|---|------|---------|--------|------------|---------------------|
| 1 | Add partial index migration | `migrations/20240622000003_add_requests_log_error_index.sql` | New file | — | `sqlx migrate run` succeeds idempotently; `\d requests_log` shows `idx_requests_log_errors` |
| 2 | Add `record_gateway_rejection` helper | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Add function | — | Compiles; helper is callable from `proxy_handler` without changing its return type |
| 3 | Wire middleware pre_governance rejection recording | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Modify function | 2 | Proxy request blocked by `request.pre_governance` middleware writes `requests_log` row with `status_code=400` |
| 4 | Wire middleware pre_upstream rejection recording | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Modify function | 2 | Proxy request blocked by `request.pre_upstream` middleware writes `requests_log` row with `status_code=400` |
| 5 | Wire governance pipeline rejection recording | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Modify function | 2 | Proxy request blocked by rate limit / budget / model allowlist / circuit breaker writes `requests_log` row with correct status code |
| 6 | Add `record_http_request` call for rejections | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Modify function | 2 | Prometheus `zorch_http_requests_total{status="429"}` increments on rate-limit rejection |
| 7 | Add `zorch_gateway_rejections_total` counter | `crates/zorch-api/src/routes/v1/proxy/mod.rs` + `crates/zorch-telemetry/src/metrics.rs` | Modify / instrument | 2 | Counter exported on `/metrics` with label `reason`; increments on every rejection |
| 8 | Add rejection analytics backend endpoint | `crates/zorch-api/src/routes/admin/rejections.rs` (new) | New file | 1 | `GET /api/v1/admin/analytics/rejections` returns correct JSON shape; `range` and `tag` params work |
| 9 | Register rejection analytics route | `crates/zorch-api/src/routes/admin/mod.rs` | Modify | 8 | `curl /api/v1/admin/analytics/rejections` returns `200 OK` with JSON |
| 10 | Update `RecentActivity` type to include `errorMessage` | `apps/admin/lib/api.ts` | Modify | — | TypeScript interface updated; no runtime change yet |
| 11 | Update backend `fetch_recent_activity` to select `error_message` | `crates/zorch-api/src/routes/admin/dashboard.rs` | Modify | — | Dashboard response includes `errorMessage` for error rows |
| 12 | Add `fetchRejectionAnalytics` to API client | `apps/admin/lib/api.ts` | Add function | — | Compiles; returns fallback on 404 |
| 13 | Add Rejection Breakdown section to Analytics page | `apps/admin/app/analytics/page.tsx` | Modify component | 12 | New card renders with bar chart and summary table; data updates when range changes |
| 14 | Enhance Dashboard Recent Activity with error tooltip | `apps/admin/app/dashboard/page.tsx` | Modify component | 10, 11 | Hovering an "error" row shows the `errorMessage` in a tooltip |
| 15 | Add unit tests for `record_gateway_rejection` | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Add tests | 2 | Tests for status-code mapping, invalid UUID fallback, long message, zero latency pass |
| 16 | Add integration test for rate-limit rejection | `tests/gateway_rejections.rs` (new) | New file | 5 | Test server boots, creates key with RPM=1, sends 2 requests, asserts DB state and HTTP 429 |
| 17 | Add integration test for model-allowlist rejection | `tests/gateway_rejections.rs` | Add tests | 5 | Asserts HTTP 400 and `requests_log` row with correct error message |
| 18 | Add integration test for middleware block rejection | `tests/gateway_rejections.rs` | Add tests | 5 | Asserts HTTP 400 and `requests_log` row after `request_blocker` triggers |
| 19 | Add integration test for rejection analytics endpoint | `tests/gateway_rejections.rs` | Add tests | 8, 16 | After inducing rejections, asserts `/api/v1/admin/analytics/rejections` returns non-zero counts |
| 20 | Run full workspace tests | `cargo test --workspace` | Command | 15, 16, 17, 18, 19 | All tests green; no compiler warnings |

---

## 5. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | **DB connection failure during rejection recording** → client gets internal error instead of rejection | Low | Medium (operator confusion, client sees 500 instead of 429/400) | Helper is fire-and-forget: DB errors are logged as `WARN` but never propagated. The original `Err` is always returned to the client. |
| 2 | **Analytics query slow on massive `requests_log` table** without partial index | Low | Medium (dashboard timeout) | Migration adds a partial index (`status_code >= 400`) which is tiny. Query planner will use it automatically. |
| 3 | **Error message string changes** → `CASE` expressions miscategorize rejections | Medium (if governance messages are refactored) | Low (analytics miscategorization) | The `CASE` patterns use stable prefixes (`"Rate limit exceeded:"`, `"Budget exceeded:"`, `"Middleware blocked request:"`) that are defined in the same codebase. Any refactor of these strings must include a corresponding update to the analytics query. A comment in `proxy/mod.rs` will flag this coupling. |
| 4 | **Concurrent admin mutation of `requests_log` schema** during deploy | Low | Low (brief lock on index creation) | The migration uses `CREATE INDEX IF NOT EXISTS` without `CONCURRENTLY`. For a high-traffic deploy, the DBA can run the index creation manually with `CONCURRENTLY` before code deploy. |
| 5 | **Old admin dashboard calls new endpoint and gets 404** | Low | Low (missing rejection chart) | The frontend uses `fetchObject` with a fallback, so a 404 renders empty data rather than crashing. |
| 6 | **Prometheus metric cardinality explosion** if `reason` label is unbounded | Very Low | Medium (memory growth in metrics exporter) | The `reason` label is drawn from a closed set of 6 hardcoded strings inside `record_gateway_rejection`. No user input ever reaches the label. |
