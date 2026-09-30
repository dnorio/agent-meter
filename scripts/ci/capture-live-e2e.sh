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
WORKDIR="$(mktemp -d /tmp/agent-meter-capture-live.XXXXXX)"
chmod 700 "$WORKDIR"
DB="${WORKDIR}/collector.db"
CA_DIR="${WORKDIR}/ca-home"
COLLECTOR_BIN="${ROOT}/target/debug/agent-meter-collector"
PROXY_BIN="${ROOT}/target/debug/agent-meter-proxy"
LOG_C="${WORKDIR}/collector.log"
LOG_P="${WORKDIR}/proxy.log"
MITM_JSON="${WORKDIR}/mitm.json"
MITM_BODY="${WORKDIR}/mitm-body.json"
CLI_OUT_DIR="${WORKDIR}/cli"
mkdir -p "$CLI_OUT_DIR"

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
  rm -rf "$WORKDIR"
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
  export CAPTURE_LIVE_MITM_JSON="$MITM_JSON"
  export CAPTURE_LIVE_MITM_BODY="$MITM_BODY"

  python3 - "$BASE" "$PROXY_URL" "$CA_CERT" "$MITM_JSON" "$MITM_BODY" <<'PY' || true
import json, subprocess, sys, time, urllib.request
from pathlib import Path

