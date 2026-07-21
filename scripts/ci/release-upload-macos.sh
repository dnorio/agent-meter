#!/usr/bin/env bash
# release-upload-macos.sh — append macOS artifacts to an existing GitHub release
set -euo pipefail

log() { printf '[release-upload-macos] %s\n' "$*"; }

TAG="${1:-${RELEASE_TAG:-}}"
[[ -n "$TAG" ]] || {
  log "usage: release-upload-macos.sh v0.1.x"
  exit 2
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIST="${DIST_DIR:-$APP_DIR/dist}"
REPO="${GITHUB_REPOSITORY:-dnorio/agent-meter}"

shopt -s nullglob
mac_archives=("$DIST"/agent-meter-darwin-*.tar.gz "$DIST"/agent-meter-proxy-darwin-*.tar.gz)
shopt -u nullglob

[[ ${#mac_archives[@]} -gt 0 ]] || {
  log "ERROR: no macOS tarballs in $DIST"
  exit 2
}

[[ -n "${GITHUB_TOKEN:-}" ]] || {
  log "ERROR: GITHUB_TOKEN required"
  exit 2
}

export GH_TOKEN="$GITHUB_TOKEN"
merge_dir="$(mktemp -d)"
trap 'rm -rf "$merge_dir"' EXIT

gh release download "$TAG" --repo "$REPO" -p SHA256SUMS -D "$merge_dir"
cat "$DIST/SHA256SUMS" >> "$merge_dir/SHA256SUMS"

upload=( "${mac_archives[@]}" "$merge_dir/SHA256SUMS" )
gh release upload "$TAG" "${upload[@]}" --repo "$REPO" --clobber

log "✓ uploaded ${#mac_archives[@]} macOS archive(s) + updated SHA256SUMS"
