---
description: Explore current project status
agent: build
---
You maintain a single source of truth file:

docs/project_state.yaml

Purpose:

* Provide a complete project status snapshot for humans and AI agents.
* Allow future agents to understand project progress without scanning the entire repository.
* Track implemented capabilities incrementally across commits.
* Maintain a concise summary of current project state.

Workflow:

1. Check whether docs/project_state.yaml exists.

2. If the file does not exist:

   * Create it.
   * Analyze the current repository.
   * Generate an initial project snapshot.
   * Set last_commit to the current HEAD commit hash.

3. If the file exists:

   * Read last_commit.
   * Compare last_commit with current HEAD.
   * Analyze only the git diff between those commits.
   * Detect newly added, removed, or significantly modified features.
   * Update the project state accordingly.

Update the following sections:

* summary
* features
* next_recommended
* recent_changes
* last_commit
* updated_at

Feature Rules:

* Track user-visible capabilities.
* Track operator-visible capabilities.
* Track API-visible capabilities.
* Ignore internal refactors.
* Ignore file moves.
* Ignore formatting changes.
* Ignore dependency updates unless they enable new functionality.
* Prefer stable feature identifiers.

Summary Rules:

* Maximum 10 lines.
* Describe what the project currently does.
* Mention major completed capabilities.
* Mention major missing capabilities.

Next Recommended Rules:

* Recommend the highest-leverage next actions.
* Base recommendations on current project state.
* Remove recommendations that are already completed.

Recent Changes Rules:

* Only record meaningful feature changes.
* Keep the most recent 20 entries.
* Summarize changes in business or user-facing language.

After updating the file:

* Set last_commit to current HEAD.
* Save docs/project_state.yaml.
