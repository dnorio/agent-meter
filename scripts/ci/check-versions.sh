#!/usr/bin/env bash
# check-versions.sh — fail when workspace and SDK versions diverge
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

workspace_version="$(grep '^version' "$ROOT/Cargo.toml" | head -1 | sed 's/.*"\([^"]*\)".*/\1/')"
node_version="$(node -p "require('$ROOT/sdk/node/package.json').version")"
python_version="$(grep '^version' "$ROOT/sdk/python/pyproject.toml" | head -1 | sed 's/.*"\([^"]*\)".*/\1/')"

if [[ "$workspace_version" != "$node_version" || "$workspace_version" != "$python_version" ]]; then
  echo "version mismatch:" >&2
  echo "  workspace: $workspace_version" >&2
  echo "  npm:       $node_version" >&2
  echo "  python:    $python_version" >&2
  exit 1
fi

echo "✓ versions synced at $workspace_version"
