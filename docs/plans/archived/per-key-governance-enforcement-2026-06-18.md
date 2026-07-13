# Zorch Builder Plan: Per-Key Governance Enforcement

> Plan generated: 2026-06-18  
> Based on canonical state: `docs/project_state.yaml` (last updated 2024-06-18, commit `7edb0ab`)

---

## 1. State of the Union

### 1.1 Canonical State (from `docs/project_state.yaml`)

| Feature | Status | Notes |
|---------|--------|-------|
| `proxy::provider-proxy` | complete | Pass-through HTTP proxy to OpenAI/Anthropic upstreams |
| `proxy::multi-key-rotation` | complete | AES-256-GCM at-rest with fallback |
| `proxy::streaming-support` | complete | SSE usage parsing for per-request billing |
| `proxy::retry-logic` | complete | Exponential-backoff with jitter |
| `proxy::middleware-engine` | complete | DB-driven with priority/scope/failure modes |
| `proxy::middleware-request-phases` | complete | `request.pre_governance` and `request.pre_upstream` |
| `proxy::middleware-response-phases` | missing | Types and DB exist; never invoked in proxy handler |
| `proxy::circuit-breaker` | complete | In-memory per provider |
| `proxy::access-windows` | complete | Time-of-day with IANA timezone support |
| `proxy::per-key-rate-limit` | partial | Redis sliding windows exist; proxy uses hardcoded defaults |
| `proxy::per-key-budget` | partial | DB column exists; proxy uses hardcoded $100 |
| `proxy::per-key-model-allowlist` | partial | DB column exists; proxy uses empty allowlist |
| `admin_dashboard::api-key-crud` | complete | Tags, access windows, expiration, scopes |
| `admin_dashboard::real-time-updates` | missing | No WebSocket/SSE |
| `analytics::clickhouse-inspector` | partial | Capture exists; admin never queries ClickHouse |
| `analytics::prometheus-metrics` | complete | `/metrics` endpoint |
| `security::key-encryption` | complete | AES-256-GCM |
| `security::client-key-hashing` | complete | SHA-256 |

**Next recommended (from YAML):**
1. `per-key-governance-enforcement` — **critical**
2. `dynamic-pricing-engine` — high
3. `persist-gateway-rejections` — high
4. `add-integration-tests` — high
5. `wire-response-middleware` — medium
6. `clickhouse-analytics-read` — medium

### 1.2 Spot-Check Deltas

**Delta A: Confirmed hardcoded governance in proxy pipeline**
- File: `crates/zorch-api/src/routes/v1/proxy/governance.rs:27`
- Code: `&KeyLimitConfig::default()`
- `KeyLimitConfig::default()` is hardcoded to `100 RPM / 10_000 RPD / $100.0 / empty allowlist` in `crates/zorch-gateway/src/key_limits.rs:275-284`.
- **Impact:** Every proxied request ignores the DB-stored `requests_per_minute`, `requests_per_day`, `max_spend_usd`, and `allowed_models` values.

**Delta B: Confirmed hardcoded pricing on cold start**
- File: `crates/zorch-gateway/src/pricing.rs:30-71`
- `PricingEngine::new()` registers exactly 4 hardcoded models (gpt-4o, gpt-4o-mini, claude-3-5-sonnet, claude-3-opus).
- File: `crates/zorch-api/src/server/mod.rs:123` initializes with `PricingEngine::new()`.
- Admin hot-reload (`reload_pricing`) works on mutation, but cold-start and empty-DB scenarios serve only hardcoded rows.

**Delta C: Zero integration / E2E tests**
- No `tests/` directories. No `#[tokio::test]` integration tests outside unit `#[cfg(test)]` blocks.
- This matches YAML claim exactly.

**Delta D: Admin API does not expose governance fields**
- `crates/zorch-api/src/routes/admin/api_keys.rs:338-353` — `fetch_api_keys_with_filter` does **not** `SELECT requests_per_minute, requests_per_day, max_spend_usd, allowed_models`.
- `crates/zorch-api/src/routes/admin/types.rs:44-55` — `ApiKeyResponse` lacks these fields.
- `apps/admin/lib/api.ts:107-118` — frontend `ApiKey` interface lacks them.
- **Implication:** Even if the proxy were wired, admins could not view or edit the limits through the dashboard.

**Delta E: `GovernanceEngine` performs redundant DB query**
- `crates/zorch-gateway/src/governance.rs:32-42` re-queries `api_keys` for every proxied request to read `allowed_models` and `max_spend_usd`.
- The auth middleware (`crates/zorch-api/src/middleware/auth.rs:88-99`) already fetches the full row (including all 4 governance columns) for every request.
- **Implication:** 1 extra DB query per proxy request that can be eliminated.

---

## 2. Candidate Feature Ranking

### 2.1 #1 — Per-Key Governance Enforcement

**User/Business Value:** 10/10  
Customers set RPM, RPD, budget, and model allowlists in the DB, but the proxy silently ignores them. This is a broken contract — the most critical credibility gap in the product.

