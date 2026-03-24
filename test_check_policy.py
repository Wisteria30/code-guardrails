#!/usr/bin/env python3
"""Tests for check_policy.py enhancements."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
CHECK_POLICY = SCRIPT_DIR / "check_policy.py"
FIXTURE_ROOT = SCRIPT_DIR / "fixtures"

RED = "\033[91m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
RESET = "\033[0m"


def run_check_policy(*extra_args: str) -> subprocess.CompletedProcess[str]:
    """Run check_policy.py with given arguments."""
    cmd = [sys.executable, str(CHECK_POLICY)] + list(extra_args)
    return subprocess.run(cmd, capture_output=True, text=True)


def test_changed_only_should_fail() -> bool:
    """--changed-only on a should_fail fixture returns exit 1."""
    fixture = FIXTURE_ROOT / "python" / "fallback" / "should_fail" / "or_default.py"
    if not fixture.exists():
        print(f"  {YELLOW}SKIP{RESET} fixture not found: {fixture}")
        return True
    proc = run_check_policy("--changed-only", str(fixture))
    if proc.returncode == 1:
        print(f"  {GREEN}PASS{RESET} --changed-only should_fail => exit 1")
        return True
    else:
        print(f"  {RED}FAIL{RESET} --changed-only should_fail => exit {proc.returncode} (expected 1)")
        if proc.stdout.strip():
            print(f"    stdout: {proc.stdout.strip()[:200]}")
        if proc.stderr.strip():
            print(f"    stderr: {proc.stderr.strip()[:200]}")
        return False


def test_changed_only_should_pass() -> bool:
    """--changed-only on a should_pass fixture returns exit 0."""
    fixture = FIXTURE_ROOT / "python" / "fallback" / "should_pass" / "no_fallback.py"
    if not fixture.exists():
        print(f"  {YELLOW}SKIP{RESET} fixture not found: {fixture}")
        return True
    proc = run_check_policy("--changed-only", str(fixture))
    if proc.returncode == 0:
        print(f"  {GREEN}PASS{RESET} --changed-only should_pass => exit 0")
        return True
    else:
        print(f"  {RED}FAIL{RESET} --changed-only should_pass => exit {proc.returncode} (expected 0)")
        if proc.stdout.strip():
            print(f"    stdout: {proc.stdout.strip()[:200]}")
        if proc.stderr.strip():
            print(f"    stderr: {proc.stderr.strip()[:200]}")
        return False


def test_format_json() -> bool:
    """--format json outputs valid JSON lines."""
    fixture = FIXTURE_ROOT / "python" / "fallback" / "should_fail" / "or_default.py"
    if not fixture.exists():
        print(f"  {YELLOW}SKIP{RESET} fixture not found: {fixture}")
        return True
    proc = run_check_policy("--changed-only", str(fixture), "--format", "json")
    if proc.returncode != 1:
        print(f"  {RED}FAIL{RESET} --format json expected exit 1, got {proc.returncode}")
        return False

    lines = [l for l in proc.stdout.strip().splitlines() if l.strip()]
    if not lines:
        print(f"  {RED}FAIL{RESET} --format json produced no output")
        return False

    for i, line in enumerate(lines):
        try:
            obj = json.loads(line)
            # Verify required fields
            for key in ("file", "line", "column", "rule_id", "severity", "message", "code"):
                if key not in obj:
                    print(f"  {RED}FAIL{RESET} JSON line {i} missing key: {key}")
                    return False
        except json.JSONDecodeError as e:
            print(f"  {RED}FAIL{RESET} JSON line {i} is invalid: {e}")
            print(f"    line: {line[:200]}")
            return False

    print(f"  {GREEN}PASS{RESET} --format json outputs {len(lines)} valid JSON line(s)")
    return True


def test_changed_only_approved() -> bool:
    """--changed-only on an approved fixture returns exit 0."""
    fixture = FIXTURE_ROOT / "python" / "fallback" / "approved" / "approved_or_default.py"
    if not fixture.exists():
        print(f"  {YELLOW}SKIP{RESET} fixture not found: {fixture}")
        return True
    proc = run_check_policy("--changed-only", str(fixture))
    if proc.returncode == 0:
        print(f"  {GREEN}PASS{RESET} --changed-only approved => exit 0")
        return True
    else:
        print(f"  {RED}FAIL{RESET} --changed-only approved => exit {proc.returncode} (expected 0)")
        if proc.stdout.strip():
            print(f"    stdout: {proc.stdout.strip()[:200]}")
        if proc.stderr.strip():
            print(f"    stderr: {proc.stderr.strip()[:200]}")
        return False


def test_test_globs_skip() -> bool:
    """--test-globs causes test files to be skipped (exit 0)."""
    # Use a should_fail fixture but pretend it's in a test directory
    # by providing a glob that matches the fixture path
    fixture = FIXTURE_ROOT / "python" / "fallback" / "should_fail" / "or_default.py"
    if not fixture.exists():
        print(f"  {YELLOW}SKIP{RESET} fixture not found: {fixture}")
        return True

    # The fixture path contains "should_fail" -- use a custom glob that matches it
    proc = run_check_policy(
        "--changed-only", str(fixture),
        "--test-globs", "**/should_fail/**",
    )
    if proc.returncode == 0:
        print(f"  {GREEN}PASS{RESET} --test-globs skips matching files => exit 0")
        return True
    else:
        print(f"  {RED}FAIL{RESET} --test-globs skip => exit {proc.returncode} (expected 0)")
        return False


def main() -> int:
    print(f"\n{YELLOW}=== test_check_policy.py ==={RESET}\n")

    tests = [
        test_changed_only_should_fail,
        test_changed_only_should_pass,
        test_format_json,
        test_changed_only_approved,
        test_test_globs_skip,
    ]

    passed = 0
    failed = 0
    for test_fn in tests:
        if test_fn():
            passed += 1
        else:
            failed += 1

    print(f"\n{'=' * 40}")
    print(f"Total: {GREEN}{passed} passed{RESET}, {RED}{failed} failed{RESET}")
    return 1 if failed > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
