#!/usr/bin/env bash
# agent-meter-release-publish.sh — gh release upload (T-365 Fase 3)
set -euo pipefail

log() { printf '[release-publish] %s\n' "$*"; }

TAG="${1:-${TAG_NAME:-${RELEASE_TAG:-}}}"
REPO="${GITHUB_REPOSITORY:-dnorio/agent-meter}"
DRY_RUN="${DRY_RUN:-false}"

[[ -n "$TAG" ]] || {
  log "usage: release-publish.sh <tag>  (e.g. v0.1.0 or agent-meter-v0.1.0)"
  exit 2
}

# Resolve dist dir
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -n "${APP_DIR:-}" && -d "${APP_DIR}/dist" ]]; then
  DIST="${APP_DIR}/dist"
elif [[ -d "${WORKSPACE:-}/agent-meter-src/dist" ]]; then
  DIST="${WORKSPACE}/agent-meter-src/dist"
elif [[ -d "${WORKSPACE:-}/apps/agent-meter/dist" ]]; then
  DIST="${WORKSPACE}/apps/agent-meter/dist"
elif [[ -d "${WORKSPACE:-}/dist" ]]; then
  DIST="${WORKSPACE}/dist"
elif [[ -d "$SCRIPT_DIR/../../../apps/agent-meter/dist" ]]; then
  DIST="$(cd "$SCRIPT_DIR/../../../apps/agent-meter/dist" && pwd)"
else
  log "ERROR: dist/ not found"
  exit 2
fi

shopt -s nullglob
files=("$DIST"/*.tar.gz "$DIST"/*.zip)
shopt -u nullglob
[[ ${#files[@]} -gt 0 ]] || {
  log "ERROR: no .tar.gz/.zip in $DIST"
  exit 2
}

[[ -n "${GITHUB_TOKEN:-}" ]] || {
  log "ERROR: GITHUB_TOKEN required"
  exit 2
}

if ! command -v gh >/dev/null 2>&1; then
  log "installing gh CLI..."
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq curl ca-certificates >/dev/null
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | tee /usr/share/keyrings/githubcli-archive-keyring.gpg >/dev/null
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | tee /etc/apt/sources.list.d/github-cli.list >/dev/null
  apt-get update -qq
  apt-get install -y -qq gh >/dev/null
fi

if [[ "$DRY_RUN" == "true" ]]; then
  log "DRY_RUN — would publish $TAG to $REPO:"
  printf '  %s\n' "${files[@]}"
  exit 0
fi

export GH_TOKEN="$GITHUB_TOKEN"
if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  log "release $TAG exists — uploading assets"
  gh release upload "$TAG" "${files[@]}" --repo "$REPO" --clobber
else
  name="${TAG#agent-meter-v}"
  name="${name#v}"
  gh release create "$TAG" "${files[@]}" \
    --repo "$REPO" \
    --title "agent-meter v${name}" \
    --generate-notes
fi

log "✓ published ${#files[@]} asset(s) → https://github.com/${REPO}/releases/tag/${TAG}"
