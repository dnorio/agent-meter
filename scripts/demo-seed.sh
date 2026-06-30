#!/usr/bin/env bash
# Populate a running agent-meter collector with synthetic demo data so the
# dashboard, conversations and reports have something to show for a showcase.
#
# Usage:
#   ./scripts/demo-seed.sh                 # targets http://127.0.0.1:8081
#   BASE=http://127.0.0.1:8099 ./scripts/demo-seed.sh
set -euo pipefail

BASE="${BASE:-http://127.0.0.1:8081}"
EVENTS_PER_CONV="${EVENTS_PER_CONV:-8}"
CONVERSATIONS="${CONVERSATIONS:-6}"

agents=("cursor" "copilot" "claude-code" "codex-cli" "antigravity")
ides=("cursor" "vscode" "eclipse" "cli" "antigravity")
models=("gpt-4o" "claude-sonnet-4" "gemini-2.5-pro" "o3-mini" "gpt-4o-mini")
servers=("filesystem" "git" "chromeDevtools" "playwright" "fetch" "vscode-builtin")
tools=("read_file" "grep_search" "run_in_terminal" "edit_file" "list_dir" "semantic_search" "fetch_webpage" "create_file")

iso() { date -u -d "@$1" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -r "$1" +%Y-%m-%dT%H:%M:%SZ; }
gen_uuid() {
  if [ -r /proc/sys/kernel/random/uuid ]; then cat /proc/sys/kernel/random/uuid
  elif command -v uuidgen >/dev/null 2>&1; then uuidgen
  else printf '%08x-%04x-4%03x-8%03x-%04x%08x' $((RANDOM)) $((RANDOM & 0xFFF)) $((RANDOM & 0xFFF)) $((RANDOM & 0xFFF)) $((RANDOM)) $((RANDOM)); fi
}

now=$(date +%s)
total=0
for c in $(seq 1 "$CONVERSATIONS"); do
  ai=$(( (c - 1) % ${#agents[@]} ))
  agent="${agents[$ai]}"; ide="${ides[$ai]}"; model="${models[$(( (c-1) % ${#models[@]} ))]}"
  conv="conv-$(printf '%04x' $((RANDOM)))-$c"
  # spread conversations over the last few days
  base_ts=$(( now - c * 7200 - RANDOM % 86400 ))
  for e in $(seq 1 "$EVENTS_PER_CONV"); do
    srv="${servers[$((RANDOM % ${#servers[@]}))]}"
    tool="${tools[$((RANDOM % ${#tools[@]}))]}"
    start=$(( base_ts + e * 45 ))
    dur=$(( 200 + RANDOM % 4000 ))
    end=$(( start + dur / 1000 + 1 ))
    rbytes=$(( 300 + RANDOM % 4000 ))
    pbytes=$(( 800 + RANDOM % 60000 ))
    ok=true; err=null
    if (( RANDOM % 17 == 0 )); then ok=false; err='"timeout contacting tool"'; fi
    uid=$(gen_uuid)
    payload=$(cat <<JSON
{"event_id":"$uid","tool_name":"$tool","mcp_server":"$srv","ide":"$ide","agent":"$agent","model":"$model","conversation_id":"$conv","user_prompt":"Demo prompt for $conv step $e","started_at":"$(iso "$start")","ended_at":"$(iso "$end")","ok":$ok,"error":$err,"request_bytes":$rbytes,"response_bytes":$pbytes}
JSON
)
    curl -s -o /dev/null -X POST "$BASE/events/tool-call" -H 'Content-Type: application/json' -d "$payload"
    total=$(( total + 1 ))
  done
  echo "  seeded conversation $conv ($EVENTS_PER_CONV events, $agent/$model)"
done

echo "✓ seeded $total events across $CONVERSATIONS conversations into $BASE"
echo "  open $BASE to explore the dashboard"
