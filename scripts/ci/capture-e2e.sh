#!/usr/bin/env bash
# Capture e2e — binary collector + OTLP fixture contracts from manifest.json.
# Asserts tool_name, ide, conversation_id, model after ingest flush.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

FIXDIR="${ROOT}/crates/collector/tests/fixtures"
MANIFEST="${FIXDIR}/manifest.json"
PORT="${CAPTURE_E2E_PORT:-$((18000 + RANDOM % 1000))}"
OTLP_PORT="${CAPTURE_E2E_OTLP_PORT:-$((19000 + RANDOM % 1000))}"
DB="/tmp/agent-meter-capture-e2e-$$.db"
BIN="${ROOT}/target/debug/agent-meter-collector"
LOG="/tmp/agent-meter-capture-e2e-$$.log"

echo "[capture-e2e] building collector"
cargo build -p agent-meter-collector -q

echo "[capture-e2e] starting collector :${PORT} otlp :${OTLP_PORT}"
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
  if curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null; then
    break
  fi
  sleep 0.2
done
curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null || {
  echo "[capture-e2e] collector failed to start"
  tail -50 "$LOG" || true
  exit 1
}

python3 - "$MANIFEST" "$FIXDIR" "$PORT" "$OTLP_PORT" <<'PY'
import json, sys, time, urllib.request, urllib.error
from pathlib import Path

manifest_path, fixdir, port, otlp_port = sys.argv[1:5]
fixdir = Path(fixdir)
manifest = json.loads(Path(manifest_path).read_text())
base = f"http://127.0.0.1:{port}"
otlp = f"http://127.0.0.1:{otlp_port}/v1/traces"

required = set(
    manifest.get("required_keys")
    or ["id", "file", "harness", "user_agent", "expect_tool_names", "min_events"]
)
fixtures = manifest.get("fixtures") or []
if not fixtures:
    raise SystemExit("manifest has zero fixtures")

listed = set()
for fx in fixtures:
    missing = required - set(fx)
    if missing:
        raise SystemExit(f"fixture {fx.get('id', '?')}: missing keys {sorted(missing)}")
    path = fixdir / fx["file"]
    if not path.is_file():
        raise SystemExit(f"fixture file missing: {path}")
    listed.add(fx["file"])
    if fx["min_events"] < 1:
        raise SystemExit(f"{fx['id']}: min_events must be >= 1")
    if not fx["expect_tool_names"]:
        raise SystemExit(f"{fx['id']}: expect_tool_names empty")

orphan = sorted(
    p.name
    for p in fixdir.glob("*.json")
    if p.name != "manifest.json" and p.name not in listed
)
if orphan:
    raise SystemExit(f"fixtures not in manifest (orphan): {orphan}")


def http(method, url, body=None, headers=None):
    req = urllib.request.Request(url, data=body, method=method, headers=headers or {})
    with urllib.request.urlopen(req, timeout=20) as resp:
        return resp.status, resp.read()


def wait_events(min_n, timeout=25.0):
    deadline = time.time() + timeout
    last = []
    while time.time() < deadline:
        _, raw = http("GET", f"{base}/reports/events?limit=200")
        last = json.loads(raw)
        if len(last) >= min_n:
            return last
        time.sleep(0.15)
    raise SystemExit(f"timeout waiting for {min_n} events, got {len(last)}")


def reset():
    http(
        "POST",
        f"{base}/api/admin/reset",
        body=b"{}",
        headers={"Content-Type": "application/json"},
    )


failures = []
for fx in fixtures:
    reset()
    body = (fixdir / fx["file"]).read_bytes()
    try:
        json.loads(body)
    except json.JSONDecodeError as e:
        failures.append(f"{fx['id']}: invalid JSON ({e})")
        continue

    headers = {"Content-Type": "application/json", "User-Agent": fx["user_agent"]}
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

    convs = {e.get("conversation_id") for e in events}
    for cid in fx.get("expect_conversation_ids") or []:
        if cid not in convs:
            failures.append(
                f"{fx['id']}: expected conversation_id={cid!r}, got {sorted(c for c in convs if c)}"
            )
            break
    else:
        models_any = fx.get("expect_models_any") or []
        if models_any:
            models = {e.get("model") for e in events if e.get("model")}
            ok = any(
                any(exp in (m or "") or (m or "") in exp for m in models)
                for exp in models_any
            )
            if not ok:
                failures.append(
                    f"{fx['id']}: expected model matching {models_any}, got {sorted(models)}"
                )
                continue

        bad = [
            e.get("event_id")
            for e in events
            if not e.get("started_at") or e.get("duration_ms") is None
        ]
        if bad:
            failures.append(f"{fx['id']}: events missing timestamps: {bad[:3]}")
            continue

        print(
            f"  ✓ {fx['id']} ({fx['harness']}) "
            f"events={len(events)} ide={expect_ide or '-'} tools={sorted(names)}"
        )
        continue

    # conversation_id failure already recorded
    continue

if failures:
    print("[capture-e2e] FAILURES:")
    for f in failures:
        print("  -", f)
    sys.exit(1)

print(f"[capture-e2e] OK — {len(fixtures)} harness contracts passed (strict)")
PY

echo "✓ capture e2e"
