# code-guardrails Claude Code Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Claude Code plugin that statically detects test doubles and unapproved fallbacks in production code using ast-grep, with ripgrep pre-filtering for large codebases.

**Architecture:** Three layers built in order: (1) test fixtures validating all 17 ast-grep rules, (2) enhanced CLI with --changed-only, JSON/human output, ripgrep pre-filter, and file read caching, (3) Claude Code plugin with PostToolUse prompt-based hook and /scan command.

**Tech Stack:** ast-grep (AST pattern detection), ripgrep (file pre-filter), Python 3.10+ (CLI wrapper), Claude Code plugin system (hooks, commands, skills)

**Spec:** `~/.gstack/projects/code-guardrails/wis30-main-design-20260324-134339.md`

---

### Task 0: Repository Restructure

**Files:**
- Move: `ast_grep_policy_pack/rules/` -> `rules/`
- Move: `ast_grep_policy_pack/sgconfig.yml` -> `sgconfig.yml`
- Move: `ast_grep_policy_pack/check_policy.py` -> `check_policy.py`
- Delete: `ast_grep_policy_pack/` (after migration)
- Create: `.gitignore`

- [ ] **Step 1: Move files to plugin-root layout**

```bash
cd /Users/wis30/ghq/github.com/Wisteria30/code-guardrails
mv ast_grep_policy_pack/rules ./rules
mv ast_grep_policy_pack/sgconfig.yml ./sgconfig.yml
mv ast_grep_policy_pack/check_policy.py ./check_policy.py
mv ast_grep_policy_pack/README.md ./README.md
```

- [ ] **Step 2: Clean up old directory**

```bash
rm -rf ast_grep_policy_pack/
```

- [ ] **Step 3: Create .gitignore**

```gitignore
__pycache__/
*.pyc
.DS_Store
fixtures/**/node_modules/
```

- [ ] **Step 4: Verify ast-grep still works with new layout**

```bash
ast-grep scan .
```

