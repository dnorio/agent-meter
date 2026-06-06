#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "[agent-meter] validate_codex_cli: running OTLP regression test"
cd "$APP_DIR"

if cargo test --package agent-meter-collector test_otlp_codex_cli_execute_tool; then
  echo "[PASS] Codex CLI OTLP ingestion/regression is healthy"
  exit 0
fi

echo "[FAIL] Codex CLI OTLP regression failed"
exit 1