**Implementation Complexity:** 4/10  
Schema already exists. Auth middleware already fetches the row. Changes are localized to: (a) extending admin API DTOs, (b) adding form fields in Next.js, (c) passing the pre-fetched `ApiKey` through request extensions to the proxy pipeline, and (d) constructing `KeyLimitConfig` from DB values instead of `Default`. No new services, no new infra.

**Foundation for Future Features:** 9/10  
Unblocks `persist-gateway-rejections` (we need to know *why* a request was blocked to log it). Provides the governance hook needed for `wire-response-middleware` (response-phase plugins may want to know the key’s budget state). Also establishes the pattern for passing key metadata through the proxy lifecycle.

**Risk:** 2/10  
Purely additive; no schema deletion. The only runtime risk is that a key with `NULL` values suddenly gets different limits. Mitigation: preserve current hardcoded values as explicit fallbacks.

**Time-to-Value:** 10/10  
Can ship incrementally: backend proxy wiring first (immediate value), admin dashboard second (discoverability), analytics third (observability). Each sub-ship is independently testable.

**Score:** **Winner** — highest value, lowest risk, fastest incremental delivery.

---

### 2.2 #2 — Dynamic Pricing Engine

**User/Business Value:** 7/10  
Admins can add pricing rows, but cold-start and new-model onboarding require code changes. Limits provider agility.

**Implementation Complexity:** 3/10  
Single change: load `provider_model_config` from DB at startup instead of hardcoding. `reload_pricing` logic already works.

**Foundation for Future Features:** 6/10  
Required for any multi-tenant or white-label scenario where models are not known at compile time.

**Risk:** 2/10  
Additive; if DB load fails, fall back to hardcoded defaults.

**Time-to-Value:** 9/10  
One backend change, no frontend work.

---

### 2.3 #3 — Persist Gateway Rejections

**User/Business Value:** 8/10  
Rate-limit and budget blocks are invisible in analytics. Admins cannot debug why traffic dropped.

**Implementation Complexity:** 5/10  
Requires writing `BillingRecord::with_error` for every rejection path in the pipeline (rate limit, budget, model allowlist, circuit breaker, governance block). Needs a new `requests_log` index on `status_code >= 400` for performant analytics queries.

**Foundation for Future Features:** 7/10  
Essential for accurate billing dashboards and audit trails.

**Risk:** 3/10  
Must ensure rejected requests do not double-count spend or trigger side effects (e.g., do not call `record_spend` for blocked requests).

**Time-to-Value:** 7/10  
Requires touching every rejection point in the pipeline; higher regression risk than #1.

---

### 2.4 #4 — Add Integration Tests

**User/Business Value:** 5/10  
No direct user value, but prevents regressions in proxy governance and pricing.

**Implementation Complexity:** 6/10  
Requires standing up Postgres + Redis + mock upstream server in tests. No existing test harness.

**Foundation for Future Features:** 9/10  
Enables confident refactoring of the proxy pipeline, which is currently untested at the HTTP layer.

**Risk:** 1/10  
Purely additive test code.

**Time-to-Value:** 4/10  
High setup cost before first test passes; best done after #1 stabilizes the pipeline contract.

---

### 2.5 #5 — Wire Response Middleware

**User/Business Value:** 4/10  
Enables response transformation and inspector capture plugins. No users are currently asking for this because the phases are not advertised.

**Implementation Complexity:** 5/10  
Requires refactoring the proxy handler to buffer or tee the upstream response so that `response.pre_client` and `inspector.pre_capture` can run. Streaming paths are tricky.

**Foundation for Future Features:** 8/10  
Required for response logging, PII redaction on egress, and advanced analytics.

**Risk:** 5/10  
Streaming response buffering can introduce latency and memory pressure.

**Time-to-Value:** 5/10  
Needs careful design; not an incremental afternoon fix.

---

## 3. Deep-Dive Technical Plan: Per-Key Governance Enforcement

### 3.1 Goal & Definition of Done

An admin creates or edits an API key and sets **Requests Per Minute (RPM)**, **Requests Per Day (RPD)**, **Daily Budget (USD)**, and **Allowed Models**. The proxy pipeline reads these values from the database (via the already-fetched `ApiKey` row) and enforces them for every inbound request. A request that exceeds RPM is rejected with HTTP 429; a request that exceeds budget is rejected with HTTP 429; a request for a non-allowed model is rejected with HTTP 400. The admin dashboard displays the current limits for each key and allows editing them. All existing keys with `NULL` values retain behavior identical to today (100 RPM / 10k RPD / $100 / no model restriction).

### 3.2 Schema Changes

**No new migration required.** Columns were added in `migrations/20240613000007_add_api_key_governance_columns.sql`:

```sql
ALTER TABLE api_keys
    ADD COLUMN IF NOT EXISTS allowed_models TEXT[] DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS max_spend_usd DOUBLE PRECISION DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS requests_per_minute INTEGER DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS requests_per_day INTEGER DEFAULT NULL;
```

