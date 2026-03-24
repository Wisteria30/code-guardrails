#!/usr/bin/env python3
from __future__ import annotations

import argparse
import fnmatch
import json
import re
import yaml
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

APPROVAL_RE = re.compile(
    r'policy-approved:\s*(REQ|ADR|SPEC)-[A-Za-z0-9._-]+',
    re.IGNORECASE,
)

DEFAULT_TEST_GLOBS = (
    '**/test/**',
    '**/tests/**',
    '**/*_test.py',
    '**/test_*.py',
    '**/conftest.py',
    '**/*.test.ts',
    '**/*.spec.ts',
    '**/__tests__/**',
)

# ripgrep pre-filter pattern for candidate files
_RG_PATTERN = r'mock|stub|fake|= .* or |\?\?|\|\||except.*pass|catch|suppress|getattr|getenv|\.get\('

# Module-level file cache
_FILE_CACHE: dict[Path, list[str]] = {}

# Module-level rule metadata cache (ruleId -> metadata dict)
_RULE_METADATA: dict[str, dict] = {}


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


def _load_rule_metadata(config_dir: Path) -> dict[str, dict]:
    """Load metadata from all rule YAML files under the config directory.

    Returns a mapping from ruleId to metadata dict.
    """
    global _RULE_METADATA
    if _RULE_METADATA:
        return _RULE_METADATA

    sgconfig = config_dir / 'sgconfig.yml'
    rule_dirs: list[str] = ['rules']
    if sgconfig.exists():
        try:
            doc = yaml.safe_load(sgconfig.read_text(encoding='utf-8'))
            if isinstance(doc, dict) and 'ruleDirs' in doc:
                rule_dirs = doc['ruleDirs']
        except (OSError, yaml.YAMLError):
            pass

    seen_dirs: set[str] = set()
    for rd in rule_dirs:
        rd_path = config_dir / rd
        if not rd_path.is_dir() or str(rd_path) in seen_dirs:
            continue
        seen_dirs.add(str(rd_path))
        for yml_file in rd_path.glob('*.yml'):
            _parse_rule_yaml(yml_file)

    return _RULE_METADATA


def _parse_rule_yaml(yml_path: Path) -> None:
    """Parse a rule YAML to extract id and metadata block."""
    try:
        text = yml_path.read_text(encoding='utf-8')
        doc = yaml.safe_load(text)
    except (OSError, UnicodeDecodeError, yaml.YAMLError):
        return
    if not isinstance(doc, dict):
        return
    rule_id = doc.get('id')
    metadata = doc.get('metadata')
    if rule_id and isinstance(metadata, dict):
        _RULE_METADATA[rule_id] = {str(k): str(v) for k, v in metadata.items()}


def iter_json_stream(stdout: str) -> Iterator[dict]:
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            # Skip non-JSON lines (e.g. ast-grep error/help messages)
            continue


def read_lines(path: Path) -> list[str]:
    try:
        return path.read_text(encoding='utf-8').splitlines()
    except UnicodeDecodeError:
        return path.read_text(encoding='utf-8', errors='replace').splitlines()


def read_lines_cached(path: Path) -> list[str]:
    """Read file lines with module-level caching."""
    resolved = path.resolve()
    if resolved not in _FILE_CACHE:
        _FILE_CACHE[resolved] = read_lines(resolved)
    return _FILE_CACHE[resolved]


def has_adjacent_policy_approval(file_path: Path, line0: int) -> bool:
    lines = read_lines_cached(file_path)
    candidates: list[str] = []

    if 0 <= line0 < len(lines):
        candidates.append(lines[line0])

    for idx in (line0 - 1, line0 - 2):
        if 0 <= idx < len(lines):
            candidates.append(lines[idx])

    return any(APPROVAL_RE.search(raw.strip()) for raw in candidates)


def to_finding(item: dict, rule_metadata: dict[str, dict]) -> Finding:
    rule_id = item.get('ruleId', '<unknown-rule>')
    # Merge metadata from the JSON output (if present) with rule-file metadata
    metadata = dict(rule_metadata.get(rule_id, {}))
    if item.get('metadata'):
        metadata.update(item['metadata'])
    return Finding(
        file=item['file'],
        line0=item['range']['start']['line'],
        column0=item['range']['start']['column'],
        rule_id=rule_id,
        severity=item.get('severity', 'hint'),
        message=item.get('message', ''),
        note=item.get('note'),
        text=item.get('text', ''),
        metadata=metadata,
    )


