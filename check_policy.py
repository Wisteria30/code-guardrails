#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

APPROVAL_RE = re.compile(
    r'policy-approved:\s*(REQ|ADR|SPEC)-[A-Za-z0-9._-]+',
    re.IGNORECASE,
)

@dataclass
class Finding:
    file: str
    line0: int
    column0: int
    rule_id: str
    severity: str
    message: str
    note: str | None
    text: str
    metadata: dict

def iter_json_stream(stdout: str) -> Iterator[dict]:
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        yield json.loads(line)

def read_lines(path: Path) -> list[str]:
    try:
        return path.read_text(encoding='utf-8').splitlines()
    except UnicodeDecodeError:
        return path.read_text(encoding='utf-8', errors='replace').splitlines()

def has_adjacent_policy_approval(file_path: Path, line0: int) -> bool:
    lines = read_lines(file_path)
    candidates: list[str] = []

    if 0 <= line0 < len(lines):
        candidates.append(lines[line0])

    for idx in (line0 - 1, line0 - 2):
        if 0 <= idx < len(lines):
            candidates.append(lines[idx])

    return any(APPROVAL_RE.search(raw.strip()) for raw in candidates)

def to_finding(item: dict) -> Finding:
    return Finding(
        file=item['file'],
        line0=item['range']['start']['line'],
        column0=item['range']['start']['column'],
        rule_id=item.get('ruleId', '<unknown-rule>'),
        severity=item.get('severity', 'hint'),
        message=item.get('message', ''),
        note=item.get('note'),
        text=item.get('text', ''),
        metadata=item.get('metadata') or {},
    )

def run_ast_grep(root: Path, ast_grep_bin: str) -> list[Finding]:
    cmd = [
        ast_grep_bin,
        'scan',
        '--json=stream',
        '--include-metadata',
        '.',
    ]
    try:
        proc = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=str(root),
        )
    except FileNotFoundError as exc:
        print(
            f'Could not execute {ast_grep_bin!r}. Install ast-grep and/or pass --ast-grep-bin.',
            file=sys.stderr,
        )
        raise SystemExit(2) from exc

    # ast-grep returns non-zero when error rules fire, so we still parse stdout.
    if proc.returncode not in (0, 1):
        print(proc.stderr, file=sys.stderr)
        raise SystemExit(proc.returncode)

    return [to_finding(item) for item in iter_json_stream(proc.stdout)]

def is_approved(finding: Finding, root: Path) -> bool:
    approval_mode = finding.metadata.get('approval_mode', 'none')
    if approval_mode != 'adjacent_policy_comment':
        return False
    return has_adjacent_policy_approval(root / finding.file, finding.line0)

def main() -> int:
    parser = argparse.ArgumentParser(
        description='Run ast-grep and enforce explicit approval comments for fallback rules.'
    )
    parser.add_argument('root', nargs='?', default='.', help='Project root')
    parser.add_argument('--ast-grep-bin', default='ast-grep', help='ast-grep binary name/path')
    args = parser.parse_args()

    root = Path(args.root).resolve()
    findings = run_ast_grep(root, args.ast_grep_bin)

    unsuppressed: list[Finding] = []
    approved: list[Finding] = []

    for finding in findings:
        if is_approved(finding, root):
            approved.append(finding)
        else:
            unsuppressed.append(finding)

    for finding in unsuppressed:
        line1 = finding.line0 + 1
        col1 = finding.column0 + 1
        print(
            f'{finding.file}:{line1}:{col1}: '
            f'{finding.severity}[{finding.rule_id}] {finding.message}'
        )
        if finding.note:
            print(f'  note: {finding.note}')
        snippet = finding.text.strip().replace('\n', ' ')
        if snippet:
            print(f'  code: {snippet[:200]}')

    if approved:
        print('', file=sys.stderr)
        print(f'approved findings filtered by wrapper: {len(approved)}', file=sys.stderr)

    return 1 if unsuppressed else 0

if __name__ == '__main__':
    raise SystemExit(main())