Expected: runs without config errors (ast-grep auto-detects sgconfig.yml in cwd). May produce findings on existing files, that's OK.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: restructure repo to Claude Code plugin layout"
```

---

### Task 1: Test Runner Script + ast-grep Verification

**Files:**
- Create: `test_rules.py`

- [ ] **Step 1: Verify ast-grep is installed**

```bash
ast-grep --version
```

If not installed: `brew install ast-grep` or `cargo install ast-grep --locked`

- [ ] **Step 2: Verify ripgrep is installed**

```bash
rg --version
```

If not installed: `brew install ripgrep` or `cargo install ripgrep`

- [ ] **Step 3: Write the test runner**

Create `test_rules.py` — a script that:
1. Walks `fixtures/` directories
2. For each `should_fail/` file: runs ast-grep, asserts at least 1 finding
3. For each `should_pass/` file: runs ast-grep, asserts 0 findings
4. For each `approved/` file: runs `check_policy.py`, asserts 0 unsuppressed findings (policy-approved comments filter them)

```python
#!/usr/bin/env python3
"""Test runner for ast-grep rules against fixtures."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

FIXTURE_ROOT = Path(__file__).parent / "fixtures"
SGCONFIG = Path(__file__).parent / "sgconfig.yml"
CHECK_POLICY = Path(__file__).parent / "check_policy.py"

RED = "\033[91m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
RESET = "\033[0m"


def run_ast_grep_on_file(filepath: Path) -> list[dict]:
    """Run ast-grep on a single file and return findings."""
    proc = subprocess.run(
        ["ast-grep", "scan", "--json=stream", str(filepath)],
        capture_output=True,
        text=True,
        cwd=str(SGCONFIG.parent),  # run from repo root so sgconfig.yml is auto-detected
    )
    findings = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line:
            findings.append(json.loads(line))
    return findings


def run_check_policy_on_file(filepath: Path) -> int:
    """Run check_policy.py on a single file and return exit code."""
    proc = subprocess.run(
        [sys.executable, str(CHECK_POLICY), "--changed-only", str(filepath)],
        capture_output=True,
        text=True,
    )
    return proc.returncode


def test_fixture_dir(fixture_dir: Path, expect: str) -> tuple[int, int]:
    """Test all files in a fixture directory. Returns (passed, failed)."""
    passed = failed = 0
    if not fixture_dir.exists():
        return passed, failed

    for filepath in sorted(fixture_dir.iterdir()):
        if filepath.is_dir() or filepath.name.startswith("."):
            continue

        if expect == "fail":
            findings = run_ast_grep_on_file(filepath)
            if len(findings) > 0:
                print(f"  {GREEN}PASS{RESET} {filepath.name} ({len(findings)} findings)")
                passed += 1
            else:
                print(f"  {RED}FAIL{RESET} {filepath.name} (expected findings, got 0)")
                failed += 1

        elif expect == "pass":
            findings = run_ast_grep_on_file(filepath)
            if len(findings) == 0:
                print(f"  {GREEN}PASS{RESET} {filepath.name} (0 findings)")
                passed += 1
            else:
                print(f"  {RED}FAIL{RESET} {filepath.name} (expected 0 findings, got {len(findings)})")
                for f in findings:
                    rule_id = f.get("ruleId", "?")
                    line = f.get("range", {}).get("start", {}).get("line", "?")
                    print(f"    -> {rule_id} at line {line}")
                failed += 1

        elif expect == "approved":
            exit_code = run_check_policy_on_file(filepath)
            if exit_code == 0:
                print(f"  {GREEN}PASS{RESET} {filepath.name} (approved, 0 unsuppressed)")
                passed += 1
            else:
                print(f"  {RED}FAIL{RESET} {filepath.name} (expected approved, exit code {exit_code})")
                failed += 1

    return passed, failed


def main() -> int:
    total_passed = total_failed = 0

    for lang_dir in sorted(FIXTURE_ROOT.iterdir()):
        if not lang_dir.is_dir():
            continue
        for group_dir in sorted(lang_dir.iterdir()):
            if not group_dir.is_dir():
                continue

            print(f"\n{YELLOW}=== {lang_dir.name}/{group_dir.name} ==={RESET}")

            for expect_dir_name, expect in [("should_fail", "fail"), ("should_pass", "pass"), ("approved", "approved")]:
                expect_dir = group_dir / expect_dir_name
                if expect_dir.exists():
                    print(f"\n  [{expect_dir_name}]")
                    p, f = test_fixture_dir(expect_dir, expect)
                    total_passed += p
                    total_failed += f

    print(f"\n{'=' * 40}")
    print(f"Total: {GREEN}{total_passed} passed{RESET}, {RED}{total_failed} failed{RESET}")
    return 1 if total_failed > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run test runner (should show no fixtures yet)**

```bash
python test_rules.py
```

Expected: `Total: 0 passed, 0 failed` (no fixtures exist yet).

- [ ] **Step 5: Commit**

```bash
git add test_rules.py .gitignore
git commit -m "feat: add test runner for ast-grep rule validation"
```

---

### Task 2: Python Test-Double Fixtures + Rule Validation

**Files:**
- Create: `fixtures/python/test-double/should_fail/mock_service.py`
- Create: `fixtures/python/test-double/should_fail/fake_client.py`
- Create: `fixtures/python/test-double/should_fail/stub_handler.py`
- Create: `fixtures/python/test-double/should_fail/unittest_mock_import.py`
- Create: `fixtures/python/test-double/should_fail/unittest_mock_from_import.py`
- Create: `fixtures/python/test-double/should_pass/real_service.py`
- Create: `fixtures/python/test-double/should_pass/legitimate_names.py`
- Verify: `rules/py-no-test-double-identifier.yml`
- Verify: `rules/py-no-test-double-unittest-mock.yml`

- [ ] **Step 1: Create should_fail fixtures for test-double identifiers**

`fixtures/python/test-double/should_fail/mock_service.py`:
```python
class MockPaymentGateway:
    def charge(self, amount):
        return {"status": "ok"}

mock_client = MockPaymentGateway()
result = mock_client.charge(100)
```

`fixtures/python/test-double/should_fail/fake_client.py`:
```python
def create_fake_database():
    return {"users": []}

fake_db = create_fake_database()
```

`fixtures/python/test-double/should_fail/stub_handler.py`:
```python
def stub_response():
    return {"data": "stubbed"}

stub_result = stub_response()
```

- [ ] **Step 2: Create should_fail fixtures for unittest.mock imports**

`fixtures/python/test-double/should_fail/unittest_mock_import.py`:
```python
import unittest.mock

patcher = unittest.mock.patch("os.path.exists")
```

`fixtures/python/test-double/should_fail/unittest_mock_from_import.py`:
```python
from unittest.mock import MagicMock, patch

service = MagicMock()
```

- [ ] **Step 3: Create should_pass fixtures**

`fixtures/python/test-double/should_pass/real_service.py`:
```python
class PaymentGateway:
    def __init__(self, api_key: str):
        self.api_key = api_key

    def charge(self, amount: int) -> dict:
        return self._call_api("/charge", {"amount": amount})

    def _call_api(self, path: str, data: dict) -> dict:
        raise NotImplementedError
```

`fixtures/python/test-double/should_pass/legitimate_names.py`:
```python
# Words containing mock/stub/fake as substrings should NOT trigger
# unless they are standalone identifiers matching the regex

def process_hammock_data(data):
    """Hammock contains 'mock' but is not a test double."""
    return data

class Stockbroker:
    """Stockbroker contains 'stub' but is not a test double."""
    pass
```

- [ ] **Step 4: Run test runner**

```bash
python test_rules.py
```

Expected: All should_fail fixtures produce findings, all should_pass produce 0 findings. If `legitimate_names.py` fails (false positive on "hammock"/"stockbroker"), the regex `(?i)(mock|stub|fake)` matches substrings. Check if ast-grep's `regex` on `kind: identifier` only matches the full identifier or does substring matching. If substring, consider adjusting the regex to `(?i)\b(mock|stub|fake)` or using word boundaries.

- [ ] **Step 5: Fix rules if needed based on test results**

If substring matches are an issue, update `rules/py-no-test-double-identifier.yml`:
```yaml
rule:
  kind: identifier
  regex: '(?i)^.*(mock|stub|fake).*$'
```
Note: ast-grep's `kind: identifier` already limits to identifier nodes. The regex `(?i)(mock|stub|fake)` does substring match on the identifier text. This is likely intentional — `mock_service` should be caught. But `hammock` should also be caught by this regex. Decide: is catching `hammock` acceptable (safe-side) or should the regex require word boundaries?

- [ ] **Step 6: Commit**

```bash
git add fixtures/python/test-double/ rules/py-no-test-double-*.yml
git commit -m "test: add Python test-double fixtures and validate rules"
```

---

### Task 3: Python Fallback Fixtures + Rule Fixes

**Files:**
- Create: `fixtures/python/fallback/should_fail/*.py` (7+ files)
- Create: `fixtures/python/fallback/should_pass/*.py` (3+ files)
- Create: `fixtures/python/fallback/approved/*.py` (2+ files)
- Modify: `rules/py-no-fallback-bool-or.yml` (restrict to assignment context)
- Modify: All Python fallback rules (add `ignores` for test paths)

- [ ] **Step 1: Add `ignores` to ALL Python fallback rules**

Add to each of these rules (`py-no-fallback-bool-or.yml`, `py-no-fallback-get-default.yml`, `py-no-fallback-getattr-default.yml`, `py-no-fallback-next-default.yml`, `py-no-fallback-os-getenv-default.yml`, `py-no-fallback-contextlib-suppress.yml`, `py-no-swallowing-except-pass.yml`):

```yaml
ignores:
  - '**/test/**'
  - '**/tests/**'
  - '**/*_test.py'
  - '**/test_*.py'
  - '**/conftest.py'
```

- [ ] **Step 2: Restrict `py-no-fallback-bool-or` to assignment context**

Update `rules/py-no-fallback-bool-or.yml`:

```yaml
id: py-no-fallback-bool-or
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
rule:
  pattern: $VAR = $A or $B
message: 'Potential fallback detected via Python `or` in assignment'
note: 'Fallback behavior is forbidden unless explicitly approved in requirements. If this one is intentional, add an adjacent `policy-approved: REQ-...` comment.'
metadata:
  policy_group: fallback
  approval_mode: adjacent_policy_comment
```

If `$VAR = $A or $B` doesn't work as expected (ast-grep may struggle with assignment + boolean_operator), try the `inside` constraint:

```yaml
rule:
  pattern: $A or $B
  inside:
    kind: assignment
    stopBy: end
```

Or the `not_inside` fallback:

```yaml
rule:
  pattern: $A or $B
  not:
    inside:
      any:
        - kind: if_statement
        - kind: while_statement
        - kind: assert_statement
        - kind: argument_list
      stopBy: end
```

Test all three approaches with fixtures.

- [ ] **Step 3: Create should_fail fixtures for Python fallback**

`fixtures/python/fallback/should_fail/or_default.py`:
```python
name = user_input or "anonymous"
lang = payload.get("lang") or "en"
```

`fixtures/python/fallback/should_fail/dict_get_default.py`:
```python
config = {}
timeout = config.get("timeout", 30)
host = config.get("host", "localhost")
```

`fixtures/python/fallback/should_fail/getattr_default.py`:
```python
class Config:
    pass

cfg = Config()
debug = getattr(cfg, "debug", False)
```

`fixtures/python/fallback/should_fail/next_default.py`:
```python
items = iter([])
first = next(items, None)
```

`fixtures/python/fallback/should_fail/os_env_default.py`:
```python
import os

db_url = os.getenv("DATABASE_URL", "sqlite:///dev.db")
port = os.environ.get("PORT", "8080")
```

`fixtures/python/fallback/should_fail/contextlib_suppress.py`:
```python
import contextlib

with contextlib.suppress(FileNotFoundError):
    data = open("config.json").read()
```

`fixtures/python/fallback/should_fail/except_pass.py`:
```python
try:
    result = risky_operation()
except Exception:
    pass

try:
    connect()
except ConnectionError as e:
    pass
```

- [ ] **Step 4: Create should_pass fixtures**

`fixtures/python/fallback/should_pass/no_fallback.py`:
```python
# None of these should trigger fallback rules
name = get_user_name()
config = load_config()
items = list(range(10))

if name and config:
    process(name, config)
```

`fixtures/python/fallback/should_pass/conditional_or.py`:
```python
# `or` in conditions should NOT trigger (after rule fix)
if x or y:
    do_something()

while running or pending:
    process()

assert valid or override, "must be valid"
```

`fixtures/python/fallback/should_pass/proper_error_handling.py`:
```python
try:
    result = risky_operation()
except ValueError as e:
    logger.error("Operation failed: %s", e)
    raise
```

- [ ] **Step 5: Create approved fixtures**

`fixtures/python/fallback/approved/approved_get_default.py`:
```python
config = {}
# policy-approved: REQ-42 explicit timeout default for health checks
timeout = config.get("timeout", 30)
```

`fixtures/python/fallback/approved/approved_or_default.py`:
```python
# policy-approved: REQ-15 explicit locale fallback per i18n spec
lang = payload.get("lang") or "ja-JP"
```

- [ ] **Step 6: Run test runner and iterate**

```bash
python test_rules.py
```

Fix rules until all fixtures pass. This is the most critical iteration step — `py-no-fallback-bool-or` is the rule most likely to need multiple attempts.

- [ ] **Step 7: Commit**

```bash
git add fixtures/python/fallback/ rules/py-no-fallback-*.yml rules/py-no-swallowing-except-pass.yml
git commit -m "test: add Python fallback fixtures, fix rules (ignores + assignment context)"
```

---

### Task 4: TypeScript Test-Double Fixtures + Rule Validation

**Files:**
- Create: `fixtures/typescript/test-double/should_fail/*.ts` (3 files)
- Create: `fixtures/typescript/test-double/should_pass/*.ts` (2 files)
- Verify: `rules/ts-no-test-double-identifier.yml`

- [ ] **Step 1: Create should_fail fixtures**

`fixtures/typescript/test-double/should_fail/mockApi.ts`:
```typescript
const mockApiClient = {
  get: async (url: string) => ({ data: "mocked" }),
};

const result = await mockApiClient.get("/users");
```

`fixtures/typescript/test-double/should_fail/fakeStore.ts`:
```typescript
function createFakeStore() {
  return { items: [] };
}

const fakeStore = createFakeStore();
```

`fixtures/typescript/test-double/should_fail/stubHandler.ts`:
```typescript
const stubHandler = (req: Request) => {
  return new Response("stubbed");
};
```

- [ ] **Step 2: Create should_pass fixtures**

`fixtures/typescript/test-double/should_pass/realApi.ts`:
```typescript
class ApiClient {
  constructor(private baseUrl: string) {}

  async get(path: string): Promise<Response> {
    return fetch(`${this.baseUrl}${path}`);
  }
}
```

`fixtures/typescript/test-double/should_pass/legitimateNames.ts`:
```typescript
// Words containing mock/stub/fake as substrings
function processHammockData(data: unknown) {
  return data;
}
```

- [ ] **Step 3: Run test runner**

```bash
python test_rules.py
```

Expected: All should_fail produce findings, all should_pass produce 0.

- [ ] **Step 4: Commit**

```bash
git add fixtures/typescript/test-double/
git commit -m "test: add TypeScript test-double fixtures and validate rules"
```

---

### Task 5: TypeScript Fallback Fixtures + Rule Fixes

**Files:**
- Create: `fixtures/typescript/fallback/should_fail/*.ts` (7 files)
- Create: `fixtures/typescript/fallback/should_pass/*.ts` (2 files)
- Create: `fixtures/typescript/fallback/approved/*.ts` (1 file)
- Modify: All TypeScript fallback rules (add `ignores` for test paths)

- [ ] **Step 1: Add `ignores` to ALL TypeScript fallback rules**

Add to each rule (`ts-no-fallback-nullish.yml`, `ts-no-fallback-nullish-assign.yml`, `ts-no-fallback-or.yml`, `ts-no-fallback-or-assign.yml`, `ts-no-empty-catch.yml`, `ts-no-catch-return-default.yml`, `ts-no-promise-catch-default.yml`):

```yaml
ignores:
  - '**/test/**'
  - '**/tests/**'
  - '**/*.test.ts'
  - '**/*.spec.ts'
  - '**/__tests__/**'
```

- [ ] **Step 2: Create should_fail fixtures**

`fixtures/typescript/fallback/should_fail/nullishCoalescing.ts`:
```typescript
const name = userInput ?? "anonymous";
```

`fixtures/typescript/fallback/should_fail/nullishAssign.ts`:
```typescript
let config: string | undefined;
config ??= "default";
```

`fixtures/typescript/fallback/should_fail/orDefault.ts`:
```typescript
const label = apiValue || "fallback";
```

`fixtures/typescript/fallback/should_fail/orAssign.ts`:
```typescript
let value: string | undefined;
value ||= "default";
```

`fixtures/typescript/fallback/should_fail/emptyCatch.ts`:
```typescript
try {
  riskyOperation();
} catch (e) {
}
```

`fixtures/typescript/fallback/should_fail/catchReturnDefault.ts`:
```typescript
function getData() {
  try {
    return fetchData();
  } catch (e) {
    return [];
  }
}
```

`fixtures/typescript/fallback/should_fail/promiseCatchDefault.ts`:
```typescript
const data = fetchApi().catch((err) => []);
const data2 = fetchApi().catch(() => null);
```

- [ ] **Step 3: Create should_pass fixtures**

`fixtures/typescript/fallback/should_pass/noFallback.ts`:
```typescript
const name = getUserName();
const config = loadConfig();

if (name && config) {
  process(name, config);
}
```

`fixtures/typescript/fallback/should_pass/properErrorHandling.ts`:
```typescript
try {
  const result = riskyOperation();
} catch (e) {
  console.error("Operation failed:", e);
  throw e;
}
```

- [ ] **Step 4: Create approved fixtures**

`fixtures/typescript/fallback/approved/approvedNullish.ts`:
```typescript
// policy-approved: ADR-7 explicit demo-mode fallback
const label = apiValue ?? "demo";
```

- [ ] **Step 5: Run test runner and iterate**

```bash
python test_rules.py
```

Fix rules until all fixtures pass.

- [ ] **Step 6: Commit**

```bash
git add fixtures/typescript/fallback/ rules/ts-no-fallback-*.yml rules/ts-no-empty-catch.yml rules/ts-no-catch-return-default.yml rules/ts-no-promise-catch-default.yml
git commit -m "test: add TypeScript fallback fixtures, fix rules (ignores for test paths)"
```

---

### Task 6: Enhance check_policy.py

**Files:**
- Modify: `check_policy.py`

- [ ] **Step 1: Write tests for the enhanced CLI**

Create `test_check_policy.py`:

```python
#!/usr/bin/env python3
"""Tests for check_policy.py enhancements."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

