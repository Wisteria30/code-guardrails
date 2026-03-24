# code-guardrails

Claude Code plugin that detects test doubles and unapproved fallbacks in production code. AI coding tools silently introduce mock/stub/fake objects and fallback behaviors — this plugin catches them.

## Install

```bash
# Project-local (recommended)
git clone https://github.com/Wisteria30/code-guardrails .claude/plugins/code-guardrails

# Or user-global
git clone https://github.com/Wisteria30/code-guardrails ~/.claude/plugins/code-guardrails

# Verify dependencies
.claude/plugins/code-guardrails/setup
```

Requires: [ast-grep](https://ast-grep.github.io/), [ripgrep](https://github.com/BurntSushi/ripgrep), Python 3.10+

## What it does

**PostToolUse hook** — After every Edit/Write, automatically scans the changed file and warns Claude if violations are found.

**`/scan` command** — Full project scan on demand.

## Two policies

### 1. No test doubles in production
- `mock`, `stub`, `fake` identifiers banned outside test files
- `unittest.mock` imports banned outside test files
- No exceptions.

### 2. No unapproved fallbacks
- Default values in `.get()`, `getattr()`, `next()`, `os.getenv()` flagged
- `or` fallbacks in assignments (`x = a or b`) flagged
- `??`, `||`, `??=`, `||=` in TypeScript flagged
- `except: pass`, `contextlib.suppress`, empty catch blocks flagged
- Promise `.catch(() => default)` flagged

### Approval model

To approve an intentional fallback, add a comment within 2 lines above:

```python
# policy-approved: REQ-123 explicit locale default
lang = payload.get("lang", "ja-JP")
```

```typescript
// policy-approved: ADR-7 explicit demo-mode fallback
const label = apiValue ?? "demo";
```

Prefix must be `REQ-`, `ADR-`, or `SPEC-` followed by an identifier.

## CLI usage

```bash
# Full project scan
python check_policy.py .

# Single file scan
python check_policy.py --changed-only path/to/file.py

# JSON output (for CI/hooks)
python check_policy.py --changed-only file.py --format json
```

## Rules

17 rules: 9 Python + 8 TypeScript. All validated against 34+ test fixtures.

Test paths (`**/test/**`, `**/tests/**`, `**/*_test.py`, etc.) are excluded from all rules.

## Development

```bash
# Run rule validation tests
python test_rules.py

# Run CLI tests
python test_check_policy.py
```
