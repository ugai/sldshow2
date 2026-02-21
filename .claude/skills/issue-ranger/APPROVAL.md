# Pattern B — Approval Message Format

When in Team Member mode, send this message to the Team Lead for approval
before creating any issues. See [ENV.md](ENV.md) for the messaging tool to use.

```
Subject: Issue list for review (N issues)

| # | Title | Labels | Perspective | Summary |
|---|-------|--------|-------------|---------|
| - | feat: ... | enhancement, priority:p2 | UX | One-line description |
| - | fix: ...  | bug, priority:p1 | Robustness | One-line description |

Ready to post. Approve all, or tell me which to skip or modify.
```

Wait for an explicit approval message from the Lead before creating any issues.
