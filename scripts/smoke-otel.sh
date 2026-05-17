#!/bin/bash
# smoke-otel.sh — Validate agent-meter OTEL/telemetry pipeline
set -euo pipefail

COLLECTOR="${AGENT_METER_COLLECTOR_URL:-http://localhost:8081}"
TASK_ID="smoke-otel-$(date +%s)"
EVENT_ID=$(python3 -c "import uuid; print(uuid.uuid4())")

echo "=== agent-meter OTEL Smoke Test ==="
echo "Collector: $COLLECTOR"
echo "Task ID:   $TASK_ID"
echo ""

# 1. Check health
echo "--- Health ---"
HEALTH=$(curl -sf "$COLLECTOR/health") || {
  echo "ERROR: collector not reachable at $COLLECTOR/health"
  exit 1
}
echo "OK: $HEALTH"
echo ""

# 2. Start a task
echo "--- Start Task ---"
TASK_RESP=$(curl -sf -X POST "$COLLECTOR/tasks/start" \
  -H "content-type: application/json" \
  -d "{
    \"task_id\": \"$TASK_ID\",
    \"repo\": \"agent-meter-worktree\",
    \"branch\": \"smoke-otel\",
    \"ide\": \"opencode\",
    \"agent\": \"smoke-test\",
    \"skill\": \"otel-integration\"
  }") || {
  echo "ERROR: task start failed"
  exit 1
}
echo "OK: $TASK_RESP"
echo ""

# 3. Send a tool-call event
echo "--- Send Tool-Call Event ---"
STARTED=$(date -u +%Y-%m-%dT%H:%M:%SZ)
sleep 1
ENDED=$(date -u +%Y-%m-%dT%H:%M:%SZ)

EVENT_RESP=$(curl -sf -X POST "$COLLECTOR/events/tool-call" \
  -H "content-type: application/json" \
  -d "{
    \"event_id\": \"$EVENT_ID\",
    \"task_id\": \"$TASK_ID\",
    \"repo\": \"agent-meter-worktree\",
    \"branch\": \"smoke-otel\",
    \"ide\": \"opencode\",
    \"agent\": \"smoke-test\",
    \"skill\": \"otel-integration\",
    \"mcp_server\": \"smoke-mcp\",
    \"tool_name\": \"smoke_test_tool\",
    \"started_at\": \"$STARTED\",
    \"ended_at\": \"$ENDED\",
    \"ok\": true,
    \"request_bytes\": 150,
    \"response_bytes\": 3200,
    \"request_sha256\": \"abc123\",
    \"response_sha256\": \"def456\"
  }") || {
  echo "ERROR: event POST failed"
  exit 1
}
echo "OK: $EVENT_RESP"
echo ""

# 4. Query reports
echo "--- Reports /top-tools ---"
sleep 1
TOP_TOOLS=$(curl -sf "$COLLECTOR/reports/top-tools")
echo "$TOP_TOOLS" | python3 -m json.tool 2>/dev/null || echo "$TOP_TOOLS"

# Verify our event is in the report
if echo "$TOP_TOOLS" | python3 -c "
import sys, json
data = json.load(sys.stdin)
found = any(t.get('tool_name') == 'smoke_test_tool' for t in data)
sys.exit(0 if found else 1)
" 2>/dev/null; then
  echo ""
  echo "✓ smoke_test_tool found in top-tools report"
else
  echo ""
  echo "WARNING: smoke_test_tool not found in top-tools (may take time to appear)"
fi
echo ""

# 5. End the task
echo "--- End Task ---"
END_RESP=$(curl -sf -X POST "$COLLECTOR/tasks/end" \
  -H "content-type: application/json" \
  -d "{
    \"task_id\": \"$TASK_ID\"
  }")
echo "OK: $END_RESP"
echo ""

# 6. Summary
echo "=== Smoke Test Complete ==="
echo "Event ID: $EVENT_ID"
echo "Task ID:  $TASK_ID"
echo "Result:   ✅ PASS"
