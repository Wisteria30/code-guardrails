#!/usr/bin/env bash
set -euo pipefail

# Usage: scripts/release.sh 0.2.0
# Syncs version across Cargo.toml, plugin.json, and marketplace.json.

if [ -z "${1:-}" ]; then
  echo "Usage: $0 <version>" >&2
  echo "Example: $0 0.2.0" >&2
  exit 1
fi

VERSION="$1"
DIR="$(cd "$(dirname "$0")/.." && pwd)"

sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$DIR/Cargo.toml"
sed -i '' "s/\"version\": *\"[^\"]*\"/\"version\": \"$VERSION\"/" \
  "$DIR/.claude-plugin/plugin.json" \
  "$DIR/.claude-plugin/marketplace.json"

echo "Synced to $VERSION:"
echo "  Cargo.toml:       $(grep '^version' "$DIR/Cargo.toml" | head -1)"
echo "  plugin.json:      $(grep '"version"' "$DIR/.claude-plugin/plugin.json")"
echo "  marketplace.json: $(grep '"version"' "$DIR/.claude-plugin/marketplace.json")"
