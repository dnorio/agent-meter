#!/usr/bin/env bash
# Capture e2e — start collector binary, replay OTLP fixtures from manifest.json,
# assert tool_name + ide landed in /reports/events after ingest flush.
#
# This is the CI-safe substitute for spinning up VS Code / Cursor / Eclipse GUIs
# (no display, no licenses, no flaky Electron on Jenkins agents).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

FIXDIR="${ROOT}/crates/collector/tests/fixtures"
MANIFEST="${FIXDIR}/manifest.json"
PORT="${CAPTURE_E2E_PORT:-$((18000 + RANDOM % 1000))}"
OTLP_PORT="${CAPTURE_E2E_OTLP_PORT:-$((19000 + RANDOM % 1000))}"
DB="/tmp/agent-meter-capture-e2e-$$.db"
BIN="${ROOT}/target/debug/agent-meter-collector"

echo "[capture-e2e] building collector"
cargo build -p agent-meter-collector -q

echo "[capture-e2e] starting collector :${PORT} otlp :${OTLP_PORT}"
rm -f "$DB"
AGENT_METER_NO_OPEN=1 \
  DATABASE_URL="sqlite://${DB}" \
  AGENT_METER_HOST=127.0.0.1 \
  AGENT_METER_PORT="$PORT" \
  AGENT_METER_OTLP_PORT="$OTLP_PORT" \
  "$BIN" serve >/tmp/agent-meter-capture-e2e.log 2>&1 &
PID=$!
cleanup() {
  kill "$PID" 2>/dev/null || true
  wait "$PID" 2>/dev/null || true
  rm -f "$DB"
}
trap cleanup EXIT

for _ in $(seq 1 40); do
  if curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null; then
    break
  fi
  sleep 0.25
done
curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null || {
  echo "[capture-e2e] collector failed to start"
  tail -40 /tmp/agent-meter-capture-e2e.log || true
  exit 1
}

python3 - "$MANIFEST" "$FIXDIR" "$PORT" "$OTLP_PORT" <<'PY'
import json, sys, time, urllib.request, urllib.error
from pathlib import Path

manifest_path, fixdir, port, otlp_port = sys.argv[1:5]
manifest = json.loads(Path(manifest_path).read_text())
base = f"http://127.0.0.1:{port}"
otlp = f"http://127.0.0.1:{otlp_port}/v1/traces"

def http(method, url, body=None, headers=None):
    req = urllib.request.Request(url, data=body, method=method, headers=headers or {})
    with urllib.request.urlopen(req, timeout=15) as resp:
        return resp.status, resp.read()

def wait_events(min_n, timeout=20.0):
    deadline = time.time() + timeout
    last = []
    while time.time() < deadline:
        _, raw = http("GET", f"{base}/reports/events?limit=200")
        last = json.loads(raw)
        if len(last) >= min_n:
            return last
        time.sleep(0.2)
    raise SystemExit(f"timeout waiting for {min_n} events, got {len(last)}")

# Reset between fixtures so counts are isolated
def reset():
    http("POST", f"{base}/api/admin/reset", body=b"{}", headers={"Content-Type": "application/json"})

failures = []
for fx in manifest["fixtures"]:
    reset()
    path = Path(fixdir) / fx["file"]
    body = path.read_bytes()
    headers = {
        "Content-Type": "application/json",
        "User-Agent": fx["user_agent"],
    }
    try:
        status, resp = http("POST", otlp, body=body, headers=headers)
    except urllib.error.HTTPError as e:
        failures.append(f"{fx['id']}: OTLP HTTP {e.code}")
        continue
    if status >= 400:
        failures.append(f"{fx['id']}: OTLP status {status}")
        continue
    buffered = json.loads(resp)
    if len(buffered) < fx["min_events"]:
        failures.append(
            f"{fx['id']}: buffered={len(buffered)} < min_events={fx['min_events']}"
        )
        continue

    events = wait_events(fx["min_events"])
    names = {e.get("tool_name") for e in events}
    missing = [t for t in fx["expect_tool_names"] if t not in names]
    if missing:
        failures.append(f"{fx['id']}: missing tools {missing}; got {sorted(names)}")
        continue

    expect_ide = fx.get("expect_ide")
    if expect_ide:
        ides = {e.get("ide") for e in events if e.get("ide")}
        if expect_ide not in ides:
            failures.append(
                f"{fx['id']}: expected ide={expect_ide!r}, got {sorted(ides)}"
            )
            continue

    print(f"  ✓ {fx['id']} ({fx['harness']}) events={len(events)} ide={expect_ide or '-'}")

if failures:
    print("[capture-e2e] FAILURES:")
    for f in failures:
        print("  -", f)
    sys.exit(1)

print(f"[capture-e2e] OK — {len(manifest['fixtures'])} harness contracts passed")
PY

echo "✓ capture e2e"
