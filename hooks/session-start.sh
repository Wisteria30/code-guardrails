#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
ENGINE_BIN="$PLUGIN_DIR/bin/code-guardrails-engine"
PLUGIN_JSON="$PLUGIN_DIR/.claude-plugin/plugin.json"

# Extract expected version from plugin.json
EXPECTED=$(grep -o '"version": *"[^"]*"' "$PLUGIN_JSON" | head -1 | cut -d'"' -f4)

# Check if binary exists and version matches
if [ -x "$ENGINE_BIN" ]; then
  CURRENT=$("$ENGINE_BIN" --version 2>/dev/null || echo "")
  if [ "$CURRENT" = "$EXPECTED" ]; then
    exit 0
  fi
  echo "code-guardrails: binary version ($CURRENT) != plugin version ($EXPECTED), updating..." >&2
  rm -f "$ENGINE_BIN"
fi

# Install/update
bash "$PLUGIN_DIR/setup" 2>&1
