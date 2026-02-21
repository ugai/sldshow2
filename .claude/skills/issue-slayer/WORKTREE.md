# Worktree Setup

## Naming convention

| Part | Value |
|------|-------|
| `<type>` | Conventional Commits type from the issue: `feat`, `fix`, `refactor`, … |
| `<N>` | Issue number |
| `<desc>` | Short kebab-case summary, max 32 chars |

- Branch: `<type>/issue-<N>-<desc>` — e.g. `feat/issue-42-ambient-fit-shader`
- Worktree path: `.agent-worktrees/<type>-issue-<N>-<desc>`

## Steps

1. Confirm the main repo root:
   ```bash
   git rev-parse --show-toplevel
   ```

2. Fetch latest remote state:
   ```bash
   git fetch origin main
   ```

3. Create the worktree (do this after issue selection so you have `<N>` and
   `<desc>`):
   ```bash
   git worktree add .agent-worktrees/<type>-issue-<N>-<desc> \
     -b <type>/issue-<N>-<desc> origin/main
   ```

4. If the worktree path already exists → **abort and ask the user**.

5. `cd` into the worktree. Do all subsequent work there.
