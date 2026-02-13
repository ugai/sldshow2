---
name: ultimate-issue-slayer
description: >
  End-to-end Issue-to-PR development workflow inside an isolated agent worktree.
  Use this when asked to pick up a GitHub Issue, implement the fix or feature,
  and open a pull request. This skill does NOT merge PRs.
---

# Ultimate Issue Slayer

Run a complete Issue → PR flow inside an isolated git worktree.

## 0. Worktree Setup

1. Determine the **main repo root** via `git rev-parse --show-toplevel`.
2. Fetch the latest remote state:
   ```bash
   git fetch origin main
   ```
3. After Issue Selection (step 1), create the worktree with the final branch name directly:
   ```bash
   git worktree add .agent-worktrees/<type>-issue-<N>-<description> -b <type>/issue-<N>-<description> origin/main
   ```
   - `<type>` — Conventional Commits type (`feat`, `fix`, `refactor`, …), determined from the issue.
   - `<N>` — Issue number.
   - `<description>` — Short kebab-case summary (max 32 chars).
   - Example branch: `feat/issue-42-ambient-fit-shader`
   - Example worktree path: `.agent-worktrees/feat-issue-42-ambient-fit-shader`
4. If the worktree path already exists, **abort** and ask the user.
5. `cd` into the new worktree to perform all subsequent work.

## 1. Issue Selection & Setup

1. If the user specifies an issue number, use that.
   Otherwise, find unassigned open issues:
   ```bash
   gh issue list --search "no:assignee state:open" --limit 20
   ```
   Present the list and let the user pick, or pick the highest-priority one if instructed.
2. Claim the issue immediately:
   ```bash
   gh issue edit <N> --add-assignee "@me"
   gh issue comment <N> --body "Starting work on this issue."
   ```
3. If assignment fails (race condition), pick another issue.
4. Now proceed with worktree creation (step 0.3) using the issue details.

## 2. Design (Plan Mode)

1. Read the issue details, relevant source files, `CLAUDE.md`, and `CONTRIBUTING.md`.
2. Use **EnterPlanMode** to design the implementation approach.
   - Do **not** create an `implementation_plan.md` file. The plan lives in Claude Code's plan mode.
3. Wait for user approval before writing any code.

## 3. Implementation

- Prefer new modules under `src/`; keep `main.rs` changes minimal.
- Follow the project's Conventional Commits format and coding standards
  defined in `CLAUDE.md` and `CONTRIBUTING.md`.
- For co-author trailers in commit messages, refer to the **AI Co-Authorship**
  section in `CLAUDE.md` and use the appropriate trailer for the current agent.

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

1. Rebase onto `origin/main` and resolve any conflicts:
   ```bash
   git fetch origin main
   git rebase origin/main
   ```
2. Push the branch and open a PR:
   ```bash
   git push -u origin <branch-name>
   gh pr create --title "<type>: <description>" --body "Closes #<N>"
   ```
   - Title follows Conventional Commits (e.g., `feat: add ambient fit shader`).
   - Body references the issue (`Closes #<N>`).
3. **Do NOT merge.** Notify the user that the PR is ready for review.

## 6. Cleanup (Optional)

When the user requests cleanup after the PR is merged:

1. Return to the main repo root.
2. Remove the worktree and branch:
   ```bash
   git worktree remove .agent-worktrees/<type>-issue-<N>-<description>
   git branch -d <type>/issue-<N>-<description>
   ```
   Use `git branch -D` only if the user explicitly requests force deletion.
