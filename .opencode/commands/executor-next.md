---
description: Execute the current plan task by task, archive on success, commit, and update project state
agent: build
---
Context: You are executing a builder-ready plan stored in `docs/plans/{plan-name}.md`. The plan contains a sequenced task list (Section 4) that must be implemented linearly. Each task specifies an exact file path, nature (new/modify), dependencies, and acceptance criteria.

Your Goal: Execute tasks in order until the plan is fully implemented and verified. Then archive the plan, commit the changes, and refresh the project state snapshot.

---
Phase 1: Load Plan
1. Discover the current plan file in `docs/plans/` (there should be exactly one `.md` file; if multiple, ask the user which one to execute).
2. Read the plan in full. Identify the task list in Section 4 (the sequenced table).
3. Note the first uncompleted task. If no progress tracking exists, start at task #1.

---
Phase 2: Execute Next Task
For the current task:
1. Verify all "Depends On" tasks are complete (check git diff or file state).
2. Implement exactly what the task specifies:
   - If "new file": create the file with the described content.
   - If "modify function": read the file, apply the minimal surgical change, preserve existing style.
   - If "add migration": create the migration in `migrations/` with timestamp prefix.
   - If "add test": write the test in the specified file or new test file.
   - If "add route": wire the route in the router module.
3. After making changes, verify the acceptance criteria:
   - If Rust code: run `cargo check -p <crate>` for the affected crate.
   - If tests exist: run `cargo test -p <crate> <filter>`.
   - If TypeScript/Next.js: run `npm run build` or `npx tsc --noEmit` in `apps/admin/`.
   - If SQL: verify syntax with `sqlx migrate run --source migrations` (if DB is available).
4. If verification fails, fix the issue. Do not proceed to the next task until the current one passes.
5. Record progress: append a brief completion note (task #, file, status) to a scratchpad comment in the conversation.

---
Phase 3: Loop Until Completion
Repeat Phase 2 for each subsequent task in the sequenced list.
- Skip tasks already completed (verify by checking file contents match the task description).
- Stop if a task cannot be completed due to a blocking issue (missing dependency, ambiguous spec, or external system failure). Report the blocker to the user.
- Stop if the user interrupts or changes scope.

---
Phase 4: Final Verification
After the last task is complete:
1. Run a full workspace check: `cargo check --workspace`.
2. Run all unit tests: `cargo test --workspace`.
3. Run the Next.js build: `cd apps/admin && npm run build`.
4. If any failures, fix them before proceeding.

---
Phase 5: Archive Plan & Commit
1. Move the completed plan from `docs/plans/{plan-name}.md` to `docs/plans/archived/{plan-name}-{date}.md` (create `docs/plans/archived/` if needed).
2. Stage all changes: `git add -A`.
3. Create a commit with a message summarizing the feature:
   - Format: `feat(scope): description`
   - Example: `feat(governance): enforce per-key RPM/RPD/budget/model-allowlist limits`
   - Body: list the major files modified and any breaking changes.
4. Do NOT push to remote unless explicitly asked.

---
Phase 6: Update Project State
Run the `/update-project-features` command to refresh `docs/project_state.yaml` with the newly completed capabilities.

---
Constraints
- Make MINIMAL changes. Do not refactor unrelated code.
- Preserve existing coding style (formatting, naming, patterns).
- Do not add new dependencies without user approval.
- Do not delete or modify plan tasks that are marked out-of-scope.
- If a task acceptance criterion requires a manual test (no automated test exists), perform the manual steps and report the result.

---
Final Deliverable
A concise summary of:
1. Which tasks were completed.
2. Any tasks skipped or blocked (with reason).
3. The archived plan path.
4. The git commit hash.
5. Confirmation that `docs/project_state.yaml` was updated.
