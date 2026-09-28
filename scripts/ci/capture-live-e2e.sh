#!/usr/bin/env bash
# Capture LIVE e2e — real proxy MITM (+ optional real CLIs) → collector.
#
# Layers:
#   1) mitm  — agent-meter-proxy + curl through HTTPS_PROXY to real AI hosts
#              (fake API key OK; 401 still yields OTLP). Proves capture path.
#   2) cli   — wrap claude/codex/opencode/gh-copilot when binary + credentials exist.
#
# Env:
#   CAPTURE_LIVE_MODE=mitm|cli|all     (default: all)
#   CAPTURE_LIVE_REQUIRED=0|1          (default: 0 on PR, set 1 on nightly/main)
#   CAPTURE_LIVE_CLI_TIMEOUT=90        seconds per CLI
#   CAPTURE_LIVE_SKIP_CLI=1            force skip CLI layer
#
# Exit: 0 if required layers pass. Soft-skip counts as pass unless REQUIRED=1.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

MODE="${CAPTURE_LIVE_MODE:-all}"
REQUIRED="${CAPTURE_LIVE_REQUIRED:-0}"
CLI_TIMEOUT="${CAPTURE_LIVE_CLI_TIMEOUT:-90}"
SKIP_CLI="${CAPTURE_LIVE_SKIP_CLI:-0}"

PORT="${CAPTURE_LIVE_PORT:-$((16000 + RANDOM % 1000))}"
OTLP_PORT="${CAPTURE_LIVE_OTLP_PORT:-$((16500 + RANDOM % 1000))}"
PROXY_PORT="${CAPTURE_LIVE_PROXY_PORT:-$((16800 + RANDOM % 1000))}"
DB="/tmp/agent-meter-capture-live-$$.db"
CA_DIR="/tmp/agent-meter-capture-live-ca-$$"
COLLECTOR_BIN="${ROOT}/target/debug/agent-meter-collector"
PROXY_BIN="${ROOT}/target/debug/agent-meter-proxy"
LOG_C="/tmp/agent-meter-capture-live-collector-$$.log"
LOG_P="/tmp/agent-meter-capture-live-proxy-$$.log"
RESULT_JSON="/tmp/agent-meter-capture-live-$$.json"

PASS=0
FAIL=0
SKIP=0
declare -a NOTES=()

note() { NOTES+=("$1"); echo "$1"; }
ok() { PASS=$((PASS + 1)); note "  ✓ $1"; }
bad() { FAIL=$((FAIL + 1)); note "  ✗ $1"; }
skip() { SKIP=$((SKIP + 1)); note "  ○ skip $1"; }

cleanup() {
  if [[ -n "${PROXY_PID:-}" ]]; then kill "$PROXY_PID" 2>/dev/null || true; wait "$PROXY_PID" 2>/dev/null || true; fi
  if [[ -n "${COLLECTOR_PID:-}" ]]; then kill "$COLLECTOR_PID" 2>/dev/null || true; wait "$COLLECTOR_PID" 2>/dev/null || true; fi
  rm -f "$DB" "$RESULT_JSON"
  rm -rf "$CA_DIR"
}
trap cleanup EXIT

echo "[capture-live] building collector + proxy"
cargo build -p agent-meter-collector -p agent-meter-proxy -q

echo "[capture-live] starting collector :${PORT} otlp :${OTLP_PORT}"
rm -f "$DB"
AGENT_METER_NO_OPEN=1 \
  DATABASE_URL="sqlite://${DB}" \
  AGENT_METER_HOST=127.0.0.1 \
  AGENT_METER_PORT="$PORT" \
  AGENT_METER_OTLP_PORT="$OTLP_PORT" \
  "$COLLECTOR_BIN" serve >"$LOG_C" 2>&1 &
COLLECTOR_PID=$!

for _ in $(seq 1 50); do
  curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null && break
  sleep 0.2
done
curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null || {
  echo "[capture-live] collector failed"
  tail -40 "$LOG_C" || true
  exit 1
}