def matches_test_globs(filepath: str, test_globs: tuple[str, ...]) -> bool:
    """Check if a file path matches any of the test glob patterns."""
    for pattern in test_globs:
        if fnmatch.fnmatch(filepath, pattern):
            return True
        # Also check sub-paths for directory-based patterns
        parts = Path(filepath).parts
        for i in range(len(parts)):
            sub = '/'.join(parts[i:])
            if fnmatch.fnmatch(sub, pattern):
                return True
    return False


def ripgrep_candidate_files(root: Path) -> list[str] | None:
    """Use ripgrep to pre-filter files that might contain policy violations.

    Returns a list of file paths, or None if ripgrep is not available
    or finds no candidates (fall back to full scan).
    """
    cmd = [
        'rg',
        '--files-with-matches',
        '-e', _RG_PATTERN,
        '--type', 'py',
        '--type', 'ts',
        str(root),
    ]
    try:
        proc = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except FileNotFoundError:
        # ripgrep not installed -- fall back to full scan
        return None

    if proc.returncode not in (0, 1):
        # rg error -- fall back to full scan
        return None

    files = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    return files if files else None


def run_ast_grep(
    config_dir: Path,
    ast_grep_bin: str,
    rule_metadata: dict[str, dict],
    targets: list[str] | None = None,
) -> list[Finding]:
    """Run ast-grep scan.

    Args:
        config_dir: Directory containing sgconfig.yml (used as cwd).
        ast_grep_bin: Path/name of ast-grep binary.
        rule_metadata: Mapping from rule ID to metadata dict.
        targets: If provided, scan these specific files/paths. Otherwise scan '.'.
    """
    if targets is not None:
        all_findings: list[Finding] = []
        for target in targets:
            cmd = [ast_grep_bin, 'scan', '--json=stream', target]
            try:
                proc = subprocess.run(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    cwd=str(config_dir),
                )
            except FileNotFoundError as exc:
                print(
                    f'Could not execute {ast_grep_bin!r}. Install ast-grep and/or pass --ast-grep-bin.',
                    file=sys.stderr,
                )
                raise SystemExit(2) from exc

            if proc.returncode not in (0, 1):
                print(proc.stderr, file=sys.stderr)
                raise SystemExit(2)

            all_findings.extend(
                to_finding(item, rule_metadata) for item in iter_json_stream(proc.stdout)
            )
        return all_findings
    else:
        cmd = [ast_grep_bin, 'scan', '--json=stream', '.']
        try:
            proc = subprocess.run(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                cwd=str(config_dir),
            )
        except FileNotFoundError as exc:
            print(
                f'Could not execute {ast_grep_bin!r}. Install ast-grep and/or pass --ast-grep-bin.',
                file=sys.stderr,
            )
            raise SystemExit(2) from exc

        if proc.returncode not in (0, 1):
            print(proc.stderr, file=sys.stderr)
            raise SystemExit(2)

        return [to_finding(item, rule_metadata) for item in iter_json_stream(proc.stdout)]


def is_approved(finding: Finding, root: Path) -> bool:
    approval_mode = finding.metadata.get('approval_mode', 'none')
    if approval_mode != 'adjacent_policy_comment':
        return False
    return has_adjacent_policy_approval(root / finding.file, finding.line0)


def format_human(unsuppressed: list[Finding], approved: list[Finding], use_color: bool) -> None:
    """Print findings in human-readable format, optionally with ANSI colors."""
    if use_color:
        RED = '\033[91m'
        YELLOW = '\033[93m'
        CYAN = '\033[96m'
        DIM = '\033[2m'
        RESET = '\033[0m'
    else:
        RED = YELLOW = CYAN = DIM = RESET = ''

    for finding in unsuppressed:
        line1 = finding.line0 + 1
        col1 = finding.column0 + 1
        print(
            f'{CYAN}{finding.file}:{line1}:{col1}{RESET}: '
            f'{RED}{finding.severity}{RESET}[{YELLOW}{finding.rule_id}{RESET}] {finding.message}'
        )
        if finding.note:
            print(f'  {DIM}note: {finding.note}{RESET}')
        snippet = finding.text.strip().replace('\n', ' ')
        if snippet:
            print(f'  {DIM}code: {snippet[:200]}{RESET}')

    if approved:
        print('', file=sys.stderr)
        print(f'approved findings filtered by wrapper: {len(approved)}', file=sys.stderr)


