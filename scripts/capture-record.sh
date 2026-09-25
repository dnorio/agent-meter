#!/usr/bin/env bash
# capture-record.sh — validate a live OTLP dump and print a suggested manifest entry.
#
# Usage:
#   1. Point IDE at http://127.0.0.1:4318 and trigger a tool/chat.
#   2. Save the raw ExportTraceServiceRequest JSON to a file.
#   3. bash scripts/capture-record.sh path/to/dump.json [--ua 'cursor/0.48']
#
# Optionally posts to a running collector (CAPTURE_RECORD_BASE / CAPTURE_RECORD_OTLP).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DUMP="${1:-}"
shift || true
UA="unknown-agent/1.0"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ua) UA="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$DUMP" && -f "$DUMP" ]] || {
  echo "usage: $0 <otlp-dump.json> [--ua 'cursor/0.48']" >&2
  exit 2
}

python3 - "$DUMP" "$UA" <<'PY'
import json, sys, re
from pathlib import Path

dump, ua = sys.argv[1:3]
data = json.loads(Path(dump).read_text())
spans = []
svc = None
for rs in data.get("resourceSpans", []):
    for a in rs.get("resource", {}).get("attributes", []):
        if a.get("key") == "service.name":
            svc = (a.get("value") or {}).get("stringValue")
    for ss in rs.get("scopeSpans", []):
        for sp in ss.get("spans", []):
            attrs = {}
            for a in sp.get("attributes", []):
                v = a.get("value") or {}
                attrs[a.get("key")] = (
                    v.get("stringValue")
                    or v.get("intValue")
                    or v.get("doubleValue")
                    or v.get("boolValue")
                )
            spans.append({"name": sp.get("name"), "attrs": attrs})

print(f"service.name = {svc!r}")
print(f"user_agent  = {ua!r}")
print(f"spans ({len(spans)}):")
for s in spans:
    print(f"  - {s['name']}")
    for k in (
        "gen_ai.tool.name",
        "gen_ai.response.model",
        "gen_ai.request.model",
        "gen_ai.conversation.id",
    ):
        if k in s["attrs"]:
            print(f"      {k}={s['attrs'][k]!r}")

tools = []
models = []
convs = []
for s in spans:
    name = s["name"] or ""
    if name.startswith("execute_tool") or name.startswith("tools/call"):
        t = s["attrs"].get("gen_ai.tool.name") or name.split(" ", 1)[-1]
        tools.append(t)
    elif name.startswith("chat"):
        tools.append("llm_chat")
    m = s["attrs"].get("gen_ai.response.model") or s["attrs"].get("gen_ai.request.model")
    if m:
        models.append(m)
    c = s["attrs"].get("gen_ai.conversation.id")
    if c:
        convs.append(c)

harness = (svc or "unknown").replace("_", "-")
fid = re.sub(r"[^a-z0-9-]+", "-", harness.lower()).strip("-") or "harness"
stem = f"{fid}_capture.json"
print("\n--- suggested manifest entry ---")
print(json.dumps({
    "id": fid,
    "file": stem,
    "harness": harness,
    "user_agent": ua,
    "expect_ide": None,
    "expect_tool_names": sorted(set(tools)),
    "expect_conversation_ids": sorted(set(convs)) or None,
    "expect_models_any": sorted(set(models)) or None,
    "min_events": max(1, len(tools)),
}, indent=2))
print(f"\nCopy dump → crates/collector/tests/fixtures/{stem}")
print("Set expect_ide from docs/capture-e2e.md / ide.rs rules, then:")
print("  bash scripts/ci/capture-e2e.sh")
PY

BASE="${CAPTURE_RECORD_BASE:-}"
OTLP="${CAPTURE_RECORD_OTLP:-}"
if [[ -n "$BASE" && -n "$OTLP" ]]; then
  echo "[capture-record] posting dump to $OTLP"
  curl -sf -X POST "$OTLP" \
    -H "Content-Type: application/json" \
    -H "User-Agent: $UA" \
    --data-binary @"$DUMP" >/tmp/capture-record-otlp.json
  echo "[capture-record] OTLP ok; events:"
  sleep 1
  curl -sf "$BASE/reports/events?limit=20" | python3 -m json.tool | head -80
fi
