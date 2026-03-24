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
        cwd=str(SGCONFIG.parent),
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