**Justification for existing types:**
- `INTEGER` for RPM/RPD: sufficient range (2.1B max); no key needs >2B RPM.
- `DOUBLE PRECISION` for budget: matches the existing cost calculation pipeline (`f64` everywhere).
- `TEXT[]` for models: matches the existing `Provider::models` representation and is easy to validate in Rust as `Vec<String>`.
- All are `NULL`able: preserves backward compatibility and matches the semantic that `NULL` means "use default / unlimited."
- `api_keys.id` is already the primary key; no additional index is needed for the proxy lookups because the auth middleware queries `WHERE key_hash = $1` (which should ideally have an index on `key_hash`, but that is out of scope and already exists).

**No new indexes.** The proxy pipeline will read the values from the `ApiKey` struct that the auth middleware already loaded via `key_hash` lookup.

### 3.3 Data Model

**Crate:** `crates/zorch-db/src/models.rs` (already exists; no new structs needed)

The `ApiKey` struct already contains the fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub requests_per_minute: Option<i32>,   // existing
    pub requests_per_day: Option<i32>,      // existing
    pub max_spend_usd: Option<f64>,          // existing
    pub allowed_models: Option<Vec<String>>, // existing
    pub tags: serde_json::Value,
    pub allowed_hours_start: Option<i16>,
    pub allowed_hours_end: Option<i16>,
    pub window_timezone: Option<String>,
}
```

**Validation rules** (to be added in admin API and frontend):
- `requests_per_minute`: `Some(v)` requires `v > 0` and `v <= 1_000_000`.
- `requests_per_day`: `Some(v)` requires `v > 0` and `v <= 10_000_000`.
- `max_spend_usd`: `Some(v)` requires `v >= 0.0` and `v <= 1_000_000.0`.
- `allowed_models`: `Some(v)` requires each model string is non-empty and `len() <= 128`; max array length `128`.

**Error type:** Reuse `zorch_shared::AppError::Validation(String)` for invalid input.

**Crate:** `crates/zorch-gateway/src/key_limits.rs`  
Add a constructor on `KeyLimitConfig`:

```rust
impl KeyLimitConfig {
    pub fn from_api_key(api_key: &zorch_db::ApiKey) -> Self {
        Self {
            requests_per_minute: api_key.requests_per_minute.map(|v| v as u64).unwrap_or(100),
            requests_per_day: api_key.requests_per_day.map(|v| v as u64).unwrap_or(10_000),
            max_spend_usd: api_key.max_spend_usd.unwrap_or(f64::MAX),
            allowed_models: api_key.allowed_models.clone().unwrap_or_default(),
        }
    }
}
```

*Rationale for defaults:*
- `100` and `10_000` preserve the current hardcoded proxy behavior so no existing key changes behavior.
- `f64::MAX` for NULL budget means "unlimited" (the `KeyLimits` budget check `current_spend >= config.max_spend_usd` will never fire). This matches the documented `NULL = unlimited` semantic.
- Empty `allowed_models` means "all models allowed" (existing `KeyLimits` logic).

### 3.4 API Contract

#### 3.4.1 Modified Admin Endpoints

**`GET /api/v1/admin/api-keys`**  
Response body (`ApiKeysResponse`) now includes governance fields in each `ApiKeyResponse`:

```json
{
  "keys": [
    {
      "id": "uuid",
      "name": "prod-key",
      "key": "sk-zorch-abc...",
      "status": "active",
      "createdAt": "2024-06-18",
      "usage": "0 tokens",
      "tags": [],
      "allowedHoursStart": null,
      "allowedHoursEnd": null,
      "windowTimezone": null,
      "requestsPerMinute": 120,
      "requestsPerDay": 5000,
      "maxSpendUsd": 50.0,
      "allowedModels": ["gpt-4o", "claude-3-5-sonnet"]
    }
  ]
}
```

- `requestsPerMinute`: `number | null`
- `requestsPerDay`: `number | null`
- `maxSpendUsd`: `number | null`
- `allowedModels`: `string[] | null`

**`POST /api/v1/admin/api-keys`**  
Request body now accepts optional governance fields:

```json
{
  "name": "prod-key",
  "scopes": ["default"],
  "expiresInDays": 30,
  "tags": [],
  "allowedHoursStart": 9,
  "allowedHoursEnd": 18,
  "windowTimezone": "America/New_York",
  "requestsPerMinute": 120,
  "requestsPerDay": 5000,
  "maxSpendUsd": 50.0,
  "allowedModels": ["gpt-4o"]
}
```

All four fields are optional (`null` or omitted = no limit / use default).

**`PUT /api/v1/admin/api-keys/{id}`**  
Request body (`UpdateApiKeyRequest`) now accepts:

```json
{
  "name": "prod-key-renamed",
  "requestsPerMinute": 200,
  "requestsPerDay": null,
  "maxSpendUsd": null,
  "allowedModels": ["gpt-4o-mini"]
}
```

Fields support the same `Option<Option<T>>` pattern used by `allowed_hours_start` (i.e., `null` in JSON means "do not update", while explicit `null` wrapped in an option is tricky — we should use the same pattern as existing access-window fields: `Option<Option<T>>` where outer `Some` means "update this field", inner `Some(v)` sets it, inner `None` clears it to NULL).

**Backward compatibility:** Old admin clients that do not send the new fields will simply not update them (outer `None`). Existing keys with `NULL` columns continue to work.

#### 3.4.2 OpenAPI / utoipa Annotations

If utoipa macros exist on the admin handlers (they were not present in the read files; the project appears to use manual OpenAPI docs or no docs), add `#[serde(default)]` and `#[schema(nullable = true)]` annotations to the new fields in request/response structs. If no utoipa is wired, this section is a no-op.

