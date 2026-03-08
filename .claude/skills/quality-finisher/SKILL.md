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
5. **Always** post a `## Quality Finisher Report` comment (all outcomes)
6. Report outcome to user

## Decision Matrix

| Situation | Action |
|---|---|
| Writable tests exist | Checkout PR branch in a worktree, implement tests, push, then post report |
| Tests not writable yet (missing infra, etc.) | Post structured comment — gap, blocker, unblock path |
| Coverage already sufficient | Post short confirmation comment, done |
| Blocker requires a source-level change | Open a new `agent:proposed` issue, post link on PR |

> **Every outcome requires a `## Quality Finisher Report` PR comment.**
> `verify-sprint` uses this comment as the signal that auditing is complete.

## Writing Tests

When tests are writable, check out the PR branch in an isolated worktree.
Always quote branch names when substituting into shell commands:

```bash
git fetch origin
git worktree add -b "<branch-name>" .agent-worktrees/quality-finisher-pr-<N> "origin/<branch-name>"
```

Write tests. Follow the project's existing test conventions:

- Prefer `#[cfg(test)]` modules inline in `src/*.rs` over separate integration
  test binaries
- Use existing test helpers and fixtures where available

Commit and push. Write the commit message to a file to handle multi-line bodies
cleanly. Use the co-authorship trailer format defined in `AGENTS.md`:

```bash
git add <test-files>
cat > /tmp/qf_commit_msg.txt << 'EOF'
test: add coverage for <description>

Ref #<issue-number>

Co-Authored-By: {model} ({tool}) <email from AGENTS.md>
EOF
git commit -F /tmp/qf_commit_msg.txt
git push origin "<branch-name>"
```

Remove the worktree after pushing:

```bash
git worktree remove .agent-worktrees/quality-finisher-pr-<N>
```

Then post a report comment (see template below).

## Report Comment Templates

Write all report comments to a temp file first (see [ENV.md](ENV.md)):

```bash
cat > /tmp/qf_comment_<N>.md << 'EOF'
<report content>
EOF
gh pr comment <N> --body-file /tmp/qf_comment_<N>.md
```

### Tests Pushed

```markdown
## Quality Finisher Report

**Status**: Tests pushed

**Added**: <N> test cases covering <what was covered>
```

### Tests Not Writable

```markdown
## Quality Finisher Report

**Status**: Coverage gap — tests not yet writable

**Gap**: <what is not covered>

**Blocker**: <reason — e.g., requires a test harness that does not exist yet>

**Unblock path**: <what needs to happen first>

**Recommended next step**: <specific action>
```

### Coverage Sufficient

```markdown
## Quality Finisher Report

**Status**: Coverage sufficient — no additional tests needed.
```

### Prerequisite Required

```markdown
## Quality Finisher Report

**Status**: Prerequisite required

A source-level change is needed before tests can be written for this PR.
New issue opened: #<new-issue-number>
```

## Opening a Prerequisite Issue

When a source-level change is required before tests can be written:

```bash
gh issue create \
  --title "<type>: <description>" \
  --body-file /tmp/qf_prereq_<N>.md \
  --label "enhancement,agent:proposed"
```

## Summary Report

After all PRs are processed, output a summary to the user:

```
Quality Finisher — Summary
PR #A: tests pushed (N new test cases)
PR #B: comment posted (gap: X, blocker: Y)
PR #C: coverage sufficient
```
