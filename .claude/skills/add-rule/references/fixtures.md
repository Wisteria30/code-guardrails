# Fixture Structure & Examples

## Directory Layout

Fixtures go under `fixtures/{language}/{policy_group}/`. The test runner automatically picks them up.

```
fixtures/
  python/         (or typescript/)
    fallback/     (or test-double/)
      should_fail/    — code that MUST be detected (at least 1 file)
      should_pass/    — code that must NOT be detected (at least 1 file)
      approved/       — code with policy-approved comment (only if approval_mode is adjacent_policy_comment)
```

## should_fail

Minimal code that the rule must catch. Name the file after the pattern.

```python
# fixtures/python/fallback/should_fail/dict_get_default.py
config = {}
timeout = config.get("timeout", 30)
```

## should_pass (most important — prevents false positives)

Code that uses similar syntax but should NOT be flagged. Add a comment explaining why.

```python
# fixtures/python/fallback/should_pass/decorator_get.py
# router.get() and similar method calls should NOT trigger
# (they are not dict.get with a default value)

@router.get("/health")
def health():
    return {"status": "ok"}
```

```typescript
// fixtures/typescript/fallback/should_pass/conditionalOr.ts
// || in conditions should NOT trigger (assignment-only rule)
if (!COGNITO_DOMAIN || !COGNITO_CLIENT_ID) {
  throw new Error("Missing config");
}
```

## approved (only when `approval_mode: adjacent_policy_comment`)

Skip if `approval_mode: none`.

```python
# fixtures/python/fallback/approved/approved_get_default.py
# policy-approved: REQ-123 intentional default for locale
lang = payload.get("lang", "ja-JP")
```

## Fixing Existing Rules

### False Negative Fix (pattern not caught)

1. Get the code sample that should be caught
2. Add it to `should_fail/` as a new fixture
3. Run `pytest tests/test_rules.py` — confirm it fails (the fixture is not detected)
4. Edit the rule YAML — add variants with `any:`, broaden the pattern
5. Run tests again — confirm the new fixture passes AND existing `should_pass` fixtures still pass
6. Commit with `fix: reduce false negatives in {rule-id}`

### False Positive Fix (innocent code flagged)

1. Get the code sample that was wrongly flagged
2. Add it to `should_pass/` as a new fixture
3. Run `pytest tests/test_rules.py` — confirm it fails (the fixture is detected)
4. Edit the rule YAML:
   - Restrict pattern to assignments (`$VAR = $A ?? $B`)
   - Add paths to `ignores`
   - Use `kind:` to limit AST node type
5. Run tests again — confirm the new fixture passes AND existing `should_fail` fixtures still pass
6. Commit with `fix: reduce false positives in {rule-id}`
