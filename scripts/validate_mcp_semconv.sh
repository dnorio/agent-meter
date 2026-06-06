#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "[agent-meter] validate_mcp_semconv: running OTLP regression test"
cd "$APP_DIR"

if cargo test --package agent-meter-collector test_otlp_mcp_semconv_tools_call; then
  echo "[PASS] MCP semconv tools/call parser is healthy"
  exit 0
fi

echo "[FAIL] MCP semconv tools/call regression failed"
exit 1
