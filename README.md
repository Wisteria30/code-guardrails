# code-guardrails

Claude Code plugin that detects test doubles and unapproved fallbacks in production code. AI coding tools silently introduce mock/stub/fake objects and fallback behaviors — this plugin catches them.

## Requirements

[Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Python](https://www.python.org/) 3.12+, [ast-grep](https://ast-grep.github.io/) 0.14+, [ripgrep](https://github.com/BurntSushi/ripgrep) 14.0+

## Install — 30 seconds

### Option A: Marketplace (recommended)

Open Claude Code and paste this. Claude does the rest.

> Install code-guardrails plugin: run **`/plugin marketplace add Wisteria30/code-guardrails`** then **`/plugin install code-guardrails@code-guardrails-marketplace`**. If ast-grep or ripgrep is missing, install them with `brew install ast-grep ripgrep`. Then restart Claude Code so the plugin loads. After restart, confirm the plugin is active by running `/scan`.

### Option B: Git clone

> Install code-guardrails: run **`git clone https://github.com/Wisteria30/code-guardrails.git ~/.claude/plugins/code-guardrails && ~/.claude/plugins/code-guardrails/setup`** — if ast-grep or ripgrep is missing, install them following the setup output. Then restart Claude Code so the plugin loads.

### Add to your repo so teammates get it (optional)

> Add code-guardrails to this project: run **`cp -Rf ~/.claude/plugins/code-guardrails .claude/plugins/code-guardrails && rm -rf .claude/plugins/code-guardrails/.git && .claude/plugins/code-guardrails/setup`** then commit the `.claude/plugins/code-guardrails` directory so teammates get it on `git clone`.

Real files get committed to your repo (not a submodule), so `git clone` just works. Everything lives inside `.claude/plugins/`. Nothing touches your PATH or runs in the background.

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