### 3.5 Proxy Pipeline Integration

#### 3.5.1 Exact File/Function to Modify

1. **`crates/zorch-api/src/middleware/auth.rs`**  
   Function: `middleware` (lines 88-146 for proxy path)
   
   After validating the API key and before calling `next.run(req)`, insert the full `ApiKey` into request extensions:
   ```rust
   req.extensions_mut().insert(api_key);
   ```
   This makes the already-fetched row available downstream without an extra DB query.

2. **`crates/zorch-api/src/routes/v1/proxy/mod.rs`**  
   Function: `proxy_handler` (lines 49-257)
   
   After extracting `api_key_id` and `org_id` (lines 92-103), also extract the `ApiKey`:
   ```rust
   let api_key = parts
       .extensions
       .get::<zorch_db::ApiKey>()
       .cloned()
       .ok_or_else(|| AppError::Auth("API key metadata missing".to_string()))?;
   ```
   
   Pass `api_key` into `RequestContext` (extend the struct) or pass it separately to `run_governance_pipeline`.

3. **`crates/zorch-api/src/routes/v1/proxy/governance.rs`**  
   Function: `run_governance_pipeline` (lines 8-31)
   
   Signature change:
   ```rust
   pub async fn run_governance_pipeline(
       state: &AppState,
       ctx: &RequestContext,
       api_key: &zorch_db::ApiKey,
       _body: &Bytes,
   ) -> Result<(), AppError>
   ```
   
   Body change:
   ```rust
   let key_config = zorch_gateway::KeyLimitConfig::from_api_key(api_key);
   // ... rest unchanged, but pass &key_config instead of &KeyLimitConfig::default()
   ```

4. **`crates/zorch-gateway/src/governance.rs`**  
   Function: `check_request` (lines 25-115)
   
   Refactor to accept an optional `&zorch_db::ApiKey` to avoid the redundant DB query:
   ```rust
   pub async fn check_request(
       &self,
       api_key_id: ApiKeyId,
       _provider: &ProviderId,
       model: &ModelId,
       _estimated_tokens: u32,
       api_key: Option<&zorch_db::ApiKey>,  // NEW
   ) -> Result<GovernanceDecision, AppError>
   ```
   
   If `api_key` is `Some`, read `is_active`, `expires_at`, `allowed_models`, `max_spend_usd` directly from it. If `None`, fall back to the existing DB query (preserves any direct callers).

5. **`crates/zorch-gateway/src/pipeline.rs`**  
   Function: `execute` (lines 44-107)
   
   Extend signature to accept `api_key: &zorch_db::ApiKey` and pass it to `governance.check_request`.
   Update all call sites in tests to compile.

#### 3.5.2 Request Lifecycle Insertion Point

The governance pipeline runs after middleware `request.pre_governance` and before `request.pre_upstream`. The new logic does not change the lifecycle position; it only changes the **data source** for `KeyLimitConfig` from `Default` (hardcoded) to the DB-backed `ApiKey` extension.

#### 3.5.3 Performance Impact

- **No additional DB query.** The auth middleware already queries `api_keys` by `key_hash`. We reuse that row.
- **No additional Redis query.** `KeyLimits` already queries Redis for sliding windows and spend counters. The number of Redis round-trips is unchanged.
- **No cache needed.** The data is already in memory (fetched by auth middleware) and is key-specific; Redis or in-memory caching would add complexity without benefit.
- **Request context extension** adds one `Arc<ApiKey>` clone (or the struct itself if it’s `Clone`, which it is). Negligible overhead.

### 3.6 Admin Dashboard Changes

#### 3.6.1 Pages Modified

- **`apps/admin/app/api-keys/page.tsx`** — Create dialog and Edit dialog both gain a "Governance Limits" section.

#### 3.6.2 Data Fetching Pattern

The admin app uses raw `fetch` wrapped in `useFetchData` (SWR-like manual hook, no tRPC). We match the existing pattern exactly.

#### 3.6.3 Form Validation Schema

Add client-side validation to the create/edit handlers before calling `createApiKey` / `updateApiKey`:

