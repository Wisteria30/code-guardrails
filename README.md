# code-guardrails

**AI が書いたコードに紛れ込む「握りつぶし」と「ハリボテ」を自動検出する Claude Code プラグイン**

AI コーディングツールは、動くコードを素早く生成します。しかしその裏で、エラーを `pass` で握りつぶしたり、`mock` オブジェクトを本番コードに残したり、`?? "default"` で問題を先送りにすることがあります。レビューで見落とせば、それがそのまま本番に入ります。

code-guardrails は、ファイル保存のたびに ast-grep でコードを構文解析し、こうしたパターンを即座に検出して Claude に警告します。

---

## What It Catches

### Test doubles in production code

本番コードに mock / stub / fake が残っていたら即エラー。テストファイルでは無視します。

```python
# NG — 本番コードに mock が残っている
mock_client = MockHttpClient()
from unittest.mock import patch
```

```python
# OK — テストファイル内なら問題なし (test_*.py, **/tests/** 等)
mock_client = MockHttpClient()
```

### Unapproved fallbacks

エラーを握りつぶす・暗黙のデフォルト値で誤魔化すパターンを検出します。
適切なエラーハンドリング（ログ出力、re-raise、エラー変換）は検出しません。

#### Python

```python
# NG — 例外の握りつぶし
try:
    connect()
except ConnectionError:
    pass

# NG — 暗黙のデフォルト値
timeout = config.get("timeout", 30)
name = user_name or "unknown"
port = os.getenv("PORT", "8080")
val = getattr(obj, "attr", None)

# NG — エラーの黙殺
with contextlib.suppress(KeyError):
    process(data)
```

```python
# OK — 例外を適切に処理している
try:
    connect()
except ConnectionError as e:
    logger.error(f"Connection failed: {e}")
    raise ServiceUnavailable("DB unreachable") from e

# OK — デフォルト引数なしの .get()
value = config.get("timeout")

# OK — 条件分岐での or（代入ではない）
if user_name or fallback_name:
    greet()
```

#### TypeScript

```typescript
// NG — デフォルト値でごまかす
const port = config.port ?? 3000;
const name = input || "default";
options.timeout ||= 5000;
cache ??= new Map();

// NG — catch で握りつぶし
try { await fetch(url); } catch (e) { return []; }
try { parse(json); } catch {}
fetch(url).catch(() => null);
```

```typescript
// OK — 例外を適切に処理している
try {
  await fetch(url);
} catch (e) {
  logger.error(e);
  throw new FetchError("request failed", { cause: e });
}

// OK — catch で再 throw
promise.catch((e) => { throw new AppError(e); });
```

### Approval model

意図的なフォールバックには、隣接コメントで承認を明示できます。

```python
# policy-approved: REQ-123 explicit locale default
lang = payload.get("lang", "ja-JP")
```

```typescript
// policy-approved: ADR-7 demo-mode fallback
const label = apiValue ?? "demo";
```

プレフィックスは `REQ-`, `ADR-`, `SPEC-` + 識別子。

---

## Install

**前提:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [ast-grep](https://ast-grep.github.io/) 0.14+, [ripgrep](https://github.com/BurntSushi/ripgrep) 14.0+

```bash
brew install ast-grep ripgrep
```

### Option A: Marketplace (recommended)

Claude Code 内で実行:

```
/plugin marketplace add Wisteria30/code-guardrails
/plugin install code-guardrails@code-guardrails-marketplace
```

再起動して `/scan` で確認。

### Option B: Git clone

```bash
git clone https://github.com/Wisteria30/code-guardrails.git ~/.claude/plugins/code-guardrails
~/.claude/plugins/code-guardrails/setup
```

再起動。

### チームで共有する (optional)

```bash
cp -Rf ~/.claude/plugins/code-guardrails .claude/plugins/code-guardrails
rm -rf .claude/plugins/code-guardrails/.git
git add .claude/plugins/code-guardrails && git commit -m "chore: add code-guardrails plugin"
```

---

## How It Works

| トリガー | 動作 |
|---|---|
| **PostToolUse hook** | Edit / Write のたびに変更ファイルを自動スキャン。違反があれば Claude に即警告 |
| **`/scan` command** | プロジェクト全体をオンデマンドでスキャン |

17 rules (Python 9 + TypeScript 8)。すべて 34+ のテストフィクスチャで検証済み。
テストパス (`**/test/**`, `**/tests/**`, `**/*_test.py`, `*.test.ts` 等) は全ルールで除外。

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

---

## Requirements

Python 3.12+, [ast-grep](https://ast-grep.github.io/) 0.14+, [ripgrep](https://github.com/BurntSushi/ripgrep) 14.0+

## License

MIT
