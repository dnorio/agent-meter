#!/usr/bin/env bash
# capture-record-smoke.sh — ensure capture-record.sh parses a real fixture dump.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIX="$ROOT/crates/collector/tests/fixtures/cursor_execute_tool.json"
OUT="$(mktemp)"
trap 'rm -f "$OUT"' EXIT

bash "$ROOT/scripts/capture-record.sh" "$FIX" --ua 'cursor/0.48.0' | tee "$OUT"
grep -q "service.name = 'cursor'" "$OUT"
grep -q "suggested manifest entry" "$OUT"
grep -q "expect_tool_names" "$OUT"
echo "✓ capture-record smoke"
