# Issue Body Template

Use this template for every issue. Write the body to `issue_body_<N>.md`,
then pass it to `gh issue create --body-file`.

```markdown
## Summary

<1-2 sentences: what the problem is and why it matters>

## Details

<Specific technical approach or requirements. Include relevant file paths,
function names, and line numbers where applicable.>

## Acceptance Criteria

- [ ] <Concrete, testable criterion 1>
- [ ] <Concrete, testable criterion 2>

## Scope Notes

- Estimated files to change: <list>
- Complexity: small | medium

---
> Quest proposed by **<your actual model name>** (Issue Ranger)
```

Replace `<your actual model name>` with your real model name (e.g.,
"Claude Sonnet 4.6", "Claude Opus 4.6"). Write it directly — do not rely
on shell variable expansion.
