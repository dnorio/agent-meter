#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "[agent-meter] validate_claude_code: running OTLP regression test"
cd "$APP_DIR"

if cargo test --package agent-meter-collector test_otlp_claude_code_execute_tool_and_chat; then
  echo "[PASS] Claude Code OTLP ingestion/regression is healthy"
  exit 0
fi

echo "[FAIL] Claude Code OTLP regression failed"
exit 1