base, proxy, ca, mitm_json, mitm_body = sys.argv[1:6]
# (ide, ua, url, extra_headers, body, expect_model_substr)
cases = [
    ("cursor", "cursor/0.48.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("antigravity", "antigravity/1.0.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("codex", "codex/0.1.0 (linux amd64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-5", "messages": [{"role": "user", "content": "ping"}]}, "gpt-5"),
    ("claude-code", "claude-code/1.0.0 (linux arm64)", "https://api.anthropic.com/v1/messages",
     ["-H", "x-api-key: sk-ant-live-e2e-fake", "-H", "anthropic-version: 2023-06-01"],
     {"model": "claude-opus-4", "max_tokens": 16, "messages": [{"role": "user", "content": "ping"}]},
     "claude-opus-4"),
    ("opencode", "opencode/0.5.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("copilot-vscode", "vscode/1.100.0 (linux arm64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("copilot-cli", "github-copilot-cli/1.0.0 (linux amd64)",
     "https://api.githubcopilot.com/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4.1", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4.1"),
    ("rust-rover", "rust-rover/2025.1 (linux amd64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("copilot-eclipse", "eclipse/2026-03 jdt-language-server", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("windsurf", "Windsurf/1.2.0 (linux amd64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("jetbrains", "IntelliJ IDEA/2025.1 (linux amd64)", "https://api.openai.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]}, "gpt-4o"),
    ("gemini-cli", "gemini-cli/0.1.0 (linux amd64)",
     "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent",
     ["-H", "x-goog-api-key: sk-live-e2e-fake"],
     {"contents": [{"parts": [{"text": "ping"}]}]}, "gemini-2.0-flash"),
]

# (label, ua, url, extra, body, expect_ide, expect_model, expect_provider)
host_cases = [
    ("host-openrouter", "cursor/0.48.0 (linux arm64)",
     "https://openrouter.ai/api/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "gpt-4o", "messages": [{"role": "user", "content": "ping"}]},
     "cursor", "gpt-4o", "openrouter"),
    ("host-deepseek", "cursor/0.48.0 (linux arm64)",
     "https://api.deepseek.com/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "deepseek-chat", "messages": [{"role": "user", "content": "ping"}]},
     "cursor", "deepseek-chat", "deepseek"),
    ("host-groq", "cursor/0.48.0 (linux arm64)",
     "https://api.groq.com/openai/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "llama-3.3-70b", "messages": [{"role": "user", "content": "ping"}]},
     "cursor", "llama-3.3-70b", "groq"),
    ("host-gemini", "cursor/0.48.0 (linux arm64)",
     "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent",
     ["-H", "x-goog-api-key: sk-live-e2e-fake"],
     {"contents": [{"parts": [{"text": "ping"}]}]},
     "cursor", "gemini-2.0-flash", "google"),
    ("host-mistral", "cursor/0.48.0 (linux arm64)",
     "https://api.mistral.ai/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "mistral-small-latest", "messages": [{"role": "user", "content": "ping"}]},
     "cursor", "mistral-small-latest", "mistral"),
    ("host-fireworks", "cursor/0.48.0 (linux arm64)",
     "https://api.fireworks.ai/inference/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "accounts/fireworks/models/llama-v3p1-8b-instruct", "messages": [{"role": "user", "content": "ping"}]},
     "cursor", "llama-v3p1-8b-instruct", "fireworks"),
    ("host-xai", "cursor/0.48.0 (linux arm64)",
     "https://api.x.ai/v1/chat/completions",
     ["-H", "Authorization: Bearer sk-live-e2e-fake"],
     {"model": "grok-2", "messages": [{"role": "user", "content": "ping"}]},
     "cursor", "grok-2", "xai"),
]

def reset():
    req = urllib.request.Request(
        f"{base}/api/admin/reset", data=b"{}", method="POST",
        headers={"Content-Type": "application/json"},
    )
    urllib.request.urlopen(req, timeout=10).read()

def curl_once(label, ua, url, extra, body):
    cmd = [
        "curl", "-sS", "-o", mitm_body, "-w", "%{http_code}",
        "-x", proxy, "--cacert", ca, "--max-time", "25",
        "-H", f"User-Agent: {ua}",
        "-H", "Content-Type: application/json",
        "-H", f"x-session-id: live-{label}",
        *extra,
        "-d", json.dumps(body),
        url,
    ]
    return subprocess.check_output(cmd, text=True, stderr=subprocess.STDOUT).strip()

def check_apis(expect_conv):
    problems = []
    try:
        with urllib.request.urlopen(f"{base}/api/conversations?limit=50", timeout=10) as r:
            conv_rows = json.loads(r.read())
        if isinstance(conv_rows, dict):
            conv_rows = conv_rows.get("items") or conv_rows.get("conversations") or conv_rows.get("data") or []
        conv_ids = {
            (row.get("conversation_id") or row.get("id"))
            for row in (conv_rows or [])
            if isinstance(row, dict)
        }
        if expect_conv not in conv_ids:
            problems.append(f"api/conversations missing {expect_conv!r} (got {sorted(c for c in conv_ids if c)[:5]})")
    except Exception as e:
        problems.append(f"api/conversations error: {e}")
    try:
        with urllib.request.urlopen(f"{base}/api/conversations/{expect_conv}/timeline", timeout=10) as r:
            tl = json.loads(r.read())
        count = tl.get("event_count")
        if count is None:
            rows = tl.get("events") or tl.get("rows") or tl.get("items") or []
            count = len(rows) if isinstance(rows, list) else 0
        if not count:
            problems.append(f"timeline empty for {expect_conv!r}")
    except Exception as e:
        problems.append(f"timeline error: {e}")
    return problems

results = []
for ide, ua, url, extra, body, expect_model in cases:
    reset()
    code = None
    err = None
    for attempt in range(1, 4):
        try:
            code = curl_once(ide, ua, url, extra, body)
            err = None
            break
        except subprocess.CalledProcessError as e:
            err = e.output[-200:] if e.output else str(e)
            time.sleep(0.4 * attempt)
    if err is not None:
        results.append({"ide": ide, "ok": False, "error": f"curl fail: {err}"})
        print(f"  ✗ mitm {ide}: curl failed", flush=True)
        continue

    expect_conv = f"live-{ide}"
    deadline = time.time() + 25
    events = []
    while time.time() < deadline:
        with urllib.request.urlopen(f"{base}/reports/events?limit=50", timeout=10) as r:
            events = json.loads(r.read())
        ides = {e.get("ide") for e in events if e.get("ide")}
        if ide in ides:
            break
        time.sleep(0.25)

    ides = {e.get("ide") for e in events if e.get("ide")}
    tools = {e.get("tool_name") for e in events if e.get("tool_name")}
    convs = {e.get("conversation_id") for e in events if e.get("conversation_id")}
    models = {e.get("model") for e in events if e.get("model")}
    providers = {e.get("mcp_server") for e in events if e.get("mcp_server")}
    problems = []
    if ide not in ides:
        problems.append(f"ide missing (got {sorted(ides)})")
    if "llm_chat" not in tools:
        problems.append(f"tool llm_chat missing (got {sorted(tools)})")
    if expect_conv not in convs:
        problems.append(f"conversation_id {expect_conv!r} missing (got {sorted(convs)})")
    if not any(expect_model in (m or "") for m in models):
        if not models:
            problems.append("model empty")
        else:
            problems.append(f"model {expect_model!r} missing (got {sorted(models)})")
    bad_ts = [e.get("event_id") for e in events if not e.get("started_at") or e.get("duration_ms") is None]
    if bad_ts:
        problems.append(f"missing timestamps ({bad_ts[:2]})")
    if ide == "claude-code" and "anthropic" not in providers:
        problems.append(f"provider anthropic missing (got {sorted(providers)})")
    if ide == "copilot-cli" and "github-copilot" not in providers:
        problems.append(f"provider github-copilot missing (got {sorted(providers)})")
    if ide == "gemini-cli" and "google" not in providers:
        problems.append(f"provider google missing (got {sorted(providers)})")
    prompts = [e.get("user_prompt") or "" for e in events]
    if not any("ping" in (p or "").lower() for p in prompts):
        problems.append(f"user_prompt missing ping (got {prompts[:2]!r})")
    # Fake keys → HTTP 4xx → OTLP status ERROR → ok=false
    try:
        http_code = int(code) if code is not None else 0
    except ValueError:
        http_code = 0
    if http_code >= 400 and any(e.get("ok") is True for e in events):
        problems.append("expected ok=false for HTTP 4xx capture")
    problems.extend(check_apis(expect_conv))

    if problems:
        results.append({"ide": ide, "ok": False, "http": code, "problems": problems})
        print(f"  ✗ mitm {ide}: {'; '.join(problems)} (HTTP {code})", flush=True)
    else:
        results.append({"ide": ide, "ok": True, "http": code, "tools": sorted(tools), "model": sorted(models)})
        print(f"  ✓ mitm {ide} tools={sorted(tools)} model={sorted(models)} (HTTP {code})", flush=True)

for label, ua, url, extra, body, expect_ide, expect_model, expect_provider in host_cases:
    reset()
    code = None
    err = None
    for attempt in range(1, 4):
        try:
            code = curl_once(label, ua, url, extra, body)
            err = None
            break
        except subprocess.CalledProcessError as e:
            err = e.output[-200:] if e.output else str(e)
            time.sleep(0.4 * attempt)
    if err is not None:
        results.append({"ide": label, "ok": False, "error": f"curl fail: {err}"})
        print(f"  ✗ mitm {label}: curl failed", flush=True)
        continue

    expect_conv = f"live-{label}"
    deadline = time.time() + 25
    events = []
    while time.time() < deadline:
        with urllib.request.urlopen(f"{base}/reports/events?limit=50", timeout=10) as r:
            events = json.loads(r.read())
        if events:
            break
        time.sleep(0.25)

    ides = {e.get("ide") for e in events if e.get("ide")}
    tools = {e.get("tool_name") for e in events if e.get("tool_name")}
    convs = {e.get("conversation_id") for e in events if e.get("conversation_id")}
    models = {e.get("model") for e in events if e.get("model")}
    providers = {e.get("mcp_server") for e in events if e.get("mcp_server")}
    problems = []
    if expect_ide not in ides:
        problems.append(f"ide {expect_ide!r} missing (got {sorted(ides)})")
    if "llm_chat" not in tools:
        problems.append(f"tool llm_chat missing (got {sorted(tools)})")
    if expect_conv not in convs:
        problems.append(f"conversation_id {expect_conv!r} missing (got {sorted(convs)})")
    if not any(expect_model in (m or "") for m in models):
        if not models:
            problems.append("model empty")
        else:
            problems.append(f"model {expect_model!r} missing (got {sorted(models)})")
    if expect_provider not in providers:
        problems.append(f"provider {expect_provider!r} missing (got {sorted(providers)})")
    prompts = [e.get("user_prompt") or "" for e in events]
    if not any("ping" in (p or "").lower() for p in prompts):
        problems.append(f"user_prompt missing ping (got {prompts[:2]!r})")
    try:
        http_code = int(code) if code is not None else 0
    except ValueError:
        http_code = 0
    if http_code >= 400 and any(e.get("ok") is True for e in events):
        problems.append("expected ok=false for HTTP 4xx capture")
    problems.extend(check_apis(expect_conv))

    if problems:
        results.append({"ide": label, "ok": False, "http": code, "problems": problems})
        print(f"  ✗ mitm {label}: {'; '.join(problems)} (HTTP {code})", flush=True)
    else:
        results.append({"ide": label, "ok": True, "http": code, "provider": expect_provider, "model": sorted(models)})
        print(f"  ✓ mitm {label} provider={expect_provider} model={sorted(models)} (HTTP {code})", flush=True)

out = {"results": results, "passed": sum(1 for r in results if r.get("ok")), "total": len(results)}
Path(mitm_json).write_text(json.dumps(out, indent=2))
print(json.dumps(out))
if out["passed"] < out["total"]:
    raise SystemExit(2)
PY
  local mitm_rc=$?

  # Count from printed summary via re-read
  if [[ -f "$MITM_JSON" ]]; then
    local p t
    p=$(python3 -c 'import json,sys;d=json.load(open(sys.argv[1]));print(d["passed"])' "$MITM_JSON")
    t=$(python3 -c 'import json,sys;d=json.load(open(sys.argv[1]));print(d["total"])' "$MITM_JSON")
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
      "$@" >"${CLI_OUT_DIR}/${name}.out" 2>&1; then
    :
  else
    local rc=$?
    # timeout=124; CLI may still have emitted traffic
    if [[ "$rc" -eq 124 ]]; then
      note "  ! cli $name timed out after ${CLI_TIMEOUT}s — checking events anyway"
    else
      note "  ! cli $name exited $rc — checking events anyway"
      tail -20 "${CLI_OUT_DIR}/${name}.out" || true
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
    mkdir -p "$CA_DIR"
    export HOME="$CA_DIR"
    "$PROXY_BIN" setup --no-install >/dev/null 2>&1 || true
    CA_CERT="${HOME}/.agent-meter/ca-cert.pem"
    if [[ ! -f "$CA_CERT" ]]; then
      bad "cli: CA cert missing at $CA_CERT"
      return
    fi
    "$PROXY_BIN" start --listen "127.0.0.1:${PROXY_PORT}" --collector "$COLLECTOR_OTLP" >"$LOG_P" 2>&1 &
    PROXY_PID=$!
    sleep 0.8
  else
    CA_CERT="${HOME}/.agent-meter/ca-cert.pem"
  fi

  # Prefer PATH from common install locations on CI runners
  export PATH="${HOME}/.local/bin:${HOME}/.opencode/bin:/usr/local/bin:${PATH}"

  local ran=0
  local soft_required=0
  # If caller set REQUIRED=1 OR any provider secret exists, CLI failures are hard.
  if [[ "$REQUIRED" == "1" ]] || [[ -n "${ANTHROPIC_API_KEY:-}" ]] || [[ -n "${OPENAI_API_KEY:-}" ]]; then
    soft_required=1
  fi

  local WRAP=("$PROXY_BIN" wrap --listen "127.0.0.1:${PROXY_PORT}" --)

  if have_cmd claude && [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
    ran=1
    local old_req="$REQUIRED"
    [[ "$soft_required" == "1" ]] && REQUIRED=1
    run_one_cli claude-code claude-code \
      "${WRAP[@]}" claude -p "Reply with exactly: PONG" --bare --output-format text
    REQUIRED="$old_req"
  else
    skip "claude-code (need claude + ANTHROPIC_API_KEY)"
  fi

  if have_cmd codex && [[ -n "${OPENAI_API_KEY:-}" ]]; then
    ran=1
    local old_req="$REQUIRED"
    [[ "$soft_required" == "1" ]] && REQUIRED=1
    run_one_cli codex codex \
      "${WRAP[@]}" codex exec --skip-git-repo-check "Reply with exactly: PONG"
    REQUIRED="$old_req"
  else
    skip "codex (need codex + OPENAI_API_KEY)"
  fi

  if have_cmd opencode && { [[ -n "${OPENAI_API_KEY:-}" ]] || [[ -n "${ANTHROPIC_API_KEY:-}" ]]; }; then
    ran=1
    local old_req="$REQUIRED"
    [[ "$soft_required" == "1" ]] && REQUIRED=1
    run_one_cli opencode opencode \
      "${WRAP[@]}" opencode run "Reply with exactly: PONG"
    REQUIRED="$old_req"
  else
    skip "opencode (need opencode + API key)"
  fi

  if have_cmd gh && gh copilot --help >/dev/null 2>&1 && [[ -n "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ]]; then
    ran=1
    local old_req="$REQUIRED"
    # Copilot CLI often needs interactive auth — keep soft unless CAPTURE_LIVE_REQUIRE_COPILOT_CLI=1
    if [[ "${CAPTURE_LIVE_REQUIRE_COPILOT_CLI:-0}" == "1" ]]; then
      REQUIRED=1
    fi
    run_one_cli copilot-cli copilot-cli \
      "${WRAP[@]}" gh copilot -- -p "Reply with exactly: PONG"
    REQUIRED="$old_req"
  else
    skip "copilot-cli (need gh copilot + GH_TOKEN)"
  fi

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

SUMMARY_JSON="${CAPTURE_LIVE_SUMMARY_OUT:-$WORKDIR/summary.json}"
python3 - "$SUMMARY_JSON" "$PASS" "$FAIL" "$SKIP" "$REQUIRED" "$MODE" <<'PY'
import json, sys
path, p, f, s, req, mode = sys.argv[1:7]
data = {
    "mode": mode,
    "pass": int(p),
    "fail": int(f),
    "skip": int(s),
    "required": req == "1",
}
open(path, "w").write(json.dumps(data, indent=2) + "\n")
print(json.dumps(data))
PY

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "### Capture live (\`$MODE\`)"
    echo "| pass | fail | skip | required |"
    echo "|-----:|-----:|-----:|:--------:|"
    echo "| $PASS | $FAIL | $SKIP | $REQUIRED |"
  } >>"$GITHUB_STEP_SUMMARY"
fi

# Keep a durable copy for CI artifact upload (WORKDIR is wiped on EXIT).
if [[ -n "${CAPTURE_LIVE_SUMMARY_OUT:-}" ]]; then
  cp -f "$SUMMARY_JSON" "${CAPTURE_LIVE_SUMMARY_OUT}.bak" 2>/dev/null || true
fi

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
if [[ "$MODE" == "cli" && "$PASS" -eq 0 ]]; then
  echo "[capture-live] CLI layer produced 0 passes (missing binaries/secrets) — soft skip"
  exit 0
fi
echo "✓ capture live e2e"
