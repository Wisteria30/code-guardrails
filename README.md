# ast-grep starter pack for banning test doubles and unapproved fallbacks

This pack treats your policy as **linting rules** backed by ast-grep, with a tiny wrapper to handle explicit fallback approvals.

## Why this shape?

- ast-grep is a strong fit for syntax-level bans across many languages.
- It is *not* truly one-rule-for-all-languages. Patterns must still be valid syntax for each language.
- The clean design is:
  - one shared policy taxonomy (`test-double`, `fallback`)
  - one rule pack per language
  - one thin wrapper that enforces approval comments for fallback rules

## Approval model

Fallback rules in this pack carry:

```yaml
metadata:
  policy_group: fallback
  approval_mode: adjacent_policy_comment
```

The wrapper (`check_policy.py`) suppresses a fallback finding **only** when there is an adjacent comment like:

```py
# policy-approved: REQ-123 explicit locale default
lang = payload.get("lang", "ja-JP")
```

```ts
// policy-approved: ADR-7 explicit demo-mode fallback
const label = apiValue ?? "demo";
```

Test-double rules have:

```yaml
metadata:
  policy_group: test-double
  approval_mode: none
```

So they always fail outside tests.

## Usage

```bash
# run ast-grep directly
ast-grep scan .

# CI / stricter policy wrapper
python check_policy.py .
```

## Files vs tests

Each rule excludes common test paths with `ignores:`. Tweak these globs for your repo layout.

## Important note

These rules are a **starter pack**. They were written against ast-grep's documented rule syntax, but they were not executed in this environment because the ast-grep CLI was not installed here. Run `ast-grep test` and the playground locally, then tune patterns and globs for your codebase.
