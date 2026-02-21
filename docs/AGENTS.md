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

## Skills

The guild's agents are defined in `.claude/skills/`. Each skill can also be
invoked as an autonomous agent via the Task tool.

<table>
<tr>
<td align="center" width="50%">
<img src="assets/portrait-agent-slayer.png" width="300"><br>
<strong><code>issue-slayer</code></strong> — <em>The Blade That Closes Issues.</em><br>
Picks up <code>agent:ready</code> issues, implements in an isolated worktree, and
delivers pull requests. Does not theorize. Does not over-engineer.
One issue. One PR. Every time.
</td>
<td align="center" width="50%">
<img src="assets/portrait-agent-ranger.png" width="300"><br>
<strong><code>issue-ranger</code></strong> — <em>Eyes of the Guild.</em><br>
Ranges the codebase from six vantage points, gathers intel from abroad,
and posts well-scoped issues on the board. Never fights. Never codes.
Only scouts, only reports.
</td>
</tr>
</table>
