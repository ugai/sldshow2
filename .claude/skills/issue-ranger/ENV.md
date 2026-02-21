# Environment-Specific Commands

## Plan Approval (Pattern A)

Present the issue list and request user approval before posting anything.

| Tool | How |
|------|-----|
| Claude Code | `EnterPlanMode` |
| Other | Output the list as text and ask the user to confirm |

## Detecting Team Context (Pattern B)

| Tool | How to detect |
|------|---------------|
| Claude Code | `team_name` parameter present, or assigned via `TaskList`/`TaskUpdate` |
| Other | Check for equivalent task assignment mechanism in your tool |

## Team Coordination (Pattern B)

| Action | Claude Code | Other |
|--------|-------------|-------|
| Detect team context | `team_name` parameter present, or assigned via `TaskList`/`TaskUpdate` | Check for equivalent task assignment mechanism |
| Check teammate activity | Read `TaskList` | Check equivalent task list |
| Send issue list for approval | `SendMessage` to Team Lead | Use available messaging tool |
| Report posted issues | `SendMessage` to Team Lead | Use available messaging tool |

## Writing and Deleting Temp Files

Write the issue body to `issue_body_<N>.md`, then delete it after each issue.

**PowerShell (Windows)**:
```powershell
# Write
[System.IO.File]::WriteAllText("issue_body_<N>.md", $body, [System.Text.Encoding]::UTF8)

# Delete (always run, even on failure)
Remove-Item issue_body_<N>.md -ErrorAction SilentlyContinue
```

**bash / zsh (Linux, macOS)**:
```bash
# Write
printf '%s' "$body" > issue_body_<N>.md

# Delete (always run, even on failure)
rm -f issue_body_<N>.md
```
