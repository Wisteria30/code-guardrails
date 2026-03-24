#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
CHECK_POLICY="$PLUGIN_DIR/check_policy.py"

# Read JSON from stdin, extract file_path with jq (fast) or python3 (fallback)
INPUT=$(cat)
if command -v jq &>/dev/null; then
  FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // .tool_input.content // ""' 2>/dev/null || echo "")
else
  FILE_PATH=$(echo "$INPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    ti = d.get('tool_input', {})
    print(ti.get('file_path', ti.get('content', '')))
except Exception:
    print('')
" 2>/dev/null || echo "")
fi

if [ -z "$FILE_PATH" ]; then
  exit 0
fi

# Run policy check
set +e
OUTPUT=$(python3 "$CHECK_POLICY" --config-dir "$PLUGIN_DIR" --changed-only "$FILE_PATH" --format json 2>/dev/null)
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

# Violations found — format and output to stderr (exit 2 feeds stderr to Claude)
{
  echo ""
  echo "=== CODE GUARDRAILS: Policy Violations Found ==="
  echo ""
  if command -v jq &>/dev/null; then
    echo "$OUTPUT" | jq -r '"\(.file):\(.line):\(.column) [\(.rule_id)] \(.message)" + if .code != "" then "\n    code: \(.code)" else "" end' 2>/dev/null || echo "$OUTPUT"
  else
    echo "$OUTPUT" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        f = json.loads(line)
        print(f\"  {f['file']}:{f['line']}:{f['column']} [{f['rule_id']}] {f['message']}\")
        if f.get('code'):
            print(f\"    code: {f['code'][:200]}\")
    except (json.JSONDecodeError, KeyError):
        print(f'  {line}')
"
  fi
  echo ""
  echo "Fix these violations before proceeding. For intentional fallbacks, add:"
  echo "  # policy-approved: REQ-xxx <reason>"
  echo "=== END CODE GUARDRAILS ==="
} >&2

exit 2
