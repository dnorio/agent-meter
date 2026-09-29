#!/usr/bin/env bash
# capture-matrix.sh — run the full capture quality matrix (fail-fast).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "[capture-matrix] 0/6 coverage alignment"
bash scripts/ci/capture-matrix-coverage.sh

echo "[capture-matrix] 1/6 fixture contracts"
bash scripts/ci/capture-e2e.sh

echo "[capture-matrix] 2/6 proxy-shaped + proxy/mcp unit"
bash scripts/ci/capture-proxy-e2e.sh

echo "[capture-matrix] 3/6 capture-record smoke"
bash scripts/ci/capture-record-smoke.sh

echo "[capture-matrix] 4/6 otlp_regression"
cargo test -p agent-meter-collector --test otlp_regression -- --test-threads=1

echo "[capture-matrix] 5/6 LIVE MITM (required)"
CAPTURE_LIVE_MODE=mitm CAPTURE_LIVE_REQUIRED=1 bash scripts/ci/capture-live-e2e.sh

echo "[capture-matrix] OK — full matrix green"
