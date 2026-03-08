# Environment-Specific Notes

## PowerShell: UTF-8 Encoding for `gh`

On Japanese (and other non-English) Windows systems, PowerShell's default
encoding is not UTF-8, which causes issue and PR bodies to be garbled when
written via `gh`. Add the following before the first `gh` call:

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
```

## PR Comments via File

Always write comment bodies to a temp file to avoid shell-escaping issues
with backticks and special Markdown characters:

```bash
cat > /tmp/qf_comment_<N>.md << 'EOF'
## Quality Finisher Report
...
EOF
gh pr comment <N> --body-file /tmp/qf_comment_<N>.md
```
