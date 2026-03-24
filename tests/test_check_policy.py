"""Tests for check_policy.py CLI."""

from __future__ import annotations

import json
import subprocess
import sys

from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
CHECK_POLICY = PROJECT_ROOT / "check_policy.py"
FIXTURE_ROOT = PROJECT_ROOT / "fixtures"


def _run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECK_POLICY), *args],
        capture_output=True,
        text=True,
    )


class TestChangedOnly:
    def test_violation_exits_1(self) -> None:
        fixture = FIXTURE_ROOT / "python" / "fallback" / "should_fail" / "or_default.py"
        result = _run("--changed-only", str(fixture))
        assert result.returncode == 1

    def test_clean_exits_0(self) -> None:
        fixture = FIXTURE_ROOT / "python" / "fallback" / "should_pass" / "no_fallback.py"
        result = _run("--changed-only", str(fixture))
        assert result.returncode == 0

    def test_approved_exits_0(self) -> None:
        fixture = FIXTURE_ROOT / "python" / "fallback" / "approved" / "approved_or_default.py"
        result = _run("--changed-only", str(fixture))
        assert result.returncode == 0


class TestFormatJson:
    def test_outputs_valid_json_lines(self) -> None:
        fixture = FIXTURE_ROOT / "python" / "fallback" / "should_fail" / "or_default.py"
        result = _run("--changed-only", str(fixture), "--format", "json")
        assert result.returncode == 1
        lines = [line for line in result.stdout.strip().splitlines() if line.strip()]
        assert len(lines) > 0
        for line in lines:
            obj = json.loads(line)
            for key in ("file", "line", "column", "rule_id", "severity", "message", "code"):
                assert key in obj, f"Missing key: {key}"


class TestMetadataParsing:
    def test_approval_model_works(self) -> None:
        fixture = FIXTURE_ROOT / "python" / "fallback" / "approved" / "approved_get_default.py"
        result = _run("--changed-only", str(fixture), "--format", "json")
        assert result.returncode == 0
        assert not result.stdout.strip()


class TestTestGlobs:
    def test_skips_matching_files(self) -> None:
        fixture = FIXTURE_ROOT / "python" / "fallback" / "should_fail" / "or_default.py"
        result = _run("--changed-only", str(fixture), "--test-globs", "**/should_fail/**")
        assert result.returncode == 0
