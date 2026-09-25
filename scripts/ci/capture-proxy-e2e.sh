#!/usr/bin/env bash
# Capture proxy e2e — collector + synthetic OTLP payloads shaped like agent-meter-proxy.
# Proves the proxy→collector path (scope agent-meter-proxy) without HTTPS MITM in CI.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PORT="${CAPTURE_PROXY_E2E_PORT:-$((17000 + RANDOM % 1000))}"
OTLP_PORT="${CAPTURE_PROXY_E2E_OTLP_PORT:-$((17500 + RANDOM % 1000))}"
DB="/tmp/agent-meter-capture-proxy-e2e-$$.db"
BIN="${ROOT}/target/debug/agent-meter-collector"
LOG="/tmp/agent-meter-capture-proxy-e2e-$$.log"

echo "[capture-proxy-e2e] building collector + running proxy/mcp tests"
cargo build -p agent-meter-collector -q
cargo test -p agent-meter-proxy --quiet -- --test-threads=2
cargo test -p agent-meter-mcp-wrapper --tests --quiet -- --test-threads=2

echo "[capture-proxy-e2e] starting collector :${PORT} otlp :${OTLP_PORT}"
rm -f "$DB"
AGENT_METER_NO_OPEN=1 \
  DATABASE_URL="sqlite://${DB}" \
  AGENT_METER_HOST=127.0.0.1 \
  AGENT_METER_PORT="$PORT" \
  AGENT_METER_OTLP_PORT="$OTLP_PORT" \
  "$BIN" serve >"$LOG" 2>&1 &
PID=$!
cleanup() {
  kill "$PID" 2>/dev/null || true
  wait "$PID" 2>/dev/null || true
  rm -f "$DB"
}
trap cleanup EXIT

for _ in $(seq 1 50); do
  curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null && break
  sleep 0.2
done
curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null || {
  echo "[capture-proxy-e2e] collector failed"
  tail -40 "$LOG" || true
  exit 1
}

python3 - "$PORT" "$OTLP_PORT" <<'PY'
import json, sys, time, urllib.request, uuid

port, otlp_port = sys.argv[1:3]
base = f"http://127.0.0.1:{port}"
otlp = f"http://127.0.0.1:{otlp_port}/v1/traces"

def http(method, url, body=None, headers=None):
    req = urllib.request.Request(url, data=body, method=method, headers=headers or {})
    with urllib.request.urlopen(req, timeout=20) as resp:
        return resp.status, resp.read()

def now_ns():
    return int(time.time() * 1e9)

def proxy_payload(service, span_name, attrs):
    """Mirrors crates/proxy/src/otlp.rs build_otlp_payload shape."""
    tid = uuid.uuid4().hex
    sid = uuid.uuid4().hex[:16]
    otlp_attrs = []
    for k, v in attrs:
        if isinstance(v, str):
            av = {"stringValue": v}
        elif isinstance(v, bool):
            av = {"boolValue": v}
        elif isinstance(v, int):
            av = {"intValue": str(v)}
        else:
            av = {"stringValue": str(v)}
        otlp_attrs.append({"key": k, "value": av})
    start = now_ns()
    end = start + 50_000_000
    return {
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": service}},
                    {"key": "service.namespace", "value": {"stringValue": "ide"}},
                ]
            },
            "scopeSpans": [{
                "scope": {"name": "agent-meter-proxy", "version": "0.0.0-e2e"},
                "spans": [{
                    "traceId": tid,
                    "spanId": sid,
                    "name": span_name,
                    "kind": 3,
                    "startTimeUnixNano": str(start),
                    "endTimeUnixNano": str(end),
                    "attributes": otlp_attrs,
                    "status": {"code": 1},
                }],
            }],
        }]
    }

cases = [
    (
        "cursor",
        "execute_tool read_file",
        [
            ("gen_ai.tool.name", "read_file"),
            ("gen_ai.request.model", "gpt-4o"),
            ("gen_ai.conversation.id", "proxy-cursor-conv"),
            ("gen_ai.usage.input_tokens", 10),
            ("gen_ai.usage.output_tokens", 5),
        ],
        "cursor",
        "read_file",
        "Mozilla/5.0 cursor/0.48",
    ),
    (
        "claude",
        "chat claude-opus-4",
        [
            ("gen_ai.response.model", "claude-opus-4"),
            ("gen_ai.conversation.id", "proxy-claude-conv"),
            ("gen_ai.usage.input_tokens", 20),
            ("gen_ai.usage.output_tokens", 8),
        ],
        "claude-code",
        "llm_chat",
        "claude-code/1.0.0",
    ),
    (
        "codex",
        "execute_tool shell",
        [
            ("gen_ai.tool.name", "shell"),
            ("gen_ai.request.model", "gpt-5"),
            ("gen_ai.conversation.id", "proxy-codex-conv"),
            ("gen_ai.usage.input_tokens", 3),
            ("gen_ai.usage.output_tokens", 1),
        ],
        "codex",
        "shell",
        "codex/0.1.0",
    ),
]

for service, span, attrs, expect_ide, expect_tool, ua in cases:
    http("POST", f"{base}/api/admin/reset", body=b"{}", headers={"Content-Type": "application/json"})
    payload = proxy_payload(service, span, attrs)
    body = json.dumps(payload).encode()
    status, resp = http(
        "POST",
        otlp,
        body=body,
        headers={"Content-Type": "application/json", "User-Agent": ua},
    )
    assert status < 400, f"{service}: OTLP {status}"
    buffered = json.loads(resp)
    assert len(buffered) >= 1, f"{service}: no buffered events"

    deadline = time.time() + 20
    events = []
    while time.time() < deadline:
        _, raw = http("GET", f"{base}/reports/events?limit=50")
        events = json.loads(raw)
        if events:
            break
        time.sleep(0.15)
    assert events, f"{service}: no flushed events"
    tools = {e.get("tool_name") for e in events}
    ides = {e.get("ide") for e in events if e.get("ide")}
    assert expect_tool in tools, f"{service}: missing tool {expect_tool}, got {tools}"
    assert expect_ide in ides, f"{service}: missing ide {expect_ide}, got {ides}"
    print(f"  ✓ proxy-shaped {service} → tool={expect_tool} ide={expect_ide}")

print("[capture-proxy-e2e] OK")
PY

echo "✓ capture proxy e2e"
