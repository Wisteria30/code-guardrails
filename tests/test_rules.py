"""Test ast-grep rules against fixture files."""

from __future__ import annotations

import json
import subprocess
import sys

from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parent.parent
FIXTURE_ROOT = PROJECT_ROOT / "fixtures"
CHECK_POLICY = PROJECT_ROOT / "check_policy.py"


def _collect_fixtures(expect: str) -> list[tuple[Path, str]]:
    """Collect fixture files for parametrize."""
    items: list[tuple[Path, str]] = []
    for lang_dir in sorted(FIXTURE_ROOT.iterdir()):
        if not lang_dir.is_dir():
            continue
        for group_dir in sorted(lang_dir.iterdir()):
            if not group_dir.is_dir():
                continue
            target_dir = group_dir / (
                "should_fail"
                if expect == "fail"
                else "should_pass"
                if expect == "pass"
                else "approved"
            )
            if not target_dir.exists():
                continue
            for f in sorted(target_dir.iterdir()):
                if f.is_file() and not f.name.startswith("."):
                    test_id = f"{lang_dir.name}/{group_dir.name}/{target_dir.name}/{f.name}"
                    items.append((f, test_id))
    return items


def _run_ast_grep(filepath: Path) -> list[dict]:
    proc = subprocess.run(
        ["ast-grep", "scan", "--json=stream", str(filepath)],
        capture_output=True,
        text=True,
        cwd=str(PROJECT_ROOT),
    )
    findings: list[dict] = []
    for line in proc.stdout.splitlines():
        stripped = line.strip()
        if stripped:
            findings.append(json.loads(stripped))
    return findings


_should_fail = _collect_fixtures("fail")
_should_pass = _collect_fixtures("pass")
_approved = _collect_fixtures("approved")


@pytest.mark.parametrize("filepath", [f for f, _ in _should_fail], ids=[i for _, i in _should_fail])
def test_should_fail(filepath: Path) -> None:
    findings = _run_ast_grep(filepath)
    assert len(findings) > 0, f"Expected violations but got 0 for {filepath.name}"


@pytest.mark.parametrize("filepath", [f for f, _ in _should_pass], ids=[i for _, i in _should_pass])
def test_should_pass(filepath: Path) -> None:
    findings = _run_ast_grep(filepath)
    assert len(findings) == 0, (
        f"Expected 0 violations for {filepath.name}, got {len(findings)}: "
        + ", ".join(f.get("ruleId", "?") for f in findings)
    )


@pytest.mark.parametrize("filepath", [f for f, _ in _approved], ids=[i for _, i in _approved])
def test_approved(filepath: Path) -> None:
    proc = subprocess.run(
        [sys.executable, str(CHECK_POLICY), "--changed-only", str(filepath)],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 0, f"Expected exit 0 (approved) but got {proc.returncode}"
