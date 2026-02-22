---
name: verify-sprint
description: >
  Batch-verifies multiple sprint PR branches by merging them into a local
  ephemeral verify branch and running a combined visual check before
  squash-merging each PR into main. The verify branch is never pushed to
  remote. Use after a sprint where multiple issue-slayer agents have opened
  PRs, or whenever the user wants to verify several PRs together before merging.
---

# Verify Sprint

## Workflow checklist

Copy and track progress:

```
- [ ] Step 1: Identify PR branches
- [ ] Step 2: Fetch and create verify branch
- [ ] Step 3: Merge all PR branches
- [ ] Step 4: Visual verification (user)
- [ ] Step 5: Merge PRs into main
- [ ] Step 6: Discard verify branch
```

## Step 1 — Identify PR Branches

If the user hasn't provided PR numbers, list open PRs:

```bash
gh pr list --state open --json number,title,headRefName
```

Confirm with the user which PRs to include.

## Step 2 — Fetch and Create Verify Branch

**NEVER push this branch to remote.**

```bash
git fetch origin
git checkout -b verify/sprint-$(date +%Y-%m-%d) main
```

If the branch name already exists, append `-2`, `-3`, etc.

## Step 3 — Merge All PR Branches

```bash
git merge origin/<branch-1> origin/<branch-2> origin/<branch-3> ...
```

If the octopus merge fails due to conflicts, fall back to sequential merges.
Report any conflicts to the user before proceeding.

## Step 4 — Visual Verification

Tell the user to run:

```bash
cargo run --release -- test.sldshow
```

Ask: *"Visual check complete — did everything look correct? (yes / issue found)"*

**If an issue is found:**

1. Identify the PR branch responsible.
2. User or agent adds fix commits to that branch.
3. Re-merge and re-check:

```bash
git merge origin/<fixed-branch>
# → user runs cargo run --release again
```

Repeat until the user confirms no issues.

## Step 5 — Merge PRs into Main

```bash
gh pr merge <N> --squash --delete-branch
```

Use the Raid Commander's recommended order if available; otherwise merge fixes
before features that depend on them.

## Step 6 — Discard Verify Branch

```bash
git checkout main
git branch -D verify/sprint-<date>
git pull origin main
```