CHECK_POLICY = Path(__file__).parent / "check_policy.py"
FIXTURES = Path(__file__).parent / "fixtures"

RED = "\033[91m"
GREEN = "\033[92m"
RESET = "\033[0m"


def run_check(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECK_POLICY)] + args,
        capture_output=True,
        text=True,
    )


def test_changed_only_single_file():
    """--changed-only on a file with violations returns exit code 1."""
    target = FIXTURES / "python" / "fallback" / "should_fail" / "dict_get_default.py"
    if not target.exists():
        return "SKIP (fixture missing)"
    result = run_check(["--changed-only", str(target)])
    assert result.returncode == 1, f"expected exit 1, got {result.returncode}"
    return "PASS"


def test_changed_only_clean_file():
    """--changed-only on a clean file returns exit code 0."""
    target = FIXTURES / "python" / "fallback" / "should_pass" / "no_fallback.py"
    if not target.exists():
        return "SKIP (fixture missing)"
    result = run_check(["--changed-only", str(target)])
    assert result.returncode == 0, f"expected exit 0, got {result.returncode}"
    return "PASS"


def test_format_json():
    """--format json outputs valid JSON lines."""
    target = FIXTURES / "python" / "fallback" / "should_fail" / "dict_get_default.py"
    if not target.exists():
        return "SKIP (fixture missing)"
    result = run_check(["--changed-only", str(target), "--format", "json"])
    lines = [l for l in result.stdout.strip().splitlines() if l.strip()]
    for line in lines:
        parsed = json.loads(line)
        assert "file" in parsed
        assert "rule_id" in parsed
    return "PASS"


