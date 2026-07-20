#!/usr/bin/env bash
# smoke-demo.sh — smoke test for demo seeding and API availability
set -euo pipefail

cd "$(dirname "$0")/../.."

cargo build -p agent-meter-collector

AGENT_METER_NO_OPEN=1 \
DATABASE_URL="sqlite:///tmp/agent-meter-smoke-demo.db" \
AGENT_METER_PORT=18081 \
AGENT_METER_OTLP_PORT=14318 \
  ./target/debug/agent-meter-collector demo --conversations 3 --events 5 --force &
SERVER_PID=$!

cleanup() { kill "$SERVER_PID" 2>/dev/null || true; }
trap cleanup EXIT

for _ in $(seq 1 30); do
  if curl -sf "http://127.0.0.1:18081/health" >/dev/null; then
    break
  fi
  sleep 1
done

curl -sf "http://127.0.0.1:18081/health" >/dev/null || {
  echo "health check failed"
  exit 1
}

n=$(curl -sf "http://127.0.0.1:18081/api/conversations?limit=10" | grep -o '"conversation_id"' | wc -l)
echo "conversations seeded: $n"
[[ "$n" -ge 3 ]] || {
  echo "expected >= 3 conversations"
  exit 1
}

echo "✓ smoke demo OK"
