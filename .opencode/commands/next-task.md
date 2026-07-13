---
description: Plan a single most valuable next task
agent: build
---
Context: You are analyzing the Zorch project — a Rust Cargo workspace monolith for AI provider API key orchestration. It sits in $(workspace)/zorch.

Your Goal: Determine the single most valuable next feature to implement and produce a builder-ready execution plan.

Source of Truth: The file `docs/project_state.yaml` is the canonical project state snapshot maintained by previous agents. **Read it first.** It contains a human- and machine-readable catalog of features (complete, partial, missing), next recommendations, and recent changes. Use it as your primary input instead of scanning the entire repository.

---
Phase 1: State Validation (Targeted, Not Full-Scan)
1. Read `docs/project_state.yaml` and  `docs/plans` in full. Note:
   - Which features are marked `complete`, `partial`, or `missing`
   - The `next_recommended` list (ordered by priority)
   - The `recent_changes` timeline
   - The {PLAN}.md that in `docs/plans` so you will not make duplicated plans for the same task.
2. Do a **spot-check** only for the top 1-2 recommended features to verify claims:
   - If a feature is marked `partial` or `missing`, read the 2-3 files the state file implies are involved and confirm the gap still exists.
   - If the state file says a feature is `complete`, do a quick grep to confirm no TODO/FIXME regressions.
3. If `docs/project_state.yaml` does not exist or is severely out of date, fall back to a full scan (see full-scan procedure in git history).

Deliverable from Phase 1: A concise "State of the Union" summary referencing the YAML state plus any deltas you discovered during spot-checks.

---
Phase 2: Feature Candidate Generation
Based on the validated state, identify 3-5 candidate next features. Sources (in priority order):
1. The `next_recommended` list from `docs/project_state.yaml`
2. Any deltas you discovered during spot-checks
3. Operational hardening not yet tracked (e.g., no integration tests, no pagination)

For each candidate, score:
- User/Business Value
- Implementation Complexity
- Foundation for Future Features
- Risk (schema, concurrency, security)
- Time-to-Value (can ship incrementally?)

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
- **Trust `docs/project_state.yaml` as the primary source of truth.** Only verify specific claims when you spot-check.
- Be specific. Name the function, the SQL column, the API path. No hand-waving.
- If unsure, state the assumption and flag it.

---
Final Deliverable
A single markdown plan document saved to `docs/plans/{plan-name}.md` in the project root, containing:
1. State of the Union (with deltas from YAML if any)
2. Candidate features with scoring
3. Deep-dive plan for #1
4. Sequenced task list
5. Risk register (top 3-5 risks + mitigations)
