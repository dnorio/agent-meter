#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "[agent-meter] validate_copilot_eclipse: running OTLP regression test"
cd "$APP_DIR"

if cargo test --package agent-meter-collector test_otlp_eclipse_copilot_execute_tool_and_chat; then
  echo "[PASS] Copilot Eclipse OTLP ingestion/regression is healthy"
  exit 0
fi

echo "[FAIL] Copilot Eclipse OTLP regression failed"
exit 1
