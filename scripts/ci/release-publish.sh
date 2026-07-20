#!/usr/bin/env bash
# agent-meter-release-publish.sh — gh release upload
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
elif [[ -d "$SCRIPT_DIR/../../dist" ]]; then
  DIST="$(cd "$SCRIPT_DIR/../../dist" && pwd)"
else
  log "ERROR: dist/ not found"
  exit 2
fi

shopt -s nullglob
files=("$DIST"/*.tar.gz "$DIST"/*.zip)
shopt -u nullglob

required=(
  "agent-meter-linux-x86_64.tar.gz"
  "agent-meter-linux-arm64.tar.gz"
  "SHA256SUMS"
)
missing=()
for artifact in "${required[@]}"; do
  [[ -f "$DIST/$artifact" ]] || missing+=("$artifact")
done
if [[ ${#missing[@]} -gt 0 ]]; then
  log "ERROR: missing required release artifacts in $DIST:"
  printf '  - %s\n' "${missing[@]}"
  exit 2
fi

[[ ${#files[@]} -gt 0 ]] || {
  log "ERROR: no .tar.gz/.zip in $DIST"
  exit 2
}

if [[ -f "$DIST/SHA256SUMS" ]]; then
  while read -r _sum name; do
    [[ -n "$name" && -f "$DIST/$name" ]] || {
      log "ERROR: SHA256SUMS references missing file: ${name:-<empty>}"
      exit 2
    }
  done < "$DIST/SHA256SUMS"
  files+=("$DIST/SHA256SUMS")
fi

bash "$SCRIPT_DIR/release-smoke.sh" "$DIST"

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
