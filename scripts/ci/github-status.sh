#!/usr/bin/env bash
# github-status.sh — commit status for Jenkins PR/branch checks (OSS agent-meter)
set -euo pipefail

log() { printf '[github-status] %s\n' "$*"; }

STATE="${1:-}"
DESCRIPTION="${2:-agent-meter CI}"
TARGET_URL="${3:-${BUILD_URL:-}}"
CONTEXT="${GITHUB_STATUS_CONTEXT:-jenkins/agent-meter}"
REPO="${GITHUB_REPOSITORY:-dnorio/agent-meter}"
SHA="${CODEQL_SHA:-${GIT_COMMIT:-}}"
[[ ${#SHA} -eq 40 ]] || SHA="$(git rev-parse HEAD 2>/dev/null || true)"

[[ -n "${STATE}" ]] || {
  log "usage: github-status.sh <success|failure|pending|error> [description] [target_url]"
  exit 2
}

[[ -n "${GITHUB_TOKEN:-}" ]] || {
  log "GITHUB_TOKEN missing — skip status"
  exit 0
}

[[ ${#SHA} -eq 40 ]] || {
  log "SHA unavailable — skip status"
  exit 0
}

desc_escaped=$(printf '%s' "$DESCRIPTION" | head -c 140 | sed 's/\\/\\\\/g; s/"/\\"/g')
payload="{\"state\":\"${STATE}\",\"context\":\"${CONTEXT}\",\"description\":\"${desc_escaped}\",\"target_url\":\"${TARGET_URL}\"}"

http_code=$(curl -s -o /tmp/gh-status-am.json -w '%{http_code}' \
  -X POST \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${REPO}/statuses/${SHA}" \
  -d "$payload")

if [[ "$http_code" =~ ^20 ]]; then
  log "${STATE} → ${CONTEXT} (${SHA:0:8})"
elif [[ "$http_code" == "422" ]]; then
  log "skip status HTTP 422 (SHA ${SHA:0:8} not on GitHub yet)"
  exit 0
else
  log "failed HTTP ${http_code}: $(head -c 200 /tmp/gh-status-am.json 2>/dev/null)"
  exit 1
fi
