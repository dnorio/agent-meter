#!/usr/bin/env bash
# secret-scan-history.sh — gitleaks + blob scan over complete git history (AMOSS gate)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

GITLEAKS_VERSION="${GITLEAKS_VERSION:-8.30.1}"
TOOLS_DIR="${TOOLS_DIR:-$ROOT/scripts/ci/tools}"
GITLEAKS="$TOOLS_DIR/gitleaks"

log() { printf '[secret-scan-history] %s\n' "$*"; }

install_gitleaks() {
  mkdir -p "$TOOLS_DIR"
  local arch tar url
  case "$(uname -m)" in
    x86_64|amd64) arch=linux_x64 ;;
    aarch64|arm64) arch=linux_arm64 ;;
    *) log "unsupported arch $(uname -m)"; return 1 ;;
  esac
  tar="gitleaks_${GITLEAKS_VERSION}_${arch}.tar.gz"
  url="https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/${tar}"
  log "downloading gitleaks v${GITLEAKS_VERSION} (${arch})…"
  curl -fsSL "$url" | tar -xz -C "$TOOLS_DIR" gitleaks
  chmod +x "$GITLEAKS"
}

if [[ ! -x "$GITLEAKS" ]]; then
  install_gitleaks
fi

COMMIT_REFS=(--branches --tags)
COMMITS="$(git rev-list "${COMMIT_REFS[@]}" | wc -l | tr -d ' ')"
log "=== secret scan — $COMMITS commits ($(date -Iseconds)) ==="

CONFIG="${GITLEAKS_CONFIG:-$ROOT/.gitleaks.toml}"
GITLEAKS_ARGS=(detect --source "$ROOT" --verbose --redact --log-opts="--branches --tags")
[[ -f "$CONFIG" ]] && GITLEAKS_ARGS+=(--config "$CONFIG")

log "--- gitleaks (all commits) ---"
if ! "$GITLEAKS" "${GITLEAKS_ARGS[@]}"; then
  log "FAILED — gitleaks found leaks in history"
  exit 1
fi
log "✓ gitleaks: no leaks"

log "--- blob walker (all objects) ---"
bash "$ROOT/scripts/ci/secret-scan-blobs.sh"

log "--- git pickaxe (extra patterns) ---"
EXTRA=(
  'eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}'  # JWT-like
  'hooks\.slack\.com/services/'
  'discord(app)?\.com/api/webhooks/'
)
extra_fail=0
for pat in "${EXTRA[@]}"; do
  n=$(git log "${COMMIT_REFS[@]}" -G"$pat" --oneline 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$n" -gt 0 ]]; then
    log "❌ pickaxe /$pat/ → $n commit(s)"
    git log "${COMMIT_REFS[@]}" -G"$pat" --oneline 2>/dev/null | head -5 | sed 's/^/    /'
    extra_fail=1
  fi
done
[[ $extra_fail -eq 0 ]] || exit 1

log "✓ secret-scan-history PASSED ($COMMITS commits)"
exit 0
