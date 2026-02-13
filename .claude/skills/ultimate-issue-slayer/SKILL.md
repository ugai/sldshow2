---
name: ultimate-issue-slayer
description: >
  End-to-end Issue-to-PR development workflow inside an isolated agent worktree.
  Use this when asked to pick up a GitHub Issue, implement the fix or feature,
  and open a pull request. This skill does NOT merge PRs.
---

# Ultimate Issue Slayer

Run a complete Issue → PR flow inside a worktree managed by the `worktree-manager` skill.

> **Boundary rule**: This skill never creates or deletes worktrees.
> Worktree lifecycle is handled exclusively by `worktree-manager`.

## 0. Pre-flight — Verify Worktree

1. Check `Get-Location` and `git rev-parse --show-toplevel`.
   - Expected path pattern: `.agent-worktrees/agent-<agent_id>-<task>`.
2. If already inside an agent worktree → continue.
3. If **not** inside a worktree → ask `worktree-manager` to create a temporary
   `agent-<agent_id>-wip` worktree, then `cd` into it.
4. Abort only if the user declines or the manager reports a collision.

### Naming conventions

| Element | Format | Example |
|---------|--------|---------|
| `agent_id` | `[a-zA-Z0-9]+` | `copilot01` |
| Temporary branch | `agent-<agent_id>-wip` | `agent-copilot01-wip` |
| Final branch | `agent-<agent_id>-<task>` | `agent-copilot01-issue-42` |
| `<task>` | `[a-z0-9-]{1,32}` | `issue-42` |

## 1. Issue Selection & Setup

1. Find an unassigned issue: `gh issue list --assignee "" --state open`.
2. Claim it immediately:
   ```bash
   gh issue edit <id> --add-assignee "@me"
   gh issue comment <id> --body "Starting work (Agent: <agent_id>)"
   ```
3. If assignment fails (race condition), pick another issue.
4. Create the final branch inside the worktree:
   ```bash
   git switch -c agent-<agent_id>-issue-<number>
   ```

## 2. Design & Planning

1. Draft `implementation_plan.md` (scope, approach, acceptance criteria).
2. **Wait for user review** before writing any code.

## 3. Implementation

- Prefer new modules under `src/`; keep `main.rs` changes minimal.
- Follow the project's Conventional Commits format and coding standards
  defined in `CONTRIBUTING.md` and `CLAUDE.md`.
- Include co-author trailer when appropriate:
  ```
  Co-authored-by: GitHub Copilot <copilot@users.noreply.github.com>
  ```

## 4. Verification

Run the full quality gate locally before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

All four commands must pass. Do not push on failure.

## 5. Pull Request

1. Rebase onto `origin/main` and resolve any conflicts.
2. Push the branch and open a PR:
   - Title follows Conventional Commits (e.g., `feat: add ambient fit shader`).
   - Body references the issue (`Closes #<number>`) and links to `implementation_plan.md`.
3. **Do NOT merge.** Enter WAITING state and notify the user for review.
