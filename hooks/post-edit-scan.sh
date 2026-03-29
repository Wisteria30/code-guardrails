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

# --- Two-tier enforcement (Theorem 6: prefix-safe vs architectural) ---
#
# Error-swallowing rules are prefix-safe: always wrong, always locally fixable.
# These get hard-blocked immediately (decision: "block").
#
# Architectural violations (fallback defaults, test-doubles) may require
# multi-file refactoring through non-clean intermediate states.
# These get a warning only; the Stop hook enforces at completion time.

# Prefix-safe rules: error swallowing + approval injection
SWALLOW_RULES="py-no-swallowing-except-pass|ts-no-empty-catch|ts-no-promise-catch-default|py-no-fallback-contextlib-suppress|ts-no-catch-return-default"

# Extract hard-block violations (error swallowing + approval injection)
HARD_VIOLATIONS=$(echo "$OUTPUT" | jq -sc --arg rules "$SWALLOW_RULES" '
  [.[] |
    if .policy_group == "approval_injection" then .
    else {policy_group, findings: [.findings[] | select(.rule_id | test($rules))]}
    end
  | select(.findings | length > 0)]')

HARD_COUNT=$(echo "$HARD_VIOLATIONS" | jq 'length')

if [ "$HARD_COUNT" -gt 0 ]; then
  # --- HARD BLOCK: error swallowing or approval injection ---
  # Save full report to file, send capsule to Claude

  REPORT_DIR="${TMPDIR:-/tmp}/cg-reports"
  mkdir -p "$REPORT_DIR"
  REPORT_ID="cg-$(date +%Y%m%d-%H%M%S)-$$"
  REPORT_FILE="$REPORT_DIR/$REPORT_ID.json"
  echo "$HARD_VIOLATIONS" | jq '.' > "$REPORT_FILE"

  # Extract capsule fields from first finding
  FIRST=$(echo "$HARD_VIOLATIONS" | jq -r '.[0].findings[0]')
  SEMANTIC_CLASS=$(echo "$FIRST" | jq -r '.semantic_class // "unknown"')
  FILE=$(echo "$FIRST" | jq -r '.file // "unknown"')
  LINE=$(echo "$FIRST" | jq -r '.line // 0')
  OWNER=$(echo "$FIRST" | jq -r '.owner_guess // "unknown"')

  # Map semantic class to remedies and forbidden moves
  case "$SEMANTIC_CLASS" in
    fallback_unowned_handler)
      REMEDIES="typed_exception|boundary_parser|resilience_adapter"
      FORBIDDEN="rename|equivalent_rewrite|new_inline_default" ;;
    fallback_unowned_default)
      REMEDIES="boundary_parser|approved_policy_default|optional_exhaustive|typed_exception"
      FORBIDDEN="rename|equivalent_rewrite|new_inline_default" ;;
    runtime_double_in_graph)
      REMEDIES="move_to_tests|promote_to_adapter"
      FORBIDDEN="rename|test_support_in_runtime" ;;
    approval_injection)
      REMEDIES="rewrite_without_approval_comment"
      FORBIDDEN="add_policy_approved_comment" ;;
    *)
      REMEDIES="boundary_parser|typed_exception|approved_policy_default"
      FORBIDDEN="rename|equivalent_rewrite|new_inline_default" ;;
  esac

  HARD_TOTAL=$(echo "$HARD_VIOLATIONS" | jq '[.[].findings | length] | add // 0')

  jq -n \
    --arg semantic_class "$SEMANTIC_CLASS" \
    --arg file "$FILE" \
    --arg line "$LINE" \
    --arg owner "$OWNER" \
    --arg remedies "$REMEDIES" \
    --arg forbidden "$FORBIDDEN" \
    --arg report "$REPORT_FILE" \
    --arg report_id "$REPORT_ID" \
    --arg total "$HARD_TOTAL" \
    '{
      decision: "block",
      reason: ("Guardrail: " + $semantic_class + " at " + $file + ":" + $line + ". " + $total + " violation(s). Repair at " + $owner + " layer or raise typed error. Report: " + $report),
      systemMessage: ("Detailed report saved to " + $report),
      hookSpecificOutput: {
        hookEventName: "PostToolUse",
        additionalContext: ("guardrail_id=" + $report_id + " class=" + $semantic_class + " owner_guess=" + $owner + " remedies=" + $remedies + " forbidden=" + $forbidden)
      }
    }'
else
  # --- WARN ONLY: architectural violations (fallback defaults, test-doubles) ---
  # These require owner-layer identification and possibly multi-file refactoring.
  # The Stop hook enforces at completion time.
  TOTAL=$(echo "$OUTPUT" | jq -sc '[.[].findings | length] | add // 0')
  echo "code-guardrails: ${TOTAL} architectural violation(s) detected (fallback defaults or test-doubles). Run /fix before completing to resolve via subagents." >&2
  exit 0
fi

exit 0
