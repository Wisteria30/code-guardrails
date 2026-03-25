---
name: fix
description: Fix all code-guardrails violations in the project using subagents (preserves main context)
---

Fix all code-guardrails policy violations in the user's project by delegating to subagents.

## Step 1: Scan the project

Run a full scan to find all violations:

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

## Step 2: Group violations by file

Parse the scan output and group violations by file path. Show the user a summary:
- Total violation count
- Violations per file
- Categories (fallback, test-double, keyword-comment)

Ask the user if they want to proceed with fixes.

## Step 3: Spawn subagents to fix violations

For each file (or batch of related files), spawn a subagent using the Agent tool:

- **description**: "Fix guardrail violations in <filename>"
- **prompt**: Read the fix prompt template from `${CLAUDE_PLUGIN_ROOT}/agents/fix-prompt.md`, replace `{{VIOLATIONS}}` with the violations for that file, and include the scan command for verification:
  `${CLAUDE_PLUGIN_ROOT}/bin/code-guardrails-engine scan-file --file <path> --config-dir ${CLAUDE_PLUGIN_ROOT}`

Spawn up to 5 subagents in parallel per batch. If there are more than 5 files, process in batches of 5. Two subagents must NOT edit the same file.

## Step 4: Report results

After all subagents complete, run the full scan again to verify. Report:
- Files fixed
- Remaining violations (if any)
- Summary of architectural changes made

## Important

- Do NOT fix violations in the main conversation context — always use subagents
- Do NOT add `policy-approved` comments
- Do NOT make semantic-equivalent rewrites (`.get()` → `if key in dict`, `getattr` → `hasattr`)
- Fix the root cause: missing schemas, unclear contracts, misplaced responsibilities
