---
name: add-rule
description: "This skill should be used when the user asks to 'add a rule', 'detect this pattern', 'catch this', 'false positive', 'false negative', 'not catching', 'wrong detection', 'contribute a rule', or wants to improve detection coverage in code-guardrails. Covers new rule creation, false positive fixes, and false negative fixes."
---

# Add Rule — Contribute a Detection Rule

From pattern discovery to tested PR in 7 steps. Covers new rules, false positive fixes, and false negative fixes.

## Flow

```
1. Understand what was found
2. Set up development environment (first-time contributors)
3. Write the rule YAML
4. Write fixtures (should_fail / should_pass / approved)
5. Run tests & verify
6. Update Rust candidate selection (if needed)
7. Commit & open PR
```

## Step 1: Understand What Was Found

Identify which situation applies, then gather details.

### A. New rule — "I found a pattern that should be caught"

Collect:
1. **The actual code** (the NG example)
2. **Language** — Python / TypeScript / both
3. **Policy group** — `test-double` or `fallback`
4. **Approval model**:
   - `adjacent_policy_comment` — approvable with `policy-approved: REQ-xxx` comment (typical for fallback rules)
   - `none` — never approvable (always for test-double; also for categorically unsafe fallbacks)
5. **OK examples** — similar syntax that should NOT be flagged

### B. False negative — "This code should be caught but isn't"

Collect:
1. **The code** that slipped through
2. **Which rule** was expected to catch it
3. **Why it should be caught**

### C. False positive — "This code is flagged but shouldn't be"

Collect:
1. **The code** wrongly flagged
2. **Which rule** flagged it
3. **Why it's actually OK**

### False positive prevention (always verify)

Past incident: `||` / `??` originally matched everywhere (conditions, returns), causing massive false positives. They had to be restricted to assignments only.

Always check:
- "Match only in assignments? Or also in conditions / return statements?"
- "Are there library/framework APIs that use the same syntax innocently?" (e.g. `router.get("/path")` vs `dict.get()`)
- "Exclude generated code (`**/generated/**`) and test files as usual?"

## Step 2: Set Up Development Environment

Skip if already working inside the code-guardrails repo.

```bash
git clone https://github.com/<username>/code-guardrails.git
cd code-guardrails
./setup
git checkout -b feat/add-<rule-name>
```

Verify: `cargo test` — all existing tests pass.

## Step 3: Write the Rule YAML

Create a file in `rules/` following the naming convention `{lang}-no-{policy_group}-{pattern-name}.yml`.

Consult **`references/rule-patterns.md`** for:
- Python and TypeScript rule templates
- ast-grep pattern syntax reference
- Pattern templates by type (assignment, function call, block structure, identifier, multiple variants)
- Principles to reduce false positives

## Step 4: Write Fixtures

Consult **`references/fixtures.md`** for:
- Directory layout (`fixtures/{language}/{policy_group}/`)
- `should_fail`, `should_pass`, and `approved` examples
- Fixing existing rules (false negative / false positive workflows)

## Step 5: Run Tests & Verify

```bash
cargo test
```

Every `should_fail` fixture must be detected. Every `should_pass` fixture must have zero findings.

Smoke test on a real project (optional but recommended):
```bash
./bin/code-guardrails-engine scan-tree --root /path/to/project --config-dir .
```

When tests fail:
- **False negative**: Broaden the pattern. Add variants with `any:`.
- **False positive**: Narrow the pattern. Restrict to assignments, add ignores, use `kind:`.
- **Debug a specific file**: `ast-grep scan --json=stream fixtures/path/to/file.py`

## Step 6: Update Rust Candidate Selection (if needed)

Check `detect_rule_ids` in `src/main.rs` — the Rust-side candidate selector that decides whether a file needs an `ast-grep` scan. It should stay looser than the actual rule patterns.

```rust
if lower.contains("your_keyword") {
    ids.insert("your-rule-id".to_string());
}
```

If existing keywords already cover the new rule, no change is needed.

## Step 7: Commit & Open PR

Commit message format:
- New rule: `feat: add {rule-id} rule — detect {what it catches}`
- Fix: `fix: reduce false {positives|negatives} in {rule-id} — {what changed}`

```bash
git add rules/ fixtures/ src/main.rs
git commit
git push -u origin feat/add-<rule-name>
gh pr create
```

## Additional Resources

- **`references/rule-patterns.md`** — Rule YAML templates, ast-grep pattern syntax, false positive reduction principles
- **`references/fixtures.md`** — Fixture directory layout, examples for each type, existing rule fix workflows
