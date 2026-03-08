---
name: quality-finisher
description: >
  Audits open PRs for test coverage gaps after issue-slayer opens them.
  Writes missing tests and pushes to the PR branch, posts structured coverage
  comments, re-invokes issue-slayer for cases requiring source-level changes,
  or confirms that coverage is already sufficient. Run before verify-sprint.
---

# Quality Finisher

Post-PR test coverage audit. Confirm the kill is real before verify-sprint merges it.

## Inputs

One or more PR numbers provided by the user.

## Workflow

For each PR:

1. Read the PR diff (`gh pr diff <N>`)
2. Identify changed source files and their existing test coverage
3. Diagnose coverage gaps
4. Apply the [Decision Matrix](#decision-matrix)
5. Report outcome to user

## Decision Matrix

| Situation | Action |
|---|---|
| Writable tests exist | Checkout PR branch in a worktree, implement tests, push |
| Tests not writable yet (missing infra, etc.) | Post structured comment — gap, blocker, unblock path |
| Coverage already sufficient | Post short confirmation comment, done |
| Blocker requires a source-level change | Open a new `agent:proposed` issue, post link on PR |

## Writing Tests

When tests are writable, check out the PR branch in an isolated worktree:

```bash
git fetch origin
git worktree add .agent-worktrees/quality-finisher-pr-<N> origin/<branch-name>
```

Write tests. Follow the project's existing test conventions:

- Prefer `#[cfg(test)]` modules inline in `src/*.rs` over separate integration
  test binaries
- Use existing test helpers and fixtures where available

Commit and push:

```bash
git add <test-files>
git commit -m "test: add coverage for <description>

Ref #<issue-number>

Co-Authored-By: <model> (Claude Code) <noreply@anthropic.com>"
git push origin <branch-name>
```

Remove the worktree after pushing:

```bash
git worktree remove .agent-worktrees/quality-finisher-pr-<N>
```

## Structured Comment — Tests Not Writable

Post as a GitHub PR comment. Write to a temp file first (see [ENV.md](ENV.md)):

```markdown
## Quality Finisher Report

**Status**: Coverage gap — tests not yet writable

**Gap**: <what is not covered>

**Blocker**: <reason — e.g., requires a test harness that does not exist yet>

**Unblock path**: <what needs to happen first>

**Recommended next step**: <specific action>
```

## Confirmation Comment — Coverage Sufficient

```markdown
## Quality Finisher Report

**Status**: Coverage sufficient — no additional tests needed.
```

## Opening a Prerequisite Issue

When a source-level change is required before tests can be written:

```bash
gh issue create \
  --title "<type>: <description>" \
  --body-file /tmp/qf_prereq_<N>.md \
  --label "enhancement,agent:proposed"
```

Then post a comment on the original PR linking to the new issue:

```markdown
## Quality Finisher Report

**Status**: Prerequisite required

A source-level change is needed before tests can be written for this PR.
New issue opened: #<new-issue-number>
```

## Summary Report

After all PRs are processed, output a summary to the user:

```
Quality Finisher — Summary
PR #A: tests pushed (N new test cases)
PR #B: comment posted (gap: X, blocker: Y)
PR #C: coverage sufficient
```
