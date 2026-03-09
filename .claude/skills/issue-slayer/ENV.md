# Environment-Specific Commands

## Tool Name

When filling in `<tool-name>` in the claim comment, use the name of the tool you
are currently running under:

| Environment       | Tool name         |
|-------------------|-------------------|
| Claude Code CLI   | `Claude Code`     |
| GitHub Copilot    | `GitHub Copilot`  |
| Gemini CLI        | `Gemini CLI`      |
| Antigravity       | `Antigravity`     |

## Plan Approval (Pattern A)

Present the implementation plan and request user approval before writing code.

| Tool | How |
|------|-----|
| Claude Code | `EnterPlanMode` |
| Other | Output the plan as text and ask the user to confirm |

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
| Send plan for approval | `SendMessage` to Team Lead | Use available messaging tool |
| Notify PR ready | `SendMessage` to Team Lead with PR URL | Use available messaging tool |

## PowerShell: UTF-8 Encoding for `gh`

On Japanese (and other non-English) Windows systems, PowerShell's default encoding
is not UTF-8, which causes issue titles and bodies to be garbled when read or
written via `gh`. Add the following two lines **before** the first `gh` call:

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
```

You only need to do this once per session. Example full sequence:

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
gh issue list --label "agent:ready" ...
```

## Manual Test Command

After verification passes, output the command appropriate for the user's shell:

**PowerShell (Windows)**:
```powershell
$env:RUST_LOG="warn"; cargo run --release -- example.sldshow
```

**bash / zsh (Linux, macOS)**:
```bash
RUST_LOG=warn cargo run --release -- example.sldshow
```
