---
name: add-rule
description: "Guided workflow to add new detection rules to code-guardrails, fix false positives/negatives, and submit a PR. Use when someone says 'add a rule', 'detect this pattern', 'catch this', 'false positive', 'false negative', 'not catching', 'wrong detection', 'contribute a rule', or wants to improve detection coverage."
---

# Add Rule — Contribute a Detection Rule to code-guardrails

Guide the user from discovering a pattern in their project to submitting a tested PR.
This covers new rules, false positive fixes, and false negative fixes.

## Flow

```
1. Understand what the user found
2. Set up the development environment (first-time contributors)
3. Write the rule YAML
4. Write fixtures (should_fail / should_pass / approved)
5. Run tests & verify
6. Update ripgrep pre-filter (if needed)
7. Commit & open PR
```

---

## Step 1: Understand What the User Found

The user typically arrives from one of three situations. Figure out which one, then gather the details.

### A. "I found a pattern that should be caught" (new rule)

Ask for:
1. **The actual code** they saw in their project (the NG example)
2. **Language** — Python / TypeScript / both
3. **Policy group** — `test-double` or `fallback`
4. **Approval model**:
   - `adjacent_policy_comment` — can be approved with a `policy-approved: REQ-xxx` comment (typical for fallback rules)
   - `none` — never approvable. Always use for test-double. Also use for fallback patterns that are categorically unsafe (e.g. disabling logging entirely, disabling security features)
5. **OK examples** — similar syntax that should NOT be flagged. Ask specifically about this.

### B. "This code should be caught but isn't" (false negative)

Ask for:
1. **The code** that slipped through
2. **Which rule** they expected to catch it (or let them describe the violation)
3. **Why it should be caught** — what makes it a bad pattern

### C. "This code is flagged but it shouldn't be" (false positive)

Ask for:
1. **The code** that was wrongly flagged
2. **Which rule** flagged it
3. **Why it's actually OK** — what distinguishes it from the real violations

### Questions to Prevent False Positives (always ask)

Past incident: `||` / `??` were originally matching everywhere (conditions, returns), causing massive false positives. They had to be restricted to assignments only.

Always verify:
- "Should this only match in assignments? Or also in conditions / return statements?"
- "Are there library/framework APIs that use the same syntax innocently?"
  (Example: `router.get("/path")` is not `dict.get()` with a default)
- "Exclude generated code (`**/generated/**`) and test files as usual?"

---

## Step 2: Set Up Development Environment

If the user is already working inside the code-guardrails repo, skip this step.

For first-time contributors:

```bash
# Fork on GitHub, then:
git clone https://github.com/<your-username>/code-guardrails.git
cd code-guardrails
./setup                          # installs ast-grep and ripgrep if missing; builds the Rust engine
git checkout -b feat/add-<rule-name>
```

Verify the environment works:
```bash
python3 -m pytest tests/ -v     # all existing tests should pass
```

---

## Step 3: Write the Rule YAML

Create a file in `rules/`.

### Naming convention

```
{lang}-no-{policy_group}-{pattern-name}.yml
```

Examples: `py-no-fallback-bool-or.yml`, `ts-no-empty-catch.yml`, `py-no-test-double-identifier.yml`

### Template

**For Python rules:**
```yaml
id: py-no-{policy_group}-{pattern-name}
language: Python
severity: error
files:
  - '**/*.py'
ignores:
  - '**/test/**'
  - '**/tests/**'
  - '**/*_test.py'
  - '**/test_*.py'
  - '**/conftest.py'
  - '**/generated/**'
rule:
  pattern: <ast-grep pattern here>
message: '<what was detected>'
note: '<why it is a problem and how to fix it>'
metadata:
  policy_group: fallback
  approval_mode: adjacent_policy_comment
```

**For TypeScript rules:**
```yaml
id: ts-no-{policy_group}-{pattern-name}
language: TypeScript
severity: error
files:
  - '**/*.ts'
  - '**/*.cts'
  - '**/*.mts'
ignores:
  - '**/test/**'
  - '**/tests/**'
  - '**/*.test.ts'
  - '**/*.spec.ts'
  - '**/__tests__/**'
  - '**/generated/**'
rule:
  pattern: <ast-grep pattern here>
message: '<what was detected>'
note: '<why it is a problem and how to fix it>'
metadata:
  policy_group: fallback
  approval_mode: adjacent_policy_comment
```

### ast-grep Pattern Reference

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$VAR` | Single node (variable, expression) | `$VAR = $A or $B` |
| `$$$BODY` | Multiple nodes (list of statements) | `try: $$$BODY` |
| `any:` | OR — match any of the listed patterns | See below |
| `kind:` + `regex:` | AST node type + regex | Identifier matching |

### Pattern Templates by Type

**Assignment pattern** (common for fallback detection):
```yaml
rule:
  pattern: $VAR = $A or $B
