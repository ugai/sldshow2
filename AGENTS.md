# AI Agent Workflow

`sldshow2` supports autonomous development using AI Agents (like adding a co-pilot to your team). This document defines the rules of engagement.

## Labels

### `agent:ready`

Issues must have the **`agent:ready`** label before an AI agent can pick them up.
This is an opt-in guardrail — maintainers explicitly approve issues for autonomous implementation by adding this label.

### `agent:proposed`

Issues with **`agent:proposed`** were opened by the `issue-ranger` skill.
They are **not yet approved** for autonomous implementation. Agents must wait until a maintainer adds `agent:ready` before picking them up.

> **Note**: `agent:proposed` is an origin label, not a status. It stays on the issue even after `agent:ready` is added, so you can always filter AI-proposed issues with `--label agent:proposed`.

### Eligibility Criteria
An agent may only work on an issue if **ALL** of the following are true:
1.  Has the `agent:ready` label
2.  Is Open
3.  Is Unassigned
4.  Does **NOT** have a `pending` label

### Priority
When multiple eligible issues exist, agents favor:
1.  `bug` > `enhancement`
2.  `priority:p0` > `priority:p1` > `priority:p2` > `priority:p3` (no label = `p2`)
3.  Lowest issue number

## Recommended Workflow

```
issue-ranger          Scout the codebase → post agent:proposed issues
      ↓
  (human)             Review and add agent:ready label
      ↓
issue-raid-commander  Analyze ready queue → detect conflicts → output sprint plan
      ↓
issue-slayer × N      Implement in parallel worktrees → open PRs
```

Run `issue-raid-commander` before spawning a slayer team to avoid merge conflicts.
For single-issue work, skip it and go straight to `issue-slayer`.

**Full pipeline shortcut**: `dispatching-guild-expedition` runs the entire
workflow above in one command — Rangers × 4, user approval gate, Commander,
then Slayers × N in parallel.

## Execution Patterns

We use two primary patterns for agent work, both utilizing isolated `git worktree`s to avoid messing with your main branch.

| Pattern | How it runs | Plan Approval | Use Case |
| :--- | :--- | :--- | :--- |
| **A (Standalone)** | User invokes the skill directly | **User approves** via chat | Single-issue work |
| **B (Team)** | Team Lead spawns multiple agents | **Lead approves** via message | Parallel multi-issue sprint |

## Commit & PR conventions

-   **Co-authorship trailer** — format: `Co-Authored-By: {model} ({tool}) <email>`. Use the actual model name:
    -   Claude Code: `Co-Authored-By: {model} (Claude Code) <noreply@anthropic.com>`
    -   Gemini CLI: `Co-Authored-By: {model} (Gemini CLI) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
    -   Antigravity: `Co-Authored-By: {model} (Antigravity) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
-   **Branch Naming**: `<type>/<kebab-case-description>` (e.g., `feat/add-ambient-blur`)
-   **PR Title**: Conventional Commits (e.g., `feat: add ambient blur shader`)
-   **PR Body**: Must include `Closes #<issue-number>`.

## General Rules

-   **Do not** create git tags or releases unless explicitly instructed.
-   **New features**: Extract to dedicated modules (e.g., `src/drag_drop.rs`). Keep `main.rs` and `app.rs` diffs minimal.
-   **Conflict-prone files**: `app.rs`, `main.rs`, `Cargo.toml`, `config.rs` — keep changes small and localized.

