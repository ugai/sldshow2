# AI Agent Guide

This document covers the workflow protocol and codebase-specific rules for AI agents working in this repository.

## Workflow

### Recommended Workflow

```text
issue-ranger          Scout the codebase → post agent:proposed issues
      ↓
  (human)             Review and add agent:ready label
      ↓
issue-raid-commander  Analyze ready queue → detect conflicts → output sprint plan
      ↓
issue-slayer × N      Implement in parallel worktrees → open PRs
      ↓
quality-finisher      Audit PRs for test coverage → push tests or post comments
      ↓
verify-sprint         Merge PR branches locally → visual check → squash merge to main
```

Run `issue-raid-commander` before spawning a slayer team to avoid merge conflicts.
For single-issue work, skip it and go straight to `issue-slayer`.

**Full pipeline shortcut**: `dispatching-guild-expedition` runs the entire
workflow above in one command — Rangers × 4, user approval gate, Commander,
then Slayers × N in parallel. Follow up with `verify-sprint` to verify and
merge the opened PRs.

### Execution Patterns

We use two primary patterns for agent work, both utilizing isolated `git worktree`s to avoid messing with your main branch.

| Pattern | How it runs | Plan Approval | Use Case |
| :--- | :--- | :--- | :--- |
| **A (Standalone)** | User invokes the skill directly | **User approves** via chat | Single-issue work |
| **B (Team)** | Team Lead spawns multiple agents | **Lead approves** via message | Parallel multi-issue sprint |

### Commit & PR Conventions

- **Co-authorship trailer** — format: `Co-Authored-By: {model} ({tool}) <email>`. Use the actual model name:
  - Claude Code: `Co-Authored-By: {model} (Claude Code) <noreply@anthropic.com>`
  - GitHub Copilot: `Co-Authored-By: {model} (GitHub Copilot) <175728472+Copilot@users.noreply.github.com>`
  - Gemini CLI: `Co-Authored-By: {model} (Gemini CLI) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
  - Antigravity: `Co-Authored-By: {model} (Antigravity) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
- **Branch Naming**: `<type>/<kebab-case-description>` (e.g., `feat/add-ambient-blur`)
- **PR Title**: Conventional Commits (e.g., `feat: add ambient blur shader`)
- **PR Body**: Must include `Closes #<issue-number>`.
- **One Issue, One PR** — default policy. Each issue gets its own PR.

#### Bundle PR (Exception)

Raid Commander (or a human) may group issues into a **Bundle PR** when ALL:

1. Same fix pattern (e.g., unwrap removal, lint fix, dep bump)
2. Each issue is **small** complexity
3. No file conflicts within the group
4. Total diff is reviewable as a single unit

Bundle PR rules:

- One commit per issue (`Ref #<N>` in each commit message)
- PR body lists all `Closes #<N>`
- Slayer uses a single worktree
- Raid Commander flags candidates as `bundleable` in the sprint plan;
  Slayer follows that designation (or a direct user/lead instruction)

## Labels

### `agent:ready`

Issues must have the **`agent:ready`** label before an AI agent can pick them up.
This is an opt-in guardrail — maintainers explicitly approve issues for autonomous implementation by adding this label.

### `agent:proposed`

Issues with **`agent:proposed`** were opened by the `issue-ranger` skill.
They are **not yet approved** for autonomous implementation. Agents must wait until a maintainer adds `agent:ready` before picking them up.

> **Note**: `agent:proposed` is an origin label, not a status. It stays on the issue even after `agent:ready` is added, so you can always filter AI-proposed issues with `--label agent:proposed`.

### Issue Creation Protocol

When creating new issues, an agent MUST:

1. Add the `agent:proposed` label to every issue it creates
2. Never add `agent:ready` — that label is reserved for human maintainers

### Eligibility Criteria

An agent may only work on an issue if **ALL** of the following are true:

1. Has the `agent:ready` label
2. Is Open
3. Is Unassigned
4. Does **NOT** have a `pending` label

### Issue Pickup Protocol

Before writing any code, an agent MUST:

1. Self-assign the issue to itself
2. Post a comment on the issue announcing that work has started

### Priority

When multiple eligible issues exist, agents favor:

1. `bug` > `enhancement`
2. `priority:p0` > `priority:p1` > `priority:p2` > `priority:p3` (no label = `p2`)
3. Lowest issue number

## Codebase Rules

### General Rules

- **Do not** create git tags or releases unless explicitly instructed.
- **New features**: Extract to dedicated modules (e.g., `src/drag_drop.rs`). Keep `main.rs` and `app.rs` diffs minimal.
- **Conflict-prone files**: `app.rs`, `main.rs`, `Cargo.toml`, `config.rs` — keep changes small and localized.
- **Avoid hardcoding counts** in docs or comments (e.g., "20 transitions", "6 perspectives"). Counts change as features are added. Write descriptively ("multiple", "each") and let the source of truth (code, config, RECON.md) be the only place the number lives.
