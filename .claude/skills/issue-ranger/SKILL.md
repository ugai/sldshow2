---
name: issue-ranger
description: >
  Scouts the codebase for bugs, improvement opportunities, and missing features,
  then posts well-scoped GitHub Issues for Issue Slayers to pick up. Use when
  asked to find new issues, propose quests, scout the codebase, or populate the
  issue board. After posting, offers to add agent:ready to selected issues.
---

# Issue Ranger

Range the codebase, gather intel, and post well-scoped issues on the board
for Issue Slayers to pick up.

## Guild Rules

- Post **5–15 issues per shift** — enough to keep Slayers busy, not so many
  the guild master can't review them.
- **Never** add `agent:ready` during scouting or issue creation — only add it
  in the Ready Seal step (Step 8) when explicitly approved by the user.
- **Never** assign issues — Slayers choose their own quests.
- Issue titles and bodies in **English only**.
- **Always** tag issues with `agent:proposed`.

## Execution Mode

**Pattern A (Standalone)**: Present the issue list for user approval before
posting anything.

**Pattern B (Team Member)**: Send the issue list to the Team Lead for approval
before posting. See [APPROVAL.md](APPROVAL.md) for the message format.

Detection: see [ENV.md](ENV.md) for how to detect team context in your tool.

See [ENV.md](ENV.md) for tool-specific commands.

## Workflow

### 1. Survey the Board

Load context and avoid duplicates:

```bash
gh issue list --state open --json number,title,body,labels --limit 100
gh issue list --state closed --json number,title,body --limit 50
```

Also read `CLAUDE.md`, `Cargo.toml`, and `example.sldshow`.

### 2. Scout the Codebase

Read all files under `src/`. Range across six perspectives — stop when you
have 15 candidate ideas. See [RECON.md](RECON.md) for what to look for in
each perspective, in priority order:

1. Robustness & Error Handling
2. Code Quality & Architecture
3. Performance
4. User Experience
5. Cross-Platform & Compatibility
6. New Features (small scope only — max 2–3 files, no new subsystems)

### 3. Gather External Intel

Run **at most 4 web searches** for inspiration (rival tools, wgpu/winit
patterns, WGSL shader ideas, Rust image crates). Stop after 4 regardless of
coverage.

### 4. Vet the List

For each candidate:

- **Duplicate?** — matches an existing issue by problem (not just title) → skip
- **Overlap?** — too similar to another candidate → merge or split
- **Epic?** — touches more than 3–4 files or takes more than a few hours →
  break down
- **Vague?** — no concrete, testable outcome → sharpen or discard

### 5. Approval Gate

Present the vetted list (title, labels, one-line summary, scope estimate) and
wait for approval. Only post approved issues.

### 6. Post Issues

For each approved issue:

1. Write the body to a uniquely named temp file (`issue_body_<N>.md`) —
   use your native file-write tool if available, otherwise see [ENV.md](ENV.md)
2. Create the issue via `gh issue create --body-file issue_body_<N>.md`
3. Delete the temp file immediately — even on failure

On `gh issue create` failure: log the error, skip the issue, report in the
final summary. Do not retry.

After the **first issue** posts successfully, verify rendering:

```bash
gh issue view <number> --json body
```

If formatting is broken, fix the template before continuing.

**Issue body template** — see [TEMPLATE.md](TEMPLATE.md).

**Labels**: always `agent:proposed` + one of `enhancement`/`bug` + optional
`priority:p0`–`p3` (omit = p2). Do **not** add `agent:ready` here — that
happens in Step 8 only.

**Title format**: Conventional Commits — `feat:`, `fix:`, `refactor:`,
`perf:`, `chore:`

### 7. Report

```
## Issues Posted

| # | Title | Labels | Perspective |
|---|-------|--------|-------------|
| 42 | feat: ... | enhancement, priority:p2 | UX |
| 43 | fix: ...  | bug, priority:p1 | Robustness |

Posted: N  |  Skipped: M (duplicates/out of scope)  |  Failed: K
```

### 8. Ready Seal

After the report, offer the user a chance to immediately mark posted issues as
`agent:ready` — eliminating the separate manual labeling step.

**Pattern A (Standalone)**: Use `AskUserQuestion` with `multiSelect: true`.
Present each successfully posted issue as an option (label: `#N — title`).
For each selected issue, run:

```bash
gh issue edit <number> --add-label "agent:ready"
```

If the user selects none, skip silently — do not ask again.

**Pattern B (Team Member)**: Append to your report message to the Team Lead:

> Which of these should receive `agent:ready`? Reply with issue numbers
> (comma-separated), or "none" to skip.

Wait for the Team Lead's reply, then apply labels. See [APPROVAL.md](APPROVAL.md).
