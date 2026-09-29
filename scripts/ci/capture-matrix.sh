#!/usr/bin/env bash
# capture-matrix.sh — run the full capture quality matrix (fail-fast).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "[capture-matrix] 1/5 fixture contracts"
bash scripts/ci/capture-e2e.sh

echo "[capture-matrix] 2/5 proxy-shaped + proxy/mcp unit"
bash scripts/ci/capture-proxy-e2e.sh

echo "[capture-matrix] 3/5 capture-record smoke"
bash scripts/ci/capture-record-smoke.sh

echo "[capture-matrix] 4/5 otlp_regression"
cargo test -p agent-meter-collector --test otlp_regression -- --test-threads=1

echo "[capture-matrix] 5/5 LIVE MITM (required)"
CAPTURE_LIVE_MODE=mitm CAPTURE_LIVE_REQUIRED=1 bash scripts/ci/capture-live-e2e.sh

echo "[capture-matrix] OK — full matrix green"
