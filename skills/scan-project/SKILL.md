---
name: scan-project
description: >
  Scan the project for code-guardrails policy violations (test doubles,
  unapproved fallbacks, ownership violations). Use when asked to scan,
  check for violations, run guardrails, or verify policy compliance.
---

# Scan Project

Run the code-guardrails policy scanner on the **user's current project**.

Execute this command:
```bash
if [ -x "${CLAUDE_PLUGIN_ROOT}/bin/code-guardrails-engine" ]; then
  "${CLAUDE_PLUGIN_ROOT}/bin/code-guardrails-engine" scan-tree --root "$(pwd)" --config-dir "${CLAUDE_PLUGIN_ROOT}"
elif [ -x "${CLAUDE_PLUGIN_ROOT}/target/release/code-guardrails-engine" ]; then
  "${CLAUDE_PLUGIN_ROOT}/target/release/code-guardrails-engine" scan-tree --root "$(pwd)" --config-dir "${CLAUDE_PLUGIN_ROOT}"
elif [ -f "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
  cargo run --quiet --release --manifest-path "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" --bin code-guardrails-engine -- scan-tree --root "$(pwd)" --config-dir "${CLAUDE_PLUGIN_ROOT}"
else
  echo "code-guardrails-engine is not built. Run ${CLAUDE_PLUGIN_ROOT}/setup first." >&2
  exit 2
fi
```

Show the full output to the user. If violations are found:
1. List each violation with file, line, semantic class, owner guess, and code snippet
2. Group by semantic class for clarity
3. Suggest using `/fix` to fix them via subagents (preserves main context)
4. For intentional exceptions, tell the user that only human developers can add `policy-approved` comments
