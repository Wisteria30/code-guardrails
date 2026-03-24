# code-guardrails

**Catch silent fallbacks and test doubles that AI coding tools sneak into your production code.**

AI tools generate working code fast. But behind the scenes, they swallow exceptions with `pass`, leave `mock` objects in production, and paper over failures with `?? "default"`. Miss it in review, and it ships.

code-guardrails parses every file save with ast-grep and warns Claude immediately when these patterns appear.

---

## Install

**Prerequisites:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [ast-grep](https://ast-grep.github.io/) 0.14+, [ripgrep](https://github.com/BurntSushi/ripgrep) 14.0+

```bash
brew install ast-grep ripgrep
```

### Option A: Marketplace (recommended)

Run inside Claude Code:

```
/plugin marketplace add Wisteria30/code-guardrails
/plugin install code-guardrails@code-guardrails-marketplace
```

Restart Claude Code. Verify with `/scan`.

### Option B: Git clone

```bash
git clone https://github.com/Wisteria30/code-guardrails.git ~/.claude/plugins/code-guardrails
~/.claude/plugins/code-guardrails/setup
```

Restart Claude Code.

### Share with your team (optional)

```bash
cp -Rf ~/.claude/plugins/code-guardrails .claude/plugins/code-guardrails
rm -rf .claude/plugins/code-guardrails/.git
git add .claude/plugins/code-guardrails && git commit -m "chore: add code-guardrails plugin"
```

---

## What It Catches

### Test doubles in production code

Flags `mock` / `stub` / `fake` identifiers and `unittest.mock` imports in non-test files. Test files are always ignored.

```python
# NG — mock left in production code
mock_client = MockHttpClient()
from unittest.mock import patch
```

```python
# OK — inside test files (test_*.py, **/tests/**, etc.)
mock_client = MockHttpClient()
```

### Unapproved fallbacks

Flags patterns that silently swallow errors or substitute default values.
Proper error handling (logging, re-raise, error wrapping) is **not** flagged.

#### Python

```python
# NG — swallowed exception
try:
    connect()
except ConnectionError:
    pass

# NG — silent defaults
timeout = config.get("timeout", 30)
name = user_name or "unknown"
port = os.getenv("PORT", "8080")
val = getattr(obj, "attr", None)

# NG — suppressed error
with contextlib.suppress(KeyError):
    process(data)
```

```python
# OK — exception handled properly
try:
    connect()
except ConnectionError as e:
    logger.error(f"Connection failed: {e}")
    raise ServiceUnavailable("DB unreachable") from e

# OK — .get() without a default
value = config.get("timeout")

# OK — or in a condition, not an assignment
if user_name or fallback_name:
    greet()
```

#### TypeScript

```typescript
// NG — silent defaults
const port = config.port ?? 3000;
const name = input || "default";
options.timeout ||= 5000;
cache ??= new Map();

// NG — swallowed errors
try { await fetch(url); } catch (e) { return []; }
try { parse(json); } catch {}
fetch(url).catch(() => null);
```

```typescript
// OK — exception handled properly
try {
  await fetch(url);
} catch (e) {
  logger.error(e);
  throw new FetchError("request failed", { cause: e });
}

// OK — catch with re-throw
promise.catch((e) => { throw new AppError(e); });
```

### Approval model

Intentional fallbacks can be approved with an adjacent comment:

```python
# policy-approved: REQ-123 explicit locale default
lang = payload.get("lang", "ja-JP")
```

```typescript
// policy-approved: ADR-7 demo-mode fallback
const label = apiValue ?? "demo";
```

Accepted prefixes: `REQ-`, `ADR-`, `SPEC-` followed by an identifier.

---

## How It Works

| Trigger | Behavior |
|---|---|
| **PostToolUse hook** | Scans the changed file after every Edit / Write. Warns Claude on violations |
| **`/scan` command** | Full project scan on demand |

17 rules (9 Python + 8 TypeScript), validated against 34+ test fixtures.
Test paths (`**/test/**`, `**/tests/**`, `**/*_test.py`, `*.test.ts`, etc.) are excluded from all rules.

---

## Detection Rules

### Python

| Rule | Pattern | Example |
|---|---|---|
| `py-no-swallowing-except-pass` | `except ...: pass` | `except ValueError: pass` |
| `py-no-fallback-bool-or` | `x = a or b` | `name = val or "default"` |
| `py-no-fallback-get-default` | `.get(key, default)` | `d.get("k", 0)` |
| `py-no-fallback-getattr-default` | `getattr(o, n, default)` | `getattr(o, "x", None)` |
| `py-no-fallback-next-default` | `next(iter, default)` | `next(gen, None)` |
| `py-no-fallback-os-getenv-default` | `os.getenv(k, default)` | `os.getenv("PORT", "8080")` |
| `py-no-fallback-contextlib-suppress` | `contextlib.suppress(...)` | `with suppress(KeyError):` |
| `py-no-test-double-identifier` | identifier matching `mock\|stub\|fake` | `mock_client` |
| `py-no-test-double-unittest-mock` | `import unittest.mock` | `from unittest.mock import patch` |

### TypeScript

| Rule | Pattern | Example |
|---|---|---|
| `ts-no-fallback-or` | `a \|\| b` | `val \|\| "default"` |
| `ts-no-fallback-nullish` | `a ?? b` | `port ?? 3000` |
| `ts-no-fallback-or-assign` | `a \|\|= b` | `opt.x \|\|= 5` |
| `ts-no-fallback-nullish-assign` | `a ??= b` | `cache ??= new Map()` |
| `ts-no-catch-return-default` | `catch { return default }` | `catch(e) { return [] }` |
| `ts-no-empty-catch` | `catch {}` | `catch(e) {}` |
| `ts-no-promise-catch-default` | `.catch(() => default)` | `.catch(() => null)` |
| `ts-no-test-double-identifier` | identifier matching `mock\|stub\|fake` | `mockFetch` |

---

## CLI Usage

```bash
# Full project scan
python check_policy.py .

# Single file
python check_policy.py --changed-only path/to/file.py

# JSON output (CI / hooks)
python check_policy.py --changed-only file.py --format json
```

---

## Development

```bash
python test_rules.py        # Rule validation tests
python test_check_policy.py  # CLI tests
```

## License

MIT