```

**Function call pattern** (dangerous API calls):
```yaml
rule:
  pattern: logging.disable($LEVEL)
```

**Block structure pattern** (try/catch):
```yaml
rule:
  pattern: |
    try:
      $$$BODY
    except $ERR:
      pass
```

**Identifier pattern** (naming violations):
```yaml
rule:
  kind: identifier
  regex: '(?i)(mock|stub|fake)'
```

**Multiple variants** (use `any:` when one pattern isn't enough):
```yaml
rule:
  any:
    - pattern: $VAR = $A || $B
    - pattern: const $VAR = $A || $B
    - pattern: let $VAR = $A || $B
    - pattern: var $VAR = $A || $B
```

### Principles to Reduce False Positives

1. **Restrict to assignments**: Use `$VAR = $A || $B` instead of `$A || $B` (excludes conditions)
2. **Include declaration variants**: In TS, `const/let/var` are separate AST patterns — list them all
3. **Filter by argument count**: `$OBJ.get($KEY)` (1 arg = OK) vs `$OBJ.get($KEY, $DEFAULT)` (2 args = NG)
4. **Consider call context**: `router.get()` vs `dict.get()` look the same syntactically

---

## Step 4: Write Fixtures

Fixtures go under `fixtures/{language}/{policy_group}/`. The test runner automatically picks them up.

```
fixtures/
  python/         (or typescript/)
    fallback/     (or test-double/)
      should_fail/    — code that MUST be detected (at least 1 file)
      should_pass/    — code that must NOT be detected (at least 1 file)
      approved/       — code with policy-approved comment (only if approval_mode is adjacent_policy_comment)
```

### should_fail

Write the minimal code that the rule must catch. Name the file after the pattern.

```python
# fixtures/python/fallback/should_fail/dict_get_default.py
config = {}
timeout = config.get("timeout", 30)
```

### should_pass (most important — this prevents false positives)

Write code that uses similar syntax but should NOT be flagged. Add a comment explaining why.

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

### approved (only when `approval_mode: adjacent_policy_comment`)

Skip if `approval_mode: none`.

```python
# fixtures/python/fallback/approved/approved_get_default.py
# policy-approved: REQ-123 intentional default for locale
lang = payload.get("lang", "ja-JP")
```

---

## Step 5: Run Tests & Verify

### 5a. Unit tests

```bash
python3 -m pytest tests/test_rules.py -v
```

Every `should_fail` fixture must be detected. Every `should_pass` fixture must have zero findings.

### 5b. Smoke test on a real project (optional but recommended)

```bash
./bin/code-guardrails-engine scan-tree --root /path/to/your/project --config-dir . --format human
```

Check for false positives in real code. If too many appear, tighten the pattern.

### 5c. When tests fail

- **should_fail not detected (false negative)**: Broaden the pattern. Add variants with `any:`.
- **should_pass detected (false positive)**: Narrow the pattern. Restrict to assignments, add ignores, use `kind:` to limit AST node type.
- **Debug a specific file**:
  ```bash
  ast-grep scan --json=stream fixtures/path/to/file.py
  ```

---

## Step 6: Update Rust Candidate Selection (if needed)

Check `detect_rule_ids` in `src/main.rs`. This is the Rust-side candidate selector that decides whether a file needs an `ast-grep` scan, so it should stay looser than the actual rule patterns.

```rust
if lower.contains("your_keyword") {
    ids.insert("your-rule-id".to_string());
}
```

If your rule's keywords are already covered, no change is needed.
If not, add a broad-enough selector so the engine still sends candidate files to `ast-grep`.

---

## Step 7: Commit & Open PR

### Commit message format

New rule:
```
feat: add {rule-id} rule — detect {what it catches}
```

Fix:
```
fix: reduce false {positives|negatives} in {rule-id} — {what changed}
```

### Open the PR

```bash
git add rules/ fixtures/ src/main.rs  # only files you changed
git commit
git push -u origin feat/add-<rule-name>
```

PR body template:
```markdown
## Summary
- Add `{rule-id}` rule to detect {pattern description}
- Add fixtures: {N} should_fail, {N} should_pass{, {N} approved}
- Update ripgrep pre-filter (if applicable)

## Motivation
{What you found in your project / why this pattern matters}

## Test plan
- [x] `pytest tests/test_rules.py -v` — all pass
- [ ] Smoke test on a real project
```

Create the PR with `gh pr create` or via GitHub web UI.

---

## Fixing Existing Rules

This skill also covers fixes to existing rules.

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