```typescript
function validateGovernance(values: {
  requestsPerMinute?: number | null;
  requestsPerDay?: number | null;
  maxSpendUsd?: number | null;
  allowedModels?: string[] | null;
}): string | null {
  if (values.requestsPerMinute != null && (values.requestsPerMinute < 1 || values.requestsPerMinute > 1_000_000)) {
    return "RPM must be between 1 and 1,000,000";
  }
  if (values.requestsPerDay != null && (values.requestsPerDay < 1 || values.requestsPerDay > 10_000_000)) {
    return "RPD must be between 1 and 10,000,000";
  }
  if (values.maxSpendUsd != null && (values.maxSpendUsd < 0 || values.maxSpendUsd > 1_000_000)) {
    return "Budget must be between $0 and $1,000,000";
  }
  if (values.allowedModels != null) {
    if (values.allowedModels.length > 128) return "Max 128 allowed models";
    for (const m of values.allowedModels) {
      if (m.length === 0 || m.length > 128) return "Each model must be 1-128 characters";
    }
  }
  return null;
}
```

#### 3.6.4 UX Flow (step-by-step)

1. **Create Dialog:**
   - Admin clicks "Create New Key".
   - Dialog opens with existing fields (name, scopes, expires, tags, access window).
   - New collapsible section "Governance Limits (Optional)" appears below "Allowed Hours".
   - Fields: RPM (number input), RPD (number input), Budget (number input with `$` prefix), Allowed Models (multi-tag input, comma-separated or chip-based).
   - Helper text: "Leave blank to use default limits."
   - Admin clicks "Create Key". Client validates, then POSTs to backend.

2. **Edit Dialog:**
   - Admin clicks pencil icon on a key row.
   - Dialog opens with current values.
   - Governance section shows current values or "Using defaults" if NULL.
   - Admin can clear individual fields to revert to default.
   - Click "Save Changes" → PUT to backend.

3. **Table Row:**
   - Add a new column "Limits" between "Window" and "Usage".
   - Display a compact summary: e.g., "120 RPM / 5k RPD / $50" or "Default" if all NULL.
   - If `allowedModels` is non-empty, show a chip count badge (e.g., "3 models").

### 3.7 Analytics & Observability

#### 3.7.1 New Queries

No new ClickHouse queries (ClickHouse integration is partial per YAML). We add one PostgreSQL query for an admin endpoint to show "blocked requests by reason" over the last 24 hours.

**New admin endpoint:** `GET /api/v1/admin/analytics/rejections`

```rust
pub async fn get_rejection_analytics(
    State(state): State<AppState>,
) -> Result<Json<RejectionAnalyticsResponse>, AppError> {
    let rows = sqlx::query_as::<_, RejectionSummary>(
        r#"
        SELECT
            error_message,
            COUNT(*) as count,
            MAX(created_at) as last_at
        FROM requests_log
        WHERE status_code IN (429, 400, 403)
          AND created_at >= NOW() - INTERVAL '24 hours'
          AND error_message IS NOT NULL
        GROUP BY error_message
        ORDER BY count DESC
        LIMIT 50
        "#
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch rejections: {}", e)))?;

    Ok(Json(RejectionAnalyticsResponse { rejections: rows }))
}
```

**Why:** This enables admins to verify that governance enforcement is actually working and see which keys are hitting limits.

**Response DTO (`RejectionSummary`):**
```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectionSummary {
    pub error_message: String,
    pub count: i64,
    pub last_at: String,
}
```

#### 3.7.2 Prometheus Metrics

Add a counter in `crates/zorch-gateway/src/pipeline.rs` for each rejection reason:

```rust
zorch_telemetry::record_governance_rejection("rate_limit_rpm");
zorch_telemetry::record_governance_rejection("rate_limit_rpd");
zorch_telemetry::record_governance_rejection("budget_exceeded");
zorch_telemetry::record_governance_rejection("model_not_allowed");
zorch_telemetry::record_governance_rejection("circuit_breaker_open");
```

If `zorch_telemetry` does not yet expose a helper for custom labels, add one:

```rust
// crates/zorch-telemetry/src/metrics.rs
pub fn record_governance_rejection(reason: &str) {
    // implementation using prometheus crate counter_vec
}
```

*Assumption:* The project uses the `prometheus` crate. If it uses `metrics` crate instead, the helper signature stays the same but implementation changes.

### 3.8 Testing Plan

#### 3.8.1 Unit Tests

**`crates/zorch-gateway/src/key_limits.rs`**
- `test_key_limit_config_from_api_key_all_null` → verifies defaults (100, 10000, f64::MAX, empty).
- `test_key_limit_config_from_api_key_with_values` → verifies exact mapping.
- `test_key_limit_config_from_api_key_zero_budget` → verifies 0.0 budget (should block all requests).
- `test_key_limit_config_from_api_key_empty_models` → verifies empty vec means all allowed.
- Existing Redis tests remain unchanged.

**`crates/zorch-gateway/src/governance.rs`**
- `test_governance_uses_prefetched_api_key` → pass `Some(api_key)` with `is_active=false`, assert `Block` without DB query.
- `test_governance_fallback_to_db_query` → pass `None`, assert it still queries and works.

**`crates/zorch-api/src/routes/admin/api_keys.rs`**
- `test_build_update_sql_with_governance_fields` → verify `requests_per_minute = $N` appears in generated SQL.
- `test_validate_governance_bounds` → verify `requests_per_minute: -1` returns `Validation` error.

**`crates/zorch-api/src/middleware/auth.rs`**
- `test_api_key_inserted_into_extensions` → mock request with valid key, assert extension contains `ApiKey`.

