---
name: ultimate-issue-slayer
description: >
  End-to-end Issue-to-PR development workflow inside an isolated agent worktree.
  Use this when asked to pick up a GitHub Issue, implement the fix or feature,
  and open a pull request. This skill does NOT merge PRs.
---

# Ultimate Issue Slayer

Run a complete Issue → PR flow inside an isolated git worktree.

## Issue Selection Rules

**Eligibility** — An issue may only be picked if ALL of the following are true:

1. Has the `agent:ready` label
2. State is `open`
3. Not assigned to anyone (`no:assignee`)
4. Does NOT have the `pending` label

**Priority** — When multiple eligible issues exist, prefer in this order:

1. `bug` over `enhancement`
2. `phase:1` > `phase:2` > `phase:3`
3. Lower issue number first

**Prohibitions**:

- **Never** pick an issue without the `agent:ready` label.
- **Never** pick an issue that is already assigned.
- One agent, one issue — do not work on multiple issues simultaneously.

## Execution Mode

This skill supports two execution patterns:

**Pattern A (Standalone)**: Invoked directly by a user. Uses `EnterPlanMode`
for user approval. The user drives cleanup decisions.

**Pattern B (Team Member)**: Spawned by a Team Lead via `Task` with a
`team_name`. Uses `SendMessage` to send plans to the Lead for approval.
Cleanup requires Lead instruction.

Detection: If you were spawned with a `team_name` or received a task
assignment via `TaskList`/`TaskUpdate`, you are in Pattern B. Otherwise
Pattern A.

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

1. Query eligible issues:
   ```bash
   gh issue list --label "agent:ready" --search "no:assignee state:open -label:pending" --json number,title,labels --limit 20
   ```
2. If the user specifies an issue number, verify it meets the eligibility
   criteria above before proceeding.
3. **Pattern A**: Present the ranked list to the user and let them choose.
   **Pattern B**: Check `TaskList` to see what other teammates are working on.
   Select the highest-priority eligible issue that no other teammate has
   claimed. If a specific issue was assigned to you via `TaskUpdate`, use that.
4. Claim the issue immediately:
   ```bash
   gh issue edit <N> --add-assignee "@me"
   gh issue comment <N> --body "🤖 Starting work on this issue.

   Agent: **<agent-name>** | Model: **<model-name>** | Tool: **<tool-name>**"
   ```
   - `<agent-name>` — The agent definition name (e.g., `issue-slayer`).
   - `<model-name>` — The actual model powering this session (e.g., `Claude Sonnet 4.5`, `Claude Opus 4.6`). Determined from your system prompt.
   - `<tool-name>` — The client tool (e.g., `Claude Code`).
5. If assignment fails (race condition), pick another issue.
6. Now proceed with worktree creation (step 0.3) using the issue details.

## 2. Design (Plan Mode)

1. Read the issue details, relevant source files, `CLAUDE.md`, and `CONTRIBUTING.md`.
2. **Pattern A**: Use `EnterPlanMode` to design the implementation approach.
   Wait for user approval before writing any code.
   **Pattern B**: Draft the implementation plan and send it to the Team Lead
   via `SendMessage`. Wait for the Lead's approval message before writing code.
   - Do **not** create an `implementation_plan.md` file. The plan lives in
     Claude Code's plan mode (Pattern A) or in the message exchange (Pattern B).

## 3. Implementation

- Prefer new modules under `src/`; keep `main.rs` and `app.rs` changes minimal.
- Follow the project's Conventional Commits format and coding standards
  defined in `CLAUDE.md` and `CONTRIBUTING.md`.
- For co-author trailers in commit messages, refer to the **AI Co-Authorship**
  section in `CLAUDE.md` and use the appropriate trailer for the current agent.
- **Team Note**: When working in Pattern B, minimize changes to conflict-prone
  files (`app.rs`, `main.rs`, `Cargo.toml`, `config.rs`). Extract new functionality into
  dedicated modules. Keep diffs to shared files small and localized.

### Documentation & Example Updates

After implementing the feature or fix, check whether accompanying files need
updates. This is part of the implementation — not a separate step.

- **`example.sldshow`**: If new config options were added to `config.rs`,
  add corresponding entries (with comments) to `example.sldshow` so users
  can discover them. Match the existing style (TOML comments, grouping).
- **`CONTRIBUTING.md`**: If the change introduces a new development pattern,
  build step, or workflow rule, update `CONTRIBUTING.md` accordingly.
- **`CLAUDE.md`**: If a new module was created, add it to the Module Map
  table. If a new convention was established, document it.

## 4. Verification

Run the full quality gate locally before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

All four commands must pass. Do not push on failure.

### Manual Test Command

After verification passes, output a ready-to-run command block so the user
can quickly launch the app for visual testing in the worktree:

```
To test manually, run:

cd <worktree-absolute-path>
$env:RUST_LOG="warn"; cargo run --release -- example.sldshow
```

Replace `<worktree-absolute-path>` with the actual absolute path to the
worktree directory (e.g., `D:\git\sldshow2\.agent-worktrees\feat-issue-42-ambient-fit-shader`).

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
4. **Pattern B**: Send the PR URL to the Team Lead via `SendMessage`.

## 6. Cleanup (Optional)

When the user (Pattern A) or Team Lead (Pattern B) requests cleanup after the
PR is merged:

1. Return to the main repo root.
2. Remove the worktree and branch:
   ```bash
   git worktree remove .agent-worktrees/<type>-issue-<N>-<description>
   git branch -d <type>/issue-<N>-<description>
   ```
   Use `git branch -D` only if the user explicitly requests force deletion.
3. **Pattern B**: Do not remove worktrees unless the Team Lead instructs you to.

## Team Operation Flow (Reference)

```
1. Team Lead: TeamCreate → TaskCreate (per issue) → Task spawn (issue-slayer agent)
2. Each Teammate: Issue claim → plan → Lead approval → implement → verify → PR
3. Team Lead: Coordinate PR merge order → instruct agents to rebase if needed
4. Team Lead: shutdown_request → TeamDelete
```
