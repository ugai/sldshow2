---
name: issue-slayer
description: >
  Implements GitHub Issues end-to-end inside an isolated git worktree and opens
  a pull request. Use when asked to pick up an issue, implement a fix or
  feature, or deliver a PR. Requires the agent:ready label. Does NOT merge PRs.
---

# Issue Slayer

Run a complete Issue → PR flow inside an isolated git worktree.

## Execution Mode

**Pattern A (Standalone)**: Present a plan for user approval before writing
code. User drives cleanup decisions.

**Pattern B (Team Member)**: Send plans to the Team Lead for approval before
writing code. Cleanup requires Lead instruction.

Detection: see [ENV.md](ENV.md) for how to detect team context in your tool.

See [ENV.md](ENV.md) for tool-specific and shell-specific commands.

## Issue Selection

**Eligibility** — ALL must be true:

1. Has `agent:ready` label
2. State is `open`
3. Not assigned (`no:assignee`)
4. Does NOT have `pending` label

**Priority**: `bug` > `enhancement`, then `p0` > `p1` > `p2` > `p3`
(no label = p2), then lowest issue number.

**Never** pick without `agent:ready`. **Never** pick an assigned issue.
One agent, one issue.

```bash
gh issue list --label "agent:ready" \
  --search "no:assignee state:open -label:pending" \
  --json number,title,labels --limit 20
```

If the user specifies an issue number, verify eligibility before proceeding.

**Pattern A**: Present the ranked list and let the user choose.
**Pattern B**: Check what teammates have claimed via task list. Pick the
highest-priority unclaimed issue, or use the one assigned to you.

Claim immediately after selection:

```bash
gh issue edit <N> --add-assignee "@me"
gh issue comment <N> --body "Starting work.
Agent: **<agent-name>** | Model: **<model-name>** | Tool: **<tool-name>**"
```

If assignment fails (race condition), pick another issue.

## Worktree Setup

See [WORKTREE.md](WORKTREE.md) for the full setup procedure. Summary:

```bash
git fetch origin main
git worktree add .agent-worktrees/<type>-issue-<N>-<desc> \
  -b <type>/issue-<N>-<desc> origin/main
```

If the path already exists, abort and ask the user. Do all subsequent work
inside the worktree.

## Design

Read the issue, relevant source files, `CLAUDE.md`, and `CONTRIBUTING.md`.

**Pattern A**: Present the implementation plan and wait for approval before
writing code.
**Pattern B**: Send the plan to the Lead and wait for approval.

Do not create an `implementation_plan.md` file.

## Implementation

- New functionality → new module under `src/`. Keep `main.rs` and `app.rs`
  changes minimal.
- Follow Conventional Commits and coding standards in `CLAUDE.md`.
- Co-author trailer: see AI Co-Authorship section in `CLAUDE.md`.
- **Pattern B**: Minimize changes to `app.rs`, `main.rs`, `Cargo.toml`,
  `config.rs`. Keep shared-file diffs small and localized.

**Doc updates** (part of implementation, not a separate step):
- `example.sldshow` — add entries for any new config options in `config.rs`
- `CONTRIBUTING.md` — update if a new dev pattern or workflow was introduced
- `CLAUDE.md` — add new modules to the Module Map; document new conventions

## Verification

See [VERIFY.md](VERIFY.md) for the full quality gate. All four commands must
pass before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

After passing, output the manual test command so the user can run it.
See [ENV.md](ENV.md) for the correct syntax for your shell.

## Pull Request

Write the PR body to a temp file to avoid shell-escaping issues with backticks
and other special characters in Markdown code spans. Never use inline `--body`
for multi-line PR descriptions.

```bash
git fetch origin main
git rebase origin/main
git push -u origin <branch-name>
cat > /tmp/pr_body_<N>.md << 'EOF'
Closes #<N>

## Overview
<1-2 sentences>

## Changes
- <bullet>

## Testing
- [x] All quality gate checks passed (see VERIFY.md)
- [ ] Manual testing recommended
EOF
gh pr create --title "<type>: <description>" --body-file /tmp/pr_body_<N>.md
```

Do **not** merge. Notify the approver that the PR is ready.

## Cleanup (On Request)

When instructed after the PR is merged:

```bash
git worktree remove .agent-worktrees/<type>-issue-<N>-<desc>
git branch -d <type>/issue-<N>-<desc>
```

Use `git branch -D` only if explicitly requested. Pattern B: wait for Lead
instruction before removing anything.

## Team Operation Flow

```
1. Lead: create team → create tasks (per issue) → spawn issue-slayer agents
2. Each agent: claim → plan → Lead approval → implement → verify → PR
3. Lead: coordinate merge order → rebase if needed → shut down → delete team
```
