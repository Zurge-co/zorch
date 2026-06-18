---
description: Plan a single most valuable next task
agent: plan
---
Context: You are analyzing the Zorch project — a Rust Cargo workspace monolith for AI provider API key orchestration. It sits in $(workspace)/zorch. You must discover ground truth by reading code.
Your Goal: Determine the single most valuable next feature to implement and produce a builder-ready execution plan.
---
Phase 1: Ground-Truth Discovery
Read and catalog the actual state of the codebase:
1. Workspace Structure — Read Cargo.toml, all crates/*/Cargo.toml, and crates/*/src/lib.rs to map crate boundaries and dependencies.
2. Database Schema — Read ALL files in /migrations/ and query the actual schema state if possible. Catalog every table, column, index, and foreign key.
3. API Surface — Read zorch-api/src/routes/ completely. List every implemented endpoint (method + path) and note which admin features exist.
4. Proxy Pipeline — Read zorch-gateway/src/pipeline.rs, zorch-gateway/src/lib.rs, and zorch-providers/src/proxy.rs. Trace the exact request lifecycle from auth → governance → middleware → upstream → response → inspector. Note where data is captured and where it's missing.
5. Middleware Engine — Read zorch-gateway/src/middleware/ entirely. Which plugins exist? Is the engine fully wired? Is request.pre_governance, request.pre_upstream, response.pre_client, inspector.pre_capture actually executed?
6. Admin Dashboard — Read apps/admin/ pages. List every page, what data it fetches, and what CRUD operations it supports.
7. Inspector & Analytics — Read zorch-inspector/src/, ClickHouse init scripts, and analytics route handlers. What is captured? What aggregations exist? What is missing?
8. Test Reality — List all test files. What passes? What is skipped? What coverage gaps exist?
Deliverable from Phase 1: A concise "State of the Union" summary: what exists, what is partially implemented, what is stubbed, and what is completely missing.
---
Phase 2: Feature Candidate Generation
Based on Phase 1, identify 3-5 candidate next features. Sources:
- Gaps discovered in the proxy pipeline (e.g., access windows partially exist but aren't enforced?)
- Missing analytics capabilities (e.g., no cost-by-project attribution?)
- Admin dashboard missing CRUD (e.g., can't edit middleware configs?)
- Operational hardening (e.g., no alerting thresholds? no API key rotation?)
- Performance/scalability (e.g., missing connection pooling tuning? no batching?)
Do NOT limit yourself to TODO.md suggestions. If TODO.md claims a feature is done but the code shows it's stubbed, treat it as missing.
For each candidate, score:
Criterion
User/Business Value
Implementation Complexity
Foundation for Future Features
Risk (schema, concurrency, security)
Time-to-Value (can ship incrementally?)
Deliverable from Phase 2: Ranked list with one-paragraph justification for each.
---
Phase 3: Deep-Dive Technical Plan for #1 Feature
Produce a builder-ready plan with these exact sections:
3.1 Goal & Definition of Done  
One paragraph. What user/admin journey now works?
3.2 Schema Changes  
Exact SQL migration(s). Column types, defaults, indexes, constraints. Justify each choice.
3.3 Data Model  
New/updated Rust structs (with serde annotations), validation rules, error types. Specify which crate(s) they live in.
3.4 API Contract  
- New/modified REST endpoints (method, path, request/response JSON, status codes)  
- OpenAPI/utoipa annotations required  
- Backward compatibility notes
3.5 Proxy Pipeline Integration  
- Exact file/function to modify  
- Where in the request lifecycle the new logic inserts  
- Performance: does it add a DB query per request? Can it be cached in Redis?  
- How the request context is extended
3.6 Admin Dashboard Changes  
- Page(s) modified or created  
- Data fetching pattern (match existing: SWR, tRPC, raw fetch)  
- Form validation schema  
- UX flow step-by-step
3.7 Analytics & Observability  
- New queries (PostgreSQL or ClickHouse)  
- New admin endpoint for aggregated data  
- New Prometheus metric if applicable
3.8 Testing Plan  
- Unit tests: which functions, which edge cases (boundaries, empty, max limits, invalid formats, concurrency)  
- Integration tests: full request flow; assert DB state, HTTP response, and side effects  
- Migration safety: rollback test  
- Admin manual checklist if no E2E suite exists
3.9 Security & Safety  
- Injection risks (SQL, JSONB, regex)  
- Secret exposure risks  
- Enumeration or DoS risks  
- Timezone/DST edge cases if applicable
3.10 Rollback & Compatibility  
- Is migration backward-compatible with running code?  
- Can old binaries run against new schema?  
- Feature flag needed?
3.11 Out-of-Scope  
Explicitly list what this feature does NOT include.
---
Phase 4: Sequenced Task List
10-20 linear tasks. Each task specifies:
- Exact file path (workspace-relative)
- Nature: new file / modify function / add migration / add test / add route
- Depends on: previous task numbers
- Acceptance Criteria: how to verify correctness
---
Constraints
- READ-ONLY analysis only. Do not modify any files.
- Do not trust TODO.md. Verify every claim by reading actual source.
- Be specific. Name the function, the SQL column, the API path. No hand-waving.
- If unsure, state the assumption and flag it.
---
Final Deliverable
A single markdown plan document saved to docs/plans/{plan-name}.md in the project root, containing:
1. State of the Union
2. Candidate features with scoring
3. Deep-dive plan for #1
4. Sequenced task list
5. Risk register (top 3-5 risks + mitigations)
---