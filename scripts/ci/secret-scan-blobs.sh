#!/usr/bin/env bash
# secret-scan-blobs.sh — walk every git blob (full history) for credential patterns
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PATTERN='ghp_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,}|glpat-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|sk-ant-[A-Za-z0-9_-]{20,}|sk-proj-[A-Za-z0-9_-]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY-----'
SKIP_PATH_RE='\.(png|jpg|jpeg|gif|webp|ico|woff2?|ttf|eot)$|/docs/assets/|/target/|Cargo\.lock$'

log() { printf '[secret-scan-blobs] %s\n' "$*"; }

declare -A seen=()
blobs_scanned=0
hits=0

COMMIT_REFS=(--branches --tags)
log "walking objects from $(git rev-list "${COMMIT_REFS[@]}" | wc -l | tr -d ' ') commits…"

while read -r hash path; do
  [[ -z "${hash:-}" ]] && continue
  [[ -n "${seen[$hash]+x}" ]] && continue
  seen[$hash]=1

  type=$(git cat-file -t "$hash" 2>/dev/null || continue)
  [[ "$type" == blob ]] || continue

  [[ "${path:-}" =~ $SKIP_PATH_RE ]] && continue

  size=$(git cat-file -s "$hash" 2>/dev/null || echo 0)
  [[ "$size" -gt 524288 ]] && continue

  blobs_scanned=$((blobs_scanned + 1))
  if git cat-file blob "$hash" 2>/dev/null | grep -qE "$PATTERN"; then
    echo "❌ blob ${hash:0:12} path=${path:-?} size=$size"
    git cat-file blob "$hash" 2>/dev/null | grep -oE "$PATTERN" | head -3 | sed 's/\(.\{12\}\).*/\1…/'
    hits=$((hits + 1))
  fi
done < <(git rev-list "${COMMIT_REFS[@]}" --objects)

log "scanned $blobs_scanned unique text blobs (${#seen[@]} objects total)"

if [[ $hits -gt 0 ]]; then
  log "FAILED — $hits blob(s) with secret-like content"
  exit 1
fi

log "OK — no secret patterns in blob history"
exit 0
