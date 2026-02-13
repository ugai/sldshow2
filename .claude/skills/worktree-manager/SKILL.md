---
name: worktree-manager
description: >
  Create, list, and clean up isolated agent worktrees under `.agent-worktrees/`.
  Use this when an agent needs a safe, isolated working directory.
  This skill does NOT edit code or run tests.
---

# Worktree Manager

Manage isolated git worktrees for AI agents under `.agent-worktrees/`.

> **Boundary rule**: This skill only manages worktree directories.
> It never edits source code, runs tests, or touches anything outside `.agent-worktrees/`.

## 1. Create Worktree

**Input**: `agent_id` (alphanumeric, e.g., `copilot01`)

1. **Validate**: `agent_id` must match `[a-zA-Z0-9]+`.
2. **Collision check**: If `.agent-worktrees/agent-<agent_id>-*` already exists, **abort**.
   Do not reuse existing worktrees.
3. **Create** the temporary worktree and branch:
   ```bash
   git worktree add .agent-worktrees/agent-<agent_id>-wip -b agent-<agent_id>-wip origin/main
   ```
4. After the agent selects an issue, it will create the final branch inside the worktree:
   ```bash
   git switch -c agent-<agent_id>-<task>
   ```
5. **Return** the absolute path to the new worktree.

## 2. List Worktrees

```bash
git worktree list
```

## 3. Cleanup Worktree

1. **Safety check**: Only delete if the branch is fully merged, or the user
   explicitly requests force deletion.
2. Remove the worktree:
   ```bash
   git worktree remove .agent-worktrees/agent-<agent_id>-<task>
   ```
3. Delete the branch:
   ```bash
   git branch -d agent-<agent_id>-<task>   # use -D only if force-requested
   ```

## Housekeeping

Ensure `.agent-worktrees/README.md` exists with the following content:

```
This directory is managed by AI agents. Do not edit manually.
```
