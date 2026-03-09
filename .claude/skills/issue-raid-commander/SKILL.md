---
name: issue-raid-commander
description: >
  Analyzes agent:ready issues for merge conflicts and outputs a sprint plan
  for the team lead to act on. Use before spawning issue-slayer agents to
  avoid parallel work collisions. Does NOT spawn agents or touch code.
---

# Issue Raid Commander

Battlefield awareness without intervention. Assess the ready queue, detect
collisions, and hand the team lead a plan they can execute immediately.

## Inputs

```bash
gh issue list --label "agent:ready" \
  --search "no:assignee state:open -label:pending" \
  --json number,title,labels,body --limit 50
```

Also read `AGENTS.md` for the list of conflict-prone files.

## Analysis

For each issue, estimate which files it will touch. Use:

- The issue body (issue-ranger usually notes affected files)
- The issue title and labels as hints
- Your own knowledge of the codebase

Flag any two issues that are likely to touch the same file. Issues touching
conflict-prone files (`app.rs`, `main.rs`, `Cargo.toml`, `config.rs`) warrant
extra scrutiny.

If you are genuinely uncertain whether two issues conflict, treat them as
conflicting and say so.

## Output

Tell the team lead what they need to act:

- Any **blocking conflicts**: what blocks what, which file, suggested order
- **Merge order** for conflicting PRs only
- If nothing conflicts, say so in one line

Choose the format that fits the situation. Prefer brevity. A table is
appropriate for many issues; a sentence is fine for two.

### Bundle PR candidates

Flag groups of issues as `bundleable` when they meet the criteria in
`AGENTS.md` (same pattern, small, no file conflicts, reviewable as one unit).
Example output:

```
Bundle candidate: #178, #180, #182, #183
  Pattern: unwrap → error propagation
  Files: image_loader.rs, config.rs, overlay.rs, osc.rs (no overlap)
  → Assign to a single Slayer
```

The Slayer will create one commit per issue and a single PR.