def format_json(unsuppressed: list[Finding]) -> None:
    """Print findings as JSON lines (one JSON object per line)."""
    for finding in unsuppressed:
        obj = {
            'file': finding.file,
            'line': finding.line0 + 1,
            'column': finding.column0 + 1,
            'rule_id': finding.rule_id,
            'severity': finding.severity,
            'message': finding.message,
            'code': finding.text.strip().replace('\n', ' ')[:200] if finding.text else '',
        }
        print(json.dumps(obj, ensure_ascii=False))


def main() -> int:
    script_dir = Path(__file__).resolve().parent

    parser = argparse.ArgumentParser(
        description='Run ast-grep and enforce explicit approval comments for fallback rules.'
    )
    parser.add_argument('root', nargs='?', default='.', help='Project root to scan')
    parser.add_argument('--ast-grep-bin', default='ast-grep', help='ast-grep binary name/path')
    parser.add_argument(
        '--changed-only',
        metavar='FILE',
        help='Scan a single file instead of the full project',
    )
    parser.add_argument(
        '--format',
        choices=['human', 'json'],
        default='human',
        help='Output format (default: human)',
    )
    parser.add_argument(
        '--test-globs',
        default=','.join(DEFAULT_TEST_GLOBS),
        help='Comma-separated glob patterns for test files (skipped with --changed-only)',
    )
    parser.add_argument(
        '--config-dir',
        default=None,
        help='Directory containing sgconfig.yml (default: directory of check_policy.py)',
    )
    args = parser.parse_args()

    config_dir = Path(args.config_dir).resolve() if args.config_dir else script_dir
    test_globs = tuple(g.strip() for g in args.test_globs.split(',') if g.strip())

    # Load rule metadata from YAML files
    rule_metadata = _load_rule_metadata(config_dir)

    # --changed-only mode: scan a single file
    if args.changed_only:
        filepath = Path(args.changed_only)

        # Check if file matches test globs -- if so, skip scan
        file_str = str(filepath)
        if matches_test_globs(file_str, test_globs):
            return 0

        # Make the path relative to config_dir for ast-grep
        try:
            abs_path = filepath.resolve()
            rel_path = abs_path.relative_to(config_dir)
        except ValueError:
            # File is not under config_dir -- use absolute path
            rel_path = abs_path

        findings = run_ast_grep(config_dir, args.ast_grep_bin, rule_metadata, targets=[str(rel_path)])

        # For --changed-only, root for approval checking is config_dir
        root = config_dir
    else:
        root = Path(args.root).resolve()

        # Try ripgrep pre-filter optimization for full project scan
        candidate_files = ripgrep_candidate_files(root)
        if candidate_files is not None:
            # Make paths relative to config_dir
            rel_targets = []
            for f in candidate_files:
                try:
                    rel = Path(f).resolve().relative_to(config_dir)
                    rel_targets.append(str(rel))
                except ValueError:
                    rel_targets.append(f)
            findings = run_ast_grep(config_dir, args.ast_grep_bin, rule_metadata, targets=rel_targets)
        else:
            findings = run_ast_grep(config_dir, args.ast_grep_bin, rule_metadata)

    unsuppressed: list[Finding] = []
    approved: list[Finding] = []

    for finding in findings:
        if is_approved(finding, root):
            approved.append(finding)
        else:
            unsuppressed.append(finding)

    if args.format == 'json':
        format_json(unsuppressed)
    else:
        use_color = sys.stdout.isatty()
        format_human(unsuppressed, approved, use_color)

    return 1 if unsuppressed else 0


if __name__ == '__main__':
    raise SystemExit(main())
