---
name: scan
description: Scan the project for policy violations (test doubles and unapproved fallbacks)
---

Run the code-guardrails policy scanner on the **user's current project** (not the plugin directory).

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

`--config-dir` points to the plugin directory (where sgconfig.yml and rules/ live).
The positional argument is the user's working directory (the project to scan).

Show the full output to the user. If violations are found:
1. List each violation with file, line, rule, and code snippet
2. Suggest using `/fix` to fix them via subagents (preserves main context)
3. For intentional exceptions, tell the user that only human developers can add `policy-approved` comments
