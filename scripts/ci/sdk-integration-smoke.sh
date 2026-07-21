#!/usr/bin/env bash
# sdk-integration-smoke.sh — SDK → collector end-to-end smoke
set -euo pipefail

log() { printf '[sdk-integration-smoke] %s\n' "$*"; }

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PORT="${AGENT_METER_SMOKE_PORT:-18081}"
DB="$ROOT/target/sdk-integration-smoke.db"
BIN="$ROOT/target/debug/agent-meter-collector"

rm -f "$DB"
export DATABASE_URL="sqlite://$DB"
export AGENT_METER_HOST=127.0.0.1
export AGENT_METER_PORT="$PORT"
export AGENT_METER_OTLP_PORT=14318
export RUST_LOG=warn

log "building collector"
cargo build -p agent-meter-collector

log "starting collector on :$PORT"
"$BIN" serve &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 30); do
  if curl -sf "http://127.0.0.1:$PORT/health" >/dev/null; then
    break
  fi
  sleep 1
done
curl -sf "http://127.0.0.1:$PORT/health" | grep -q '"status":"ok"'

log "REST ingest smoke"
curl -sf -X POST "http://127.0.0.1:$PORT/events/tool-call" \
  -H 'Content-Type: application/json' \
  -d '{
    "tool_name": "sdk_smoke_rest",
    "conversation_id": "sdk-smoke-conv",
    "started_at": "2026-05-17T00:00:00Z",
    "ended_at": "2026-05-17T00:00:01Z",
    "ok": true
  }' >/dev/null

sleep 1
curl -sf "http://127.0.0.1:$PORT/api/conversations?limit=5" | grep -q sdk-smoke-conv

log "Python SDK OTLP smoke"
(
  cd "$ROOT/sdk/python"
  PYTHONPATH=. ENDPOINT="http://127.0.0.1:$PORT" python3 - <<'PY'
import os
from agent_meter import AgentMeter

endpoint = os.environ["ENDPOINT"]
am = AgentMeter(endpoint=endpoint, flush_interval=999)
span = am.track("sdk_smoke_python", model="gpt-4o")
span.finish()
assert am.flush() == 1, "expected one span flushed"
am._closed = True
if am._timer:
    am._timer.cancel()
PY
)

sleep 1
curl -sf "http://127.0.0.1:$PORT/reports/top-tools?limit=10" | grep -q sdk_smoke_python

log "Node SDK OTLP smoke"
(
  cd "$ROOT/sdk/node"
  ENDPOINT="http://127.0.0.1:$PORT" node --input-type=module <<'JS'
import { AgentMeter } from "./dist/index.js";

const endpoint = process.env.ENDPOINT;
const am = new AgentMeter({ endpoint, flushInterval: 999 });
const span = am.track("sdk_smoke_node", { model: "gpt-4o" });
am.finish(span);
const sent = await am.flush();
if (sent !== 1) throw new Error(`expected 1 span, got ${sent}`);
await am.shutdown();
JS
)

sleep 1
curl -sf "http://127.0.0.1:$PORT/reports/top-tools?limit=10" | grep -q sdk_smoke_node

log "✓ SDK integration smoke OK"
