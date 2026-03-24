#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
ENGINE_BIN="$PLUGIN_DIR/bin/code-guardrails-engine"
ENGINE_CMD=()

# Resolve engine binary
if [ -x "$ENGINE_BIN" ]; then
  ENGINE_CMD=("$ENGINE_BIN")
elif [ -x "$PLUGIN_DIR/target/release/code-guardrails-engine" ]; then
  ENGINE_CMD=("$PLUGIN_DIR/target/release/code-guardrails-engine")
elif [ -f "$PLUGIN_DIR/Cargo.toml" ] && command -v cargo &>/dev/null; then
  ENGINE_CMD=(cargo run --quiet --release --manifest-path "$PLUGIN_DIR/Cargo.toml" --bin code-guardrails-engine --)
else
  {
    echo ""
    echo "=== CODE GUARDRAILS: Rust Toolchain Required ==="
    echo ""
    echo "The code-guardrails engine binary is not built yet."
    echo "The Rust toolchain (cargo/rustc) is required to compile it."
    echo ""
    echo "Please ask the user if they want to install Rust via rustup."
    echo "If the user agrees, run the following commands:"
    echo ""
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    echo "  source \"\$HOME/.cargo/env\""
    echo "  $PLUGIN_DIR/setup"
    echo ""
    echo "After installation, the engine will be compiled and policy scanning will be active."
    echo "=== END CODE GUARDRAILS ==="
  } >&2
  exit 2
fi

# scan-hook reads stdin (tool_input JSON), extracts file_path, scans, outputs JSON
set +e
OUTPUT=$(cat | "${ENGINE_CMD[@]}" scan-hook --config-dir "$PLUGIN_DIR" 2>/dev/null)
EXIT_CODE=$?
set -e

# Exit code 2 = tool error, fail-open
if [ "$EXIT_CODE" -eq 2 ]; then
  echo "code-guardrails: scan error (fail-open)" >&2
  exit 0
fi

# Exit code 0 = clean
if [ "$EXIT_CODE" -eq 0 ] || [ -z "$OUTPUT" ]; then
  exit 0
fi

# Violations found — output structured JSON to stdout with decision:block
# Claude Code parses stdout JSON on exit 0; exit 2 only shows plain stderr text
REASON="CODE GUARDRAILS: Policy violations found. Fix these violations before proceeding. For intentional fallbacks, add: # policy-approved: REQ-xxx <reason>"

jq -n \
  --arg reason "$REASON" \
  --arg context "$OUTPUT" \
  '{
    decision: "block",
    reason: $reason,
    hookSpecificOutput: {
      hookEventName: "PostToolUse",
      additionalContext: $context
    }
  }'

exit 0
