#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "[agent-meter] Running full validation harness"

"$SCRIPT_DIR/validate_copilot_eclipse.sh"
"$SCRIPT_DIR/validate_claude_code.sh"
"$SCRIPT_DIR/validate_codex_cli.sh"
"$SCRIPT_DIR/validate_mcp_semconv.sh"

echo "[agent-meter] Running complete OTLP regression suite"
cd "$SCRIPT_DIR/.."
cargo test --package agent-meter-collector --test otlp_regression

echo "[PASS] All agent OTLP validations passed"
