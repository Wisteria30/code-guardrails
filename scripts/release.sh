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

# Validate: new version must be greater than current
CURRENT=$(grep '^version' "$DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r CUR_MAJOR CUR_MINOR CUR_PATCH <<< "$CURRENT"
IFS='.' read -r NEW_MAJOR NEW_MINOR NEW_PATCH <<< "$VERSION"
CUR_NUM=$((CUR_MAJOR * 10000 + CUR_MINOR * 100 + ${CUR_PATCH:-0}))
NEW_NUM=$((NEW_MAJOR * 10000 + NEW_MINOR * 100 + ${NEW_PATCH:-0}))
if [ "$NEW_NUM" -le "$CUR_NUM" ]; then
  echo "Error: new version ($VERSION) must be greater than current ($CURRENT)" >&2
  exit 1
fi

sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$DIR/Cargo.toml"
sed -i '' "s/\"version\": *\"[^\"]*\"/\"version\": \"$VERSION\"/" \
  "$DIR/.claude-plugin/plugin.json" \
  "$DIR/.claude-plugin/marketplace.json"

echo "Synced to $VERSION:"
echo "  Cargo.toml:       $(grep '^version' "$DIR/Cargo.toml" | head -1)"
echo "  plugin.json:      $(grep '"version"' "$DIR/.claude-plugin/plugin.json")"
echo "  marketplace.json: $(grep '"version"' "$DIR/.claude-plugin/marketplace.json")"

echo ""
echo "Building release binary (updates Cargo.lock)..."
cargo build --release --manifest-path "$DIR/Cargo.toml"
cp "$DIR/target/release/code-guardrails-engine" "$DIR/bin/"
echo "Done: bin/code-guardrails-engine updated"
