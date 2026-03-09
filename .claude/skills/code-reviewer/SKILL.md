---
name: code-reviewer
description: >
  Reviews a PR diff for correctness, edge cases, and consistency with the
  surrounding codebase. Use after opening a PR and before requesting external
  review. Accepts a PR number as argument.
---

# Code Reviewer

Review a pull request diff with fresh eyes.

## Input

PR number passed as argument (e.g. `/code-reviewer 335`).
If no number is given, list open PRs and ask.

## Procedure

1. Fetch the diff and list of changed files:

```bash
gh pr diff <N>
gh pr view <N> --json title,body,headRefName
```

2. For each changed file, read enough surrounding context (±50 lines around
   each hunk) to understand the change in its environment.

3. Review for:
   - **Correctness** — logic errors, off-by-one, missed error paths
   - **Edge cases** — boundary values, empty/null inputs, overflow
   - **Consistency** — does the change align with how the rest of the codebase
     handles the same concern? Are there parallel code paths that need the same
     update?
   - **API contract** — does the public interface change? Are callers updated?

4. Output a structured review:

```markdown
## Code Review — PR #<N>

**Verdict**: approve | request-changes

### Findings
- [ ] (severity: low|medium|high) <file>:<line> — <description>

### Notes
<optional general observations>
```

## Scope

- Read-only. Do not edit files or push commits.
- Focus on the diff. Do not review unchanged code unless needed for context.
- Keep findings actionable and specific.