#### 3.8.2 Integration Tests

Since the project currently has **zero** integration tests, we create a minimal new file:

**`crates/zorch-api/tests/proxy_governance.rs`** (new file)

Test harness requirements (documented for builder, not necessarily fully implemented in this feature):
- Spin up a Postgres test container (or use `sqlx::test` with `TEST_DATABASE_URL`).
- Spin up a Redis test container (or use a mock Redis). For the first integration test, we can use a real Redis instance on a test port (6379 with db=15), or we can add a lightweight in-memory mock. **Assumption:** We will use `redis` crate pointing to `redis://localhost:6379/15` in CI.
- Stand up the Axum app with `AppState` using test pools.
- Use `reqwest` or `axum::TestClient` to send requests.

**Integration test cases:**
1. `test_proxy_allows_request_under_rpm_limit` — create key with RPM=5, send 4 requests, all return 200.
2. `test_proxy_blocks_request_over_rpm_limit` — create key with RPM=2, send 3rd request, assert 429, assert response body contains "Rate limit exceeded: 2 requests per minute".
3. `test_proxy_blocks_request_over_budget` — create key with budget=$0.01, send one request that costs >$0.01 (mock upstream response with usage), assert 429.
4. `test_proxy_blocks_disallowed_model` — create key with `allowed_models=["gpt-4o"]`, request `"claude-3-5-sonnet"`, assert 400, assert body contains "Model 'claude-3-5-sonnet' is not in the allowed models list".
5. `test_proxy_allows_null_limits` — create key with all governance NULL, send 101st request, assert 200 (because fallback is 100 RPM and we only sent 101 — wait, 101 > 100, so it would block. To test unlimited, we'd need to set explicit high values or clear Redis. Better: set explicit RPM=10_000 and send 5 requests).
6. `test_admin_api_crud_governance_fields` — POST key with RPM=60, GET list asserts RPM=60, PUT to RPM=null, GET asserts null.

**Note:** Because Redis state persists across tests, each test must call `KeyLimits::reset(api_key_id)` in setup/teardown.

#### 3.8.3 Migration Safety

- No new migration is added, so rollback is not applicable for schema.
- If the feature is reverted (code rollback), the proxy simply reverts to `KeyLimitConfig::default()`. The DB columns remain unused but harmless.

#### 3.8.4 Admin Manual Checklist

If integration tests cannot be run (e.g., no Redis in CI yet), verify manually:
1. Create a key with RPM=2 in dashboard.
2. Send 3 rapid `curl` requests to `/v1/chat/completions`.
3. Verify 1st and 2nd return 200; 3rd returns 429.
4. Verify `requests_log` contains the blocked request with `status_code=429` and `error_message` set.
5. Edit key to budget=$0.01; send a request; verify 429 if cost > $0.01.
6. Edit key to allowed_models=["gpt-4o-mini"]; request "gpt-4o"; verify 400.

### 3.9 Security & Safety

#### 3.9.1 Injection Risks

- **SQL:** The admin `fetch_api_keys_with_filter` uses string interpolation for the `filter` clause (`format!("... {} ", filter)`). This is existing tech debt. The new SELECT columns are hardcoded strings, not user input, so no new SQL injection risk is introduced.
- **JSONB:** `tags` column is already JSONB. The new governance fields are scalar/array; no JSONB injection.
- **Regex:** No regex used for validation. Model IDs are validated as plain strings (length + non-empty).

#### 3.9.2 Secret Exposure Risks

- The `ApiKey` struct contains `key_hash` (SHA-256, not the plaintext key). Inserting it into request extensions does not expose secrets; extensions are internal to the request lifecycle and are not serialized to the client.
- However, care must be taken that `ApiKey` is never accidentally logged in `tracing::debug!` or similar. Audit: search for `debug!(?api_key` or `info!(?api_key` after changes.

#### 3.9.3 Enumeration / DoS Risks

- **Budget enumeration:** An attacker with a valid key could probe the exact budget by sending requests and observing 429 vs 200. This is inherent to any budget-based system. Mitigation: return generic "Rate limit exceeded" without exposing the exact limit or current spend in the HTTP response (already the case in `KeyLimits`).
- **Model enumeration:** The 400 error message currently includes the model name: `"Model 'X' is not in the allowed models list"`. This leaks the fact that the key has an allowlist. **Mitigation:** Change error to `"Model not allowed for this API key"` to avoid leaking the allowed list contents. This is a one-line change in `key_limits.rs:105-108`.
- **DoS via Redis:** `KeyLimits` creates Redis keys `key_rpm:{id}`, `key_rpd:{id}`, `key_spend:{id}`. A malicious client could create many keys (but key creation requires admin auth). Existing keys are bounded by the application. No new DoS vector.

#### 3.9.4 Timezone / DST Edge Cases

- Not applicable to this feature. RPM/RPD use Unix epoch seconds (UTC). Budget is daily in UTC (Redis TTL = 86400 seconds). No IANA timezone math involved.
- **Caveat:** The daily budget resets every 86400 seconds from the first spend, not at midnight UTC. This is existing behavior in `record_spend`. Out of scope to fix.

### 3.10 Rollback & Compatibility

#### 3.10.1 Schema Compatibility

- No migration is added. The existing schema from `20240613000007_add_api_key_governance_columns.sql` is backward-compatible with any code version.
- Old binaries can run against the schema (they simply ignore the columns).
- New binaries can run against the schema (they read the columns; NULL yields fallback defaults).

#### 3.10.2 Code Compatibility

- If rolled back, the proxy reverts to `KeyLimitConfig::default()`. Behavior is identical to pre-feature state.
- The admin dashboard, if rolled back independently, may display stale data if the backend API no longer returns governance fields. To prevent this, the frontend should gracefully handle missing fields (use `?.` or default to `null`). The existing `ApiKey` interface addition is safe because the backend is the source of truth.

#### 3.10.3 Feature Flag

Not required. The change is purely additive and backward-compatible. However, if the team wants an emergency kill-switch, we can add an `AppConfig` flag:

```rust
// crates/zorch-shared/src/config.rs
pub struct AppConfig {
    // ... existing fields ...
    pub enforce_per_key_governance: bool, // default true
}
```

And gate the `from_api_key` usage:
```rust
let key_config = if cfg.enforce_per_key_governance {
    KeyLimitConfig::from_api_key(api_key)
} else {
    KeyLimitConfig::default()
};
```

**Recommendation:** Include this flag. It costs one config line and provides operational safety.

### 3.11 Out-of-Scope

Explicitly **not** included in this feature:
1. **Global default configuration** — The fallback values (100 RPM / 10k RPD) remain hardcoded. A future feature can add `AppConfig` defaults or a `global_limits` DB table.
2. **Atomic budget check-and-increment** — Concurrent requests can still overshoot the budget by a small margin because `check_limits` reads spend and `record_spend` increments it later. Fixing this requires Lua scripting in Redis or a DB transaction; out of scope.
3. **Response middleware phases** — `response.pre_client` and `inspector.pre_capture` remain un-wired per YAML.
4. **ClickHouse analytics read path** — Admin still queries Postgres only.
5. **Real-time dashboard updates** — No WebSocket/SSE added.
6. **Integration test harness setup** — We document the tests and write the first `.rs` file, but if CI lacks Redis/Postgres services, the tests may be `#[ignore]` until infrastructure is added.
7. **Model validation against provider registry** — We do not verify that an allowed model string actually exists in the provider registry. Admins can type arbitrary strings.
8. **Rate limiter global default change** — The `RateLimiter::check_rate_limit` call in `pipeline.rs` still uses hardcoded `60, 100`. That is the *global* per-model rate limiter, a separate feature from per-key limits.

---

## 4. Sequenced Task List

| # | File Path | Nature | Depends On | Acceptance Criteria |
|---|-----------|--------|------------|---------------------|
| 1 | `crates/zorch-shared/src/config.rs` | Modify struct | — | Add `enforce_per_key_governance: bool` with env-var parsing (`ZORCH_ENFORCE_PER_KEY_GOVERNANCE`, default `true`). Compile passes. |
| 2 | `crates/zorch-gateway/src/key_limits.rs` | Modify struct + add method | — | Add `KeyLimitConfig::from_api_key(api_key: &ApiKey) -> Self` with fallback logic. Unit tests for all-null, with-values, zero-budget, empty-models pass. |
| 3 | `crates/zorch-api/src/middleware/auth.rs` | Modify function | — | After proxy-path auth succeeds, insert `api_key` into `req.extensions()`. Add unit test asserting extension is present. |
| 4 | `crates/zorch-api/src/routes/v1/proxy/mod.rs` | Modify function | 3 | Extract `api_key` from `parts.extensions` after `api_key_id`. Add to `RequestContext`. Compilation passes. |
| 5 | `crates/zorch-api/src/routes/v1/proxy/governance.rs` | Modify function + signature | 2, 4 | Change signature to accept `api_key: &ApiKey`. Replace `&KeyLimitConfig::default()` with `&KeyLimitConfig::from_api_key(api_key)`. Compilation passes. |
| 6 | `crates/zorch-gateway/src/pipeline.rs` | Modify function + signature | 5 | Extend `execute` to accept `api_key: &ApiKey`. Pass it to `governance.check_request`. Update all call sites in tests to compile. |
| 7 | `crates/zorch-gateway/src/governance.rs` | Modify function + signature | 6 | Refactor `check_request` to accept `api_key: Option<&ApiKey>`. If `Some`, skip DB query and use prefetched fields. Add unit tests for both paths. |
| 8 | `crates/zorch-api/src/routes/admin/types.rs` | Modify struct | — | Add `requests_per_minute: Option<i32>`, `requests_per_day: Option<i32>`, `max_spend_usd: Option<f64>`, `allowed_models: Option<Vec<String>>` to `ApiKeyResponse` with `#[serde(rename_all = "camelCase")]`. |
| 9 | `crates/zorch-api/src/routes/admin/api_keys.rs` | Modify functions | 8 | Extend `CreateApiKeyRequest`, `UpdateApiKeyRequest`, `build_update_sql`, `create_api_key`, `update_api_key`, and `fetch_api_keys_with_filter` to include the 4 governance fields. SELECT statement includes columns. INSERT/UPDATE binds them. Unit tests for SQL builder pass. |
| 10 | `apps/admin/lib/api.ts` | Modify interface + functions | — | Add governance fields to `ApiKey` interface. Extend `createApiKey` and `updateApiKey` payload types. TypeScript compiles. |
| 11 | `apps/admin/app/api-keys/page.tsx` | Modify component | 10 | Add governance form section to Create and Edit dialogs. Add "Limits" column to table. Client-side validation rejects out-of-range values. Manual test: create key with RPM=5, verify it appears in table. |
| 12 | `crates/zorch-gateway/src/key_limits.rs` | Modify error message | — | Change model-block error from `"Model '{}' is not in the allowed models list"` to `"Model not allowed for this API key"` to prevent information leakage. |
| 13 | `crates/zorch-api/src/routes/admin/analytics.rs` | Add endpoint + types | — | Add `get_rejection_analytics` handler and `RejectionAnalyticsResponse` / `RejectionSummary` types. Wire route in `crates/zorch-api/src/routes/admin/mod.rs`. Query returns results in <100ms on 1M row table (uses `created_at` index + `status_code` filter). |
| 14 | `apps/admin/app/analytics/page.tsx` (or new sub-page) | Add component | 13 | Create a "Rejected Requests" card/section showing top rejection reasons in the last 24h. Uses `useFetchData` pattern. |
| 15 | `crates/zorch-telemetry/src/metrics.rs` | Add helper | — | Add `record_governance_rejection(reason: &str)` backed by a Prometheus `CounterVec`. Wire calls in `pipeline.rs` for each rejection branch. Verify `/metrics` outputs `zorch_governance_rejections_total{reason="..."}`. |
| 16 | `crates/zorch-api/tests/proxy_governance.rs` | New file | 1-7 | Integration tests for RPM block, budget block, model block, and allow pass. Uses test DB + Redis db=15. All tests pass locally. If CI lacks Redis, mark `#[ignore]` and document in `TESTING.md`. |
| 17 | `docs/plans/testing_notes.md` | New file (optional) | 16 | Document how to run integration tests: `docker run -p 6379:6379 redis:7-alpine`, `cargo test -p zorch-api --test proxy_governance -- --ignored`. |
| 18 | `crates/zorch-api/src/routes/admin/api_keys.rs` | Add validation | 9 | Add `validate_governance` helper called in `create_api_key` and `update_api_key`. Rejects negative RPM, RPD, budget, or >128 model list. Returns `AppError::Validation`. Unit tests pass. |
| 19 | End-to-end manual verification | — | 11, 13 | Use `curl` to verify proxy blocks and dashboard shows limits. Check Prometheus metrics endpoint. |
| 20 | README / CHANGELOG update | — | 19 | Add note: "Per-key governance limits (RPM, RPD, budget, model allowlist) are now enforced by the proxy."

---

## 5. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | **Regression: existing keys with NULL columns suddenly behave differently** | Low | High | `from_api_key` uses the *same* hardcoded defaults (100/10k/$100/empty) that the proxy currently uses. No behavioral change for NULL rows. Verified by unit test. |
| 2 | **Performance: `GovernanceEngine` DB query removal accidentally breaks direct callers** | Low | Medium | The refactor makes `api_key` an `Option<&ApiKey>`. All existing internal callers in `pipeline.rs` will be updated to pass `Some`. Any external callers (unlikely; `governance` is internal to the gateway crate) will fall back to the old DB query path. |
| 3 | **Info leak: error messages reveal exact limits or allowed model lists** | Medium | Medium | Change model-block error to generic message (Task 12). Budget and RPM errors already do not reveal the limit value in the message body (they say "Budget exceeded" without showing the cap). |
| 4 | **Frontend type mismatch: backend returns `i32` but frontend sends `number`** | Low | Low | JSON numbers are lossless for values < 2^53. Our max (1M) is well within safe integer range. TypeScript `number` is sufficient. |
| 5 | **Concurrent budget overshoot** | High (existing) | Medium | This is pre-existing behavior (check-then-act on Redis). Documented as out-of-scope. The feature does not make it worse; it simply makes the limit configurable instead of hardcoded. |
| 6 | **Redis key explosion if many API keys exist** | Low | Medium | Redis keys are `key_rpm:{uuid}`, `key_rpd:{uuid}`, `key_spend:{uuid}`. One set of 3 keys per active key. With 10k keys = 30k Redis keys. Redis handles millions. TTL is 120s / 86500s, so stale keys auto-expire. |
| 7 | **Deployment rollback leaves dashboard expecting new fields** | Low | Low | Backend rollback removes fields from JSON response; frontend will see `undefined` and display "Default". This is graceful degradation. Admin cannot edit limits until backend is restored, but existing proxy behavior reverts to hardcoded defaults (same as today). |

---

*End of Plan*
