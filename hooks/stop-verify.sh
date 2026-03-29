#!/usr/bin/env bash
set -euo pipefail

# Stop hook: verify all violations are resolved before allowing completion.
#
# Theorem 6: architectural violations (fallback defaults, test-doubles) are
# NOT prefix-safe — enforcing cleanliness on every edit blocks multi-file
# refactoring. Instead, we warn on each edit (PostToolUse) and enforce here
# at completion time.

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
ENGINE_BIN="$PLUGIN_DIR/bin/code-guardrails-engine"
ENGINE_CMD=()

# Read hook input from stdin
INPUT=$(cat)

# Prevent infinite loop: if stop_hook_active is true, this is a retry
# after the previous stop was blocked. Allow completion on retry to avoid
# trapping the user in an unbreakable loop.
if [ "$(echo "$INPUT" | jq -r '.stop_hook_active // false')" = "true" ]; then
  exit 0
fi

# Resolve engine binary
if [ -x "$ENGINE_BIN" ]; then
  ENGINE_CMD=("$ENGINE_BIN")
elif [ -x "$PLUGIN_DIR/target/release/code-guardrails-engine" ]; then
  ENGINE_CMD=("$PLUGIN_DIR/target/release/code-guardrails-engine")
elif [ -f "$PLUGIN_DIR/Cargo.toml" ] && command -v cargo &>/dev/null; then
  ENGINE_CMD=(cargo run --quiet --release --manifest-path "$PLUGIN_DIR/Cargo.toml" --bin code-guardrails-engine --)
else
  # Engine not available — fail-open, don't block completion
  exit 0
fi

# Run full project scan
set +e
OUTPUT=$("${ENGINE_CMD[@]}" scan-tree --root "$(pwd)" --config-dir "$PLUGIN_DIR" 2>/dev/null)
EXIT_CODE=$?
set -e

# Exit code 2 = tool error, fail-open
if [ "$EXIT_CODE" -eq 2 ]; then
  exit 0
fi

# Exit code 0 = clean, allow completion
if [ "$EXIT_CODE" -eq 0 ] || [ -z "$OUTPUT" ]; then
  exit 0
fi

# Violations remain — block completion
TOTAL=$(echo "$OUTPUT" | jq -sc '[.[].findings | length] | add // 0')

# Build a concise summary grouped by semantic class
SUMMARY=$(echo "$OUTPUT" | jq -sc '
  [.[].findings[]] |
  group_by(.semantic_class) |
  map({
    class: (.[0].semantic_class // "unknown"),
    count: length,
    files: ([.[].file] | unique | join(", "))
  }) |
  map(.class + " (" + (.count | tostring) + "): " + .files) |
  join("\n  ")
')

jq -n \
  --arg total "$TOTAL" \
  --arg summary "$SUMMARY" \
  '{
    decision: "block",
    reason: ("CODE GUARDRAILS: " + $total + " violation(s) remain. Fix them before completing.\n\nFiles:\n  " + $summary + "\n\nRun /fix to delegate fixes to subagents (preserves main context), or ask the user to add policy-approved comments for intentional exceptions.")
  }'