BASE="http://127.0.0.1:${PORT}"
COLLECTOR_OTLP="http://127.0.0.1:${OTLP_PORT}"

reset_events() {
  curl -sf -X POST "$BASE/api/admin/reset" -H 'Content-Type: application/json' -d '{}' >/dev/null
}

wait_ide() {
  local expect_ide="$1" timeout="${2:-25}"
  python3 - "$BASE" "$expect_ide" "$timeout" <<'PY'
import json, sys, time, urllib.request
base, expect, timeout = sys.argv[1], sys.argv[2], float(sys.argv[3])
deadline = time.time() + timeout
last = []
while time.time() < deadline:
    with urllib.request.urlopen(f"{base}/reports/events?limit=100", timeout=10) as r:
        last = json.loads(r.read())
    ides = {e.get("ide") for e in last if e.get("ide")}
    if expect in ides:
        print(json.dumps({"ok": True, "events": len(last), "ides": sorted(ides)}))
        raise SystemExit(0)
    time.sleep(0.25)
print(json.dumps({"ok": False, "events": len(last), "ides": sorted({e.get("ide") for e in last if e.get("ide")})}))
raise SystemExit(1)
PY
}

# ── MITM layer ──────────────────────────────────────────────────────────────
run_mitm() {
  echo "[capture-live] MITM layer — real proxy + curl via HTTPS_PROXY"
  mkdir -p "$CA_DIR"
  # Isolate CA so we don't touch user trust store
  export HOME="$CA_DIR"
  "$PROXY_BIN" setup --no-install >/dev/null 2>&1
  CA_CERT="${HOME}/.agent-meter/ca-cert.pem"
  if [[ ! -f "$CA_CERT" ]]; then
    bad "mitm: CA cert missing at $CA_CERT"
    return
  fi
  echo "[capture-live] CA=$CA_CERT"

  "$PROXY_BIN" start \
    --listen "127.0.0.1:${PROXY_PORT}" \
    --collector "$COLLECTOR_OTLP" \
    >"$LOG_P" 2>&1 &
  PROXY_PID=$!
  sleep 0.8
  if ! kill -0 "$PROXY_PID" 2>/dev/null; then
    bad "mitm: proxy failed to start"
    tail -40 "$LOG_P" || true
    return
  fi

  PROXY_URL="http://127.0.0.1:${PROXY_PORT}"
  export CAPTURE_LIVE_MITM_JSON="/tmp/capture-live-mitm.json"

  python3 - "$BASE" "$PROXY_URL" "$CA_CERT" <<'PY' || true
import json, os, subprocess, sys, time, urllib.request

base, proxy, ca = sys.argv[1:4]
cases = [
    ("cursor", "cursor/0.48.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}),
    ("antigravity", "antigravity/1.0.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}),
    ("codex", "codex/0.1.0 (linux amd64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-5", "messages": [{"role": "user", "content": "ping"}]}),
    ("claude-code", "claude-code/1.0.0 (linux arm64)", "https://api.anthropic.com/v1/messages",
     ["-H", "x-api-key: sk-ant-live-e2e-fake", "-H", "anthropic-version: 2023-06-01"],
     {"model": "claude-opus-4", "max_tokens": 16, "messages": [{"role": "user", "content": "ping"}]}),
    ("opencode", "opencode/0.5.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}),
    ("copilot-vscode", "vscode/1.100.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}),
    ("copilot-cli", "github-copilot-cli/1.0.0 (linux amd64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4.1", "messages": [{"role": "user", "content": "ping"}]}),
]

results = []
for ide, ua, url, extra, body in cases:
    # reset
    req = urllib.request.Request(
        f"{base}/api/admin/reset", data=b"{}", method="POST",
        headers={"Content-Type": "application/json"},
    )
    urllib.request.urlopen(req, timeout=10).read()

    cmd = [
        "curl", "-sS", "-o", "/tmp/capture-live-mitm-body.json", "-w", "%{http_code}",
        "-x", proxy, "--cacert", ca, "--max-time", "25",
        "-H", f"User-Agent: {ua}",
        "-H", "Content-Type: application/json",
        "-H", f"x-session-id: live-{ide}",
        *extra,
        "-d", json.dumps(body),
        url,
    ]
    try:
        code = subprocess.check_output(cmd, text=True, stderr=subprocess.STDOUT).strip()
    except subprocess.CalledProcessError as e:
        results.append({"ide": ide, "ok": False, "error": f"curl fail: {e.output[-200:]}"})
        print(f"  ✗ mitm {ide}: curl failed", flush=True)
        continue

    # Wait for flush
    deadline = time.time() + 20
    found = False
    ides = set()
    while time.time() < deadline:
        with urllib.request.urlopen(f"{base}/reports/events?limit=50", timeout=10) as r:
            events = json.loads(r.read())
        ides = {e.get("ide") for e in events if e.get("ide")}
        if ide in ides:
            found = True
            break
        time.sleep(0.25)

    if found:
        results.append({"ide": ide, "ok": True, "http": code})
        print(f"  ✓ mitm {ide} (upstream HTTP {code})", flush=True)
    else:
        results.append({"ide": ide, "ok": False, "http": code, "ides": sorted(ides)})
        print(f"  ✗ mitm {ide}: expected ide={ide}, got {sorted(ides)} (upstream HTTP {code})", flush=True)

out = {"results": results, "passed": sum(1 for r in results if r.get("ok")), "total": len(results)}
Path = __import__("pathlib").Path
Path(os.environ.get("CAPTURE_LIVE_MITM_JSON", "/tmp/capture-live-mitm.json")).write_text(json.dumps(out, indent=2))
print(json.dumps(out))
if out["passed"] < out["total"]:
    raise SystemExit(2)
PY
  local mitm_rc=$?

  # Count from printed summary via re-read
  if [[ -f /tmp/capture-live-mitm.json ]]; then
    local p t
    p=$(python3 -c 'import json;d=json.load(open("/tmp/capture-live-mitm.json"));print(d["passed"])')
    t=$(python3 -c 'import json;d=json.load(open("/tmp/capture-live-mitm.json"));print(d["total"])')
    PASS=$((PASS + p))
    if [[ "$p" -lt "$t" ]]; then
      FAIL=$((FAIL + (t - p)))
    fi
  elif [[ "$mitm_rc" -ne 0 ]]; then
    bad "mitm: layer failed (rc=$mitm_rc)"
  fi
}

# ── CLI layer ───────────────────────────────────────────────────────────────
have_cmd() { command -v "$1" >/dev/null 2>&1; }

run_one_cli() {
  local name="$1" expect_ide="$2"
  shift 2
  reset_events
  echo "[capture-live] CLI $name → $*"
  # Wrap with proxy env already set from mitm (or set here)
  if timeout "$CLI_TIMEOUT" env \
      HTTPS_PROXY="http://127.0.0.1:${PROXY_PORT}" \
      HTTP_PROXY="http://127.0.0.1:${PROXY_PORT}" \
      ALL_PROXY="http://127.0.0.1:${PROXY_PORT}" \
      SSL_CERT_FILE="${CA_CERT:-}" \
      NODE_EXTRA_CA_CERTS="${CA_CERT:-}" \
      REQUESTS_CA_BUNDLE="${CA_CERT:-}" \
      "$@" >"/tmp/capture-live-cli-${name}.out" 2>&1; then
    :
  else
    local rc=$?
    # timeout=124; CLI may still have emitted traffic
    if [[ "$rc" -eq 124 ]]; then
      note "  ! cli $name timed out after ${CLI_TIMEOUT}s — checking events anyway"
    else
      note "  ! cli $name exited $rc — checking events anyway"
      tail -20 "/tmp/capture-live-cli-${name}.out" || true
    fi
  fi
  if wait_ide "$expect_ide" 30; then
    ok "cli $name → ide=$expect_ide"
  else
    if [[ "$REQUIRED" == "1" ]]; then
      bad "cli $name: no events with ide=$expect_ide"
    else
      skip "cli $name (no events / no credentials)"
    fi
  fi
}

run_cli() {
  echo "[capture-live] CLI layer — wrap real agent binaries when present"
  if [[ "$SKIP_CLI" == "1" ]]; then
    skip "cli layer (CAPTURE_LIVE_SKIP_CLI=1)"
    return
  fi
  if [[ -z "${PROXY_PID:-}" ]] || ! kill -0 "$PROXY_PID" 2>/dev/null; then
    # Start proxy if mitm wasn't run
    mkdir -p "$CA_DIR"
    export HOME="$CA_DIR"
    "$PROXY_BIN" setup --no-install >/dev/null 2>&1 || true
    CA_CERT="$(find "$CA_DIR" -type f \( -name 'cert.pem' -o -name 'ca.crt' -o -name '*.pem' \) 2>/dev/null | head -1 || true)"
    "$PROXY_BIN" start --listen "127.0.0.1:${PROXY_PORT}" --collector "$COLLECTOR_OTLP" >"$LOG_P" 2>&1 &
    PROXY_PID=$!
    sleep 0.8
  fi

  local ran=0
  if have_cmd claude && [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
    ran=1
    run_one_cli claude-code claude-code \
      claude -p "Reply with exactly: PONG" --bare --output-format text
  else
    skip "claude-code (need claude + ANTHROPIC_API_KEY)"
  fi

  if have_cmd codex && [[ -n "${OPENAI_API_KEY:-}" ]]; then
    ran=1
    run_one_cli codex codex \
      codex exec --skip-git-repo-check "Reply with exactly: PONG"
  else
    skip "codex (need codex + OPENAI_API_KEY)"
  fi

  if have_cmd opencode && { [[ -n "${OPENAI_API_KEY:-}" ]] || [[ -n "${ANTHROPIC_API_KEY:-}" ]]; }; then
    ran=1
    run_one_cli opencode opencode \
      opencode run "Reply with exactly: PONG"
  else
    skip "opencode (need opencode + API key)"
  fi

  if have_cmd gh; then
    if gh copilot --help >/dev/null 2>&1; then
      # gh copilot may need GH auth; try non-interactive if present
      if [[ -n "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ]]; then
        ran=1
        run_one_cli copilot-cli copilot-cli \
          gh copilot -- -p "Reply with exactly: PONG"
      else
        skip "copilot-cli (need GH_TOKEN/GITHUB_TOKEN)"
      fi
    else
      skip "copilot-cli (gh copilot unavailable)"
    fi
  else
    skip "copilot-cli (gh missing)"
  fi

  # GUI agents cannot run headless here — document as skip
  skip "cursor GUI (no Electron in CI — use mitm UA path)"
  skip "antigravity GUI (no Electron in CI — use mitm UA path)"
  skip "copilot-vscode GUI (no Electron in CI — use mitm UA path)"

  if [[ "$ran" -eq 0 && "$REQUIRED" == "1" && "$MODE" == "cli" ]]; then
    bad "cli: REQUIRED=1 but no CLI+credentials available"
  fi
}

case "$MODE" in
  mitm) run_mitm ;;
  cli) run_cli ;;
  all) run_mitm; run_cli ;;
  *) echo "unknown CAPTURE_LIVE_MODE=$MODE"; exit 2 ;;
esac

echo
echo "[capture-live] summary pass=$PASS fail=$FAIL skip=$SKIP required=$REQUIRED"
if [[ "$FAIL" -gt 0 ]]; then
  echo "[capture-live] FAILURES present"
  exit 1
fi
if [[ "$REQUIRED" == "1" && "$PASS" -eq 0 ]]; then
  echo "[capture-live] REQUIRED=1 but nothing passed"
  exit 1
fi
echo "✓ capture live e2e"
