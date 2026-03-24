# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is this project?

code-guardrails is a Claude Code plugin that detects and blocks test doubles (mock/stub/fake) and unapproved fallbacks (swallowed exceptions, silent defaults) that AI coding tools sneak into production code. Built with a Rust engine + ast-grep rules.

## Commands

```bash
# Build the Rust engine
cargo build --release

# Run Rust tests (unit + integration)
cargo test --all-targets

# Run a specific test
cargo test <test_name>

# Lint
cargo fmt --check
cargo clippy -- -D warnings

# Full project scan
./bin/code-guardrails-engine scan-tree --root . --config-dir .

# Single file scan
./bin/code-guardrails-engine scan-file --file path/to/file.py --config-dir .

# Run without pre-built binary
cargo run --quiet --release --bin code-guardrails-engine -- scan-tree --root . --config-dir .

# Release (syncs version across Cargo.toml, plugin.json, marketplace.json)
scripts/release.sh <version>
```

## Architecture

### Engine (Rust)

All source code lives in `src/main.rs`. CLI with 3 subcommands:

- **`scan-file`** — Scan a single file with ast-grep rules
- **`scan-tree`** — Full project scan. Pre-filters candidate files with ripgrep, then batch-invokes ast-grep (`BATCH_SIZE=128`)
- **`scan-hook`** — For PostToolUse hook. Reads Claude Code's tool_input JSON from stdin, extracts the changed file path, and scans it

Output is JSON. Findings are suppressed if an adjacent line contains a `policy-approved: REQ-xxx` / `ADR-xxx` / `SPEC-xxx` comment.

### Rules (`rules/`)

ast-grep YAML rules. 17 total (9 Python + 8 TypeScript). `sgconfig.yml` points to the `rules/` directory.

Naming convention: `{lang}-no-{category}-{pattern}.yml` (e.g., `py-no-fallback-get-default.yml`, `ts-no-empty-catch.yml`)

Each rule controls scope via `files` / `ignores`, excluding test paths (`**/test/**`, `**/*.test.ts`, etc.) and auto-generated paths (`**/generated/**`, `**/openapi/**`, etc.).

### Hooks (`hooks/`)

- **`session-start.sh`** — Checks binary version against `plugin.json` at session start. Re-runs `setup` on mismatch
- **`post-edit-scan.sh`** — Scans after every Edit/Write. Returns `decision: block` JSON on violations, forcing Claude to fix them

### Tests

- **`tests/integration_cli.rs`** — Rust integration tests. Runs the engine binary against files in `fixtures/` to verify detection accuracy
- **`fixtures/`** — Test fixtures in 3 categories: `should_fail/` (must be detected), `should_pass/` (must not be detected), `approved/` (must be suppressed by policy-approved)

### Versioning

Version is defined in 3 places: `Cargo.toml` (single source of truth), `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`. Use `scripts/release.sh` to sync all three.

CI `version-lint` job enforces version bump when `src/`, `hooks/`, `rules/`, `commands/`, etc. are changed.

## Agent Guidelines

Always prefer simplicity over pathological correctness. YAGNI, KISS, DRY.
No backward-compat shims or fallback paths unless they come free without adding cyclomatic complexity.

The core stack is Rust + ast-grep + ripgrep. Before introducing new dependencies (Python, Node, etc.), first consider whether the task can be accomplished with the existing stack.

Performance is as important as — or more important than — detection accuracy. This plugin runs on every Edit/Write via PostToolUse hook, so scan-file and scan-hook must stay under 100ms. Never add processing that degrades hot-path latency (e.g., spawning extra processes, reading unnecessary files, network calls).

Each tool has a clear responsibility based on what it's fastest at. Do NOT reimplement what rg or ast-grep already do better:

- **ripgrep** — File discovery and plain-text filtering. rg is faster than any hand-written Rust glob/regex file scanner. Use it for candidate pre-filtering, file type matching, and string searches
- **ast-grep** — Syntax-aware pattern matching. Use it for structural code patterns that require AST understanding (e.g., `$VAR = $A || $B`)
- **Rust engine** — Orchestration only. Parses CLI args, invokes rg/ast-grep, reads their output, checks approval comments, and formats JSON results

Never rewrite rg's file traversal or text search in pure Rust thinking it will be faster — it won't. rg uses memory-mapped I/O, SIMD-accelerated search, parallel directory traversal, and a DFA-based regex engine — a naive Rust walkdir+regex cannot compete. In benchmarks on ~200 files, `rg -l` runs in ~64ms vs ~123ms for find+grep (the equivalent of hand-written traversal). Never rewrite ast-grep's pattern matching in Rust either. The engine's job is glue code, not reimplementing its dependencies.

Reference performance (scan-file hot path, the one that matters for PostToolUse):

| Case | Mean |
|---|---|
| scan-file (clean, no violations) | ~3ms |
| scan-file (with violations) | ~42ms |
| raw ast-grep single file | ~47ms |

## AI Code Policy

code-guardrails hook is active. Every Edit/Write is scanned for violations.

- NEVER write `except: pass`, empty `catch {}`, or `.catch(() => null)` — log the error and re-raise or wrap
- NEVER use `mock`, `stub`, `fake` identifiers in production code — test doubles belong in test files only
- NEVER add silent defaults (`.get(key, default)`, `?? value`, `|| value`) without spec approval — fail explicitly or mark with `# policy-approved: REQ-xxx <reason>`
- NEVER leave TODO/placeholder/stub implementations — implement fully or raise NotImplementedError
- Unspecified fallbacks are bugs. If the spec doesn't say "default to X", don't default to X

## Key Patterns

- When adding a new rule: create a YAML file in `rules/`, add corresponding fixtures to `fixtures/{lang}/{category}/should_fail/` and `should_pass/`, and add test cases to `tests/integration_cli.rs`
- Fallback rules should set `metadata.approval_mode: adjacent_policy_comment` to support suppression via `policy-approved:` comments
- TypeScript rules target `.ts` / `.cts` / `.mts` only; `.tsx` / `.jsx` are not supported (see `rules/tsx-note.txt`)
