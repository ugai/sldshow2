---
name: dispatching-guild-expedition
description: >
  Orchestrates a full sprint: Rangers scout in parallel, user approves issues,
  Raid Commander detects conflicts, then Slayers implement in parallel.
  Use for a complete autonomous development cycle from issue discovery to open
  PRs. Does NOT merge PRs.
---

# Dispatching Guild Expedition

Full pipeline: Scout → Approve → Analyze → Implement.

## Step 0 — Focus Confirmation

Before spawning anything, ask the user one question:

> "Which perspectives should Rangers focus on? (Leave blank for defaults)"

The six perspectives from RECON.md, in priority order:

1. Robustness & Error Handling
2. Code Quality & Architecture
3. Performance
4. User Experience
5. Cross-Platform & Compatibility
6. New Features (small scope only)

**Default split across 4 Rangers:**

| Ranger | Perspectives |
|--------|-------------|
| ranger-1 | 1 — Robustness & Error Handling |
| ranger-2 | 2 — Code Quality & Architecture |
| ranger-3 | 3 — Performance + 4 — User Experience |
| ranger-4 | 5 — Cross-Platform + 6 — New Features |

If the user specifies different focuses, redistribute accordingly.

## Step 1 — Scout (Ranger × 4, parallel)

Create a team. Spawn 4 `issue-ranger` agents in **Pattern B** (Team Member mode).
Pass each agent's assigned perspective(s) in their task prompt:

> "Scout from these perspectives only: [list]. Run as Pattern B — send your
> vetted candidate list to the Team Lead (me) before posting anything."

Wait until all 4 Rangers have reported their candidate lists.

## Step 2 — Aggregate & Approve

Collect the 4 reports. Before showing the user:

- **Deduplicate**: same underlying problem from multiple Rangers → keep the
  best-scoped version; notify the relevant Ranger(s) to drop duplicates.
- **Merge**: near-identical candidates → combine into one issue.

Present the consolidated list to the user with `AskUserQuestion`
(`multiSelect: true`). Each option: `[Ranger] title — one-line summary`.

Based on the user's selection:
1. Tell each Ranger which issues to post and which to skip.
2. Ask the user which posted issues should receive `agent:ready` immediately,
   or handle it as part of the ranger Ready Seal step.

Wait for all Rangers to confirm posting before proceeding.

## Step 3 — Analyze (Raid Commander)

Spawn an `issue-raid-commander` agent as a subagent via the Task tool.
Pass it the current `agent:ready` queue. Present its conflict analysis
output to the user.

If the queue is empty (no `agent:ready` issues), skip to summary and stop.

## Step 4 — Implement (Slayer × N)

Spawn `issue-slayer` agents based on the Raid Commander's sprint plan:

- **Non-conflicting issues**: spawn all in parallel, up to 8 agents.
- **Conflicting groups**: spawn in Raid Commander's recommended serial order —
  wait for each PR to open before spawning the next in the chain.

All Slayers run in **Pattern B** (Team Member mode). Coordinate merge order
per the Raid Commander's analysis. When all Slayers have opened their PRs,
shut down the team and report the sprint summary.

## Step 5 — Hand-off

End the summary with a clear next-step prompt:

```
## Next Step

All PRs are open. Run `verify-sprint` to batch-verify and squash-merge them:

> /verify-sprint

This will merge each PR branch locally, run the quality gate, let you
visually inspect the result, then squash-merge to main.
```