def test_approved_file():
    """Approved file has 0 unsuppressed findings."""
    target = FIXTURES / "python" / "fallback" / "approved" / "approved_get_default.py"
    if not target.exists():
        return "SKIP (fixture missing)"
    result = run_check(["--changed-only", str(target)])
    assert result.returncode == 0, f"expected exit 0, got {result.returncode}"
    return "PASS"


def test_test_globs_skip():
    """--test-globs skips test files."""
    # Create a temp test file path that matches test globs
    result = run_check(["--changed-only", "tests/test_example.py", "--test-globs", "**/test_*.py"])
    assert result.returncode == 0, f"expected exit 0 (skipped), got {result.returncode}"
    return "PASS"


def main():
    tests = [
        test_changed_only_single_file,
        test_changed_only_clean_file,
        test_format_json,
        test_approved_file,
        test_test_globs_skip,
    ]
    passed = failed = 0
    for test_fn in tests:
        try:
            result = test_fn()
            if result == "PASS":
                print(f"  {GREEN}PASS{RESET} {test_fn.__name__}")
                passed += 1
            else:
                print(f"  {result} {test_fn.__name__}")
        except AssertionError as e:
            print(f"  {RED}FAIL{RESET} {test_fn.__name__}: {e}")
            failed += 1
        except Exception as e:
            print(f"  {RED}ERROR{RESET} {test_fn.__name__}: {e}")
            failed += 1

    print(f"\ncheck_policy tests: {GREEN}{passed} passed{RESET}, {RED}{failed} failed{RESET}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Run tests to verify they fail (TDD)**

```bash
python test_check_policy.py
```

Expected: Most tests FAIL because --changed-only, --format json, --test-globs don't exist yet.

- [ ] **Step 3: Implement --changed-only**

Add to `check_policy.py`:
- New arg: `parser.add_argument('--changed-only', help='Scan a single file instead of full project')`
- New arg: `parser.add_argument('--config-dir', help='Directory containing sgconfig.yml (default: same as check_policy.py)')`
- When `--changed-only` is provided: run `ast-grep scan --json=stream <file>` with `cwd` set to config-dir (so sgconfig.yml is auto-detected)
- Keep existing behavior when `--changed-only` is not provided
- Error handling: if ast-grep is not found (`FileNotFoundError`), exit 2. If ast-grep returns unexpected exit code (not 0 or 1), exit 2.

- [ ] **Step 4: Implement --format json/human**

Add to `check_policy.py`:
- New arg: `parser.add_argument('--format', choices=['human', 'json'], default='human')`
- `--format json`: output each unsuppressed finding as a JSON line: `{"file": "...", "line": N, "column": N, "rule_id": "...", "severity": "...", "message": "...", "code": "..."}`
- `--format human`: current colored output (existing behavior, enhanced with colors)

- [ ] **Step 5: Implement --test-globs**

Add to `check_policy.py`:
- New arg: `parser.add_argument('--test-globs', help='Comma-separated globs; skip files matching these')`
- Default globs: `**/test/**,**/tests/**,**/*_test.py,**/test_*.py,**/conftest.py,**/*.test.ts,**/*.spec.ts,**/__tests__/**`
- When `--changed-only` is used: check if the file matches any glob. If so, exit 0 immediately (skip scan).

- [ ] **Step 6: Add file read caching**

Replace `has_adjacent_policy_approval` to use a cache:

```python
_FILE_CACHE: dict[Path, list[str]] = {}

def read_lines_cached(path: Path) -> list[str]:
    if path not in _FILE_CACHE:
        _FILE_CACHE[path] = read_lines(path)
    return _FILE_CACHE[path]
```

Update `has_adjacent_policy_approval` (line 43-44 of current check_policy.py):
- Change `lines = read_lines(file_path)` to `lines = read_lines_cached(file_path)`

- [ ] **Step 7: Add ripgrep pre-filter for full project scan**

When scanning the full project (no `--changed-only`), use ripgrep to pre-filter:

```python
def get_candidate_files_rg(root: Path) -> list[str] | None:
    """Use ripgrep to find files containing relevant patterns. Returns None if rg not available."""
    patterns = ["mock|stub|fake", "= .* or ", "\\?\\?", "\\|\\|", "except.*pass", "catch", "suppress", "getattr", "getenv", "\\.get\\("]
    combined = "|".join(patterns)
    try:
        proc = subprocess.run(
            ["rg", "--files-with-matches", "-e", combined, "--type", "py", "--type", "ts", str(root)],
            capture_output=True, text=True,
        )
        if proc.returncode in (0, 1):
            return [f for f in proc.stdout.strip().splitlines() if f]
    except FileNotFoundError:
        pass
    return None
```

When candidate files are returned, pass them to ast-grep instead of scanning the whole project.

- [ ] **Step 8: Run tests**

```bash
python test_check_policy.py
```

Expected: All PASS.

- [ ] **Step 9: Commit**

```bash
git add check_policy.py test_check_policy.py
git commit -m "feat: enhance check_policy.py with --changed-only, --format, --test-globs, ripgrep pre-filter"
```

---

### Task 7: Claude Code Plugin Structure

**Files:**
- Create: `plugin.json`
- Create: `setup`
- Create: `hooks/post-edit-scan.sh`
- Create: `commands/scan.md`
- Create: `skills/guardrails.md`

- [ ] **Step 1: Create plugin.json**

```json
{
  "name": "code-guardrails",
  "version": "0.1.0",
  "description": "Detect and block test doubles and unapproved fallbacks in production code. AI coding tools silently introduce mock/stub/fake objects and fallback behaviors — this plugin catches them.",
  "author": "Wisteria30"
}
```

- [ ] **Step 2: Create setup script**

`setup` (executable):

```bash
#!/usr/bin/env bash
set -euo pipefail

RED='\033[91m'
GREEN='\033[92m'
YELLOW='\033[93m'
RESET='\033[0m'

check_tool() {
  local tool="$1"
  local install_hint="$2"
  if command -v "$tool" &>/dev/null; then
    echo -e "${GREEN}OK${RESET} $tool ($(command -v "$tool"))"
    return 0
  else
    echo -e "${RED}MISSING${RESET} $tool"
    echo "  Install: $install_hint"
    return 1
  fi
}

echo "code-guardrails setup check"
echo "=========================="
MISSING=0

check_tool "ast-grep" "brew install ast-grep  OR  cargo install ast-grep --locked" || MISSING=1
check_tool "rg" "brew install ripgrep  OR  cargo install ripgrep" || MISSING=1
check_tool "python3" "Install Python 3.10+" || MISSING=1

if [ "$MISSING" -eq 1 ]; then
  echo ""
  echo -e "${RED}Some dependencies are missing. Install them and re-run setup.${RESET}"
  exit 1
fi

echo ""
echo -e "${GREEN}All dependencies installed. code-guardrails is ready.${RESET}"
```

```bash
chmod +x setup
```

- [ ] **Step 3: Create PostToolUse hook**

`hooks/post-edit-scan.sh`:

This is a **prompt-based hook** (handler type: `prompt`). Its stdout is fed back to Claude as context.

```bash
#!/usr/bin/env bash
# PostToolUse hook for Edit/Write — scans the changed file for policy violations.
# Handler type: prompt (stdout is fed back to Claude as additional context)
set -euo pipefail

PLUGIN_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CHECK_POLICY="$PLUGIN_DIR/check_policy.py"

# Extract file path from tool_input (passed via stdin as JSON)
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || echo "")

if [ -z "$FILE_PATH" ]; then
  exit 0
fi

# Run policy check (capture exit code properly)
set +e
OUTPUT=$(cd "$PLUGIN_DIR" && python3 "$CHECK_POLICY" --changed-only "$FILE_PATH" --format json 2>/dev/null)
EXIT_CODE=$?
set -e

# Exit code 2 = tool error, fail-open
if [ "$EXIT_CODE" -eq 2 ]; then
  exit 0
fi

# Exit code 0 = clean
if [ "$EXIT_CODE" -eq 0 ] || [ -z "$OUTPUT" ]; then
  exit 0
fi

# Violations found — output structured feedback for Claude
echo ""
echo "=== CODE GUARDRAILS: Policy Violations Found ==="
echo ""
echo "$OUTPUT" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        f = json.loads(line)
        print(f\"  {f['file']}:{f['line']}:{f['column']} [{f['rule_id']}] {f['message']}\")
        if f.get('code'):
            print(f\"    code: {f['code'][:200]}\")
    except (json.JSONDecodeError, KeyError):
        print(f'  {line}')
"
echo ""
echo "Fix these violations before proceeding. For intentional fallbacks, add:"
echo "  # policy-approved: REQ-xxx <reason>"
echo "=== END CODE GUARDRAILS ==="
```

```bash
chmod +x hooks/post-edit-scan.sh
```

- [ ] **Step 4: Create /scan command**

`commands/scan.md`:

```markdown
---
name: scan
description: Scan the project for policy violations (test doubles and unapproved fallbacks)
---

Run the code-guardrails policy scanner on the current project.

Execute:
\`\`\`bash
cd "${CLAUDE_PLUGIN_ROOT}" && python3 check_policy.py "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" --format human
\`\`\`

Note: `${CLAUDE_PLUGIN_ROOT}` is provided by Claude Code plugin system as the plugin's root directory. Verify this variable name exists during Task 8 E2E testing; adjust if needed.

Show the output to the user. If violations are found, offer to fix them.
```

- [ ] **Step 5: Create guardrails skill**

`skills/guardrails.md`:

```markdown
---
name: guardrails
description: Understand the code-guardrails policy model for writing compliant code
---

# Code Guardrails Policy

This project enforces two policies on production code:

## 1. No Test Doubles in Production
- mock, stub, fake identifiers are banned outside test files
- `unittest.mock` imports are banned outside test files
- No exceptions. Use MagicMock in your unit tests only.

## 2. No Unapproved Fallbacks
- Default values in `.get()`, `getattr()`, `next()`, `os.getenv()` are flagged
- `or` fallbacks in assignments (`x = a or b`) are flagged
- `??`, `||`, `??=`, `||=` in TypeScript are flagged
- `except: pass`, `contextlib.suppress`, empty catch blocks are flagged
- Promise `.catch(() => default)` is flagged

### Approval Model
To approve an intentional fallback, add a comment on the same line or within 2 lines above:

```python
# policy-approved: REQ-123 explicit locale default
lang = payload.get("lang", "ja-JP")
```

The comment must contain `policy-approved:` followed by a `REQ-`, `ADR-`, or `SPEC-` prefix and an identifier.

## When Writing Code
- Never introduce mock/stub/fake objects in production code
- Never add fallback defaults without explicit requirement approval
- If a fallback is needed, discuss with the user and get a REQ/ADR/SPEC reference first
```

- [ ] **Step 6: Register hook in plugin.json**

Update `plugin.json` to register the hook:

```json
{
  "name": "code-guardrails",
  "version": "0.1.0",
  "description": "Detect and block test doubles and unapproved fallbacks in production code.",
  "author": "Wisteria30",
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "handler": {
          "type": "prompt",
          "prompt": "hooks/post-edit-scan.sh"
        }
      }
    ]
  }
}
```

**IMPORTANT: The plugin.json hook schema above is a best guess.** Before implementing:
1. Read the Claude Code plugin hook documentation: https://code.claude.com/docs/en/hooks
2. Check if hooks are registered in `plugin.json` or in the user's `settings.json`
3. Use the `plugin-dev:hook-development` skill or `plugin-dev:plugin-validator` agent to verify
4. If hooks cannot be auto-registered via plugin.json, document the manual `settings.json` config the user needs to add

- [ ] **Step 7: Commit**

```bash
git add plugin.json setup hooks/ commands/ skills/
git commit -m "feat: add Claude Code plugin structure (hooks, commands, skills)"
```

---

### Task 8: End-to-End Testing

**Files:** None (testing only)

- [ ] **Step 1: Run full test suite**

```bash
python test_rules.py && python test_check_policy.py
```

Expected: All tests pass.

- [ ] **Step 2: Run setup script**

```bash
./setup
```

Expected: All dependencies OK.

- [ ] **Step 3: Test /scan on the repo itself**

```bash
python3 check_policy.py . --format human
```

Expected: May find violations in fixture files (should_fail), but not in should_pass or check_policy.py itself.

- [ ] **Step 4: Test hook in Claude Code session**

Start Claude Code with the plugin:
```bash
claude --plugin-dir /Users/wis30/ghq/github.com/Wisteria30/code-guardrails
```

Ask Claude to write code with a mock:
> "Create a file `test_integration.py` with a MockDatabase class"

The PostToolUse hook should fire and display violations.

- [ ] **Step 5: Test approved comment flow**

Ask Claude to add a fallback with approval:
> "Add `timeout = config.get('timeout', 30)` with a policy-approved comment referencing REQ-99"

Verify the hook does NOT flag the approved line.

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "chore: finalize code-guardrails v0.1.0"
```

---

### Task Summary

| Task | Description | Depends On |
|------|-------------|------------|
| 0 | Repository restructure | - |
| 1 | Test runner + tool verification | 0 |
| 2 | Python test-double fixtures | 1 |
| 3 | Python fallback fixtures + rule fixes | 1 |
| 4 | TypeScript test-double fixtures | 1 |
| 5 | TypeScript fallback fixtures + rule fixes | 1 |
| 6 | Enhanced check_policy.py | 2, 3, 4, 5 |
| 7 | Claude Code plugin structure | 6 |
| 8 | End-to-end testing | 7 |

Tasks 2-5 can run in parallel. Task 6 depends on fixtures existing. Tasks 7-8 are sequential.

**Fixture count target:** The plan lists ~34 explicit fixture files. To reach the spec target of 40-60, add edge cases during implementation:
- Empty files (0 lines)
- Files with only comments
- Files with multiple violations on same line
- Files with policy-approved comment at various distances (same line, 1 above, 2 above, 3 above = should NOT suppress)
- Identifiers where mock/stub/fake is a substring (e.g., `hammock`, `stockbroker`)
- Mixed violations (test-double + fallback in same file)

Aim for ~45 total fixtures across all tasks.
