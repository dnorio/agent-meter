#!/usr/bin/env bash
# release-build-macos.sh — native macOS release artifacts (collector + proxy)
set -euo pipefail

log() { printf '[release-build-macos] %s\n' "$*"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$APP_DIR"

DIST="${DIST_DIR:-$APP_DIR/dist}"
mkdir -p "$DIST"

export CARGO_TERM_COLOR=always
export CARGO_INCREMENTAL=0

build_one() {
  local target="$1"
  local crate="$2"
  local bin_name="$3"
  local asset="$4"

  log "building $crate ($target) → $asset"
  rustup target add "$target" >/dev/null 2>&1 || true
  cargo build --release -p "$crate" --target "$target"

  local bin_path="target/${target}/release/${bin_name}"
  cp "$bin_path" "$DIST/$asset"
  chmod +x "$DIST/$asset"
  tar -czf "$DIST/${asset}.tar.gz" -C "$DIST" "$asset"
  log "✓ $asset"
}

# GitHub macos-latest is arm64; cross-build x86_64 for Intel Macs.
build_one aarch64-apple-darwin agent-meter-collector agent-meter-collector agent-meter-darwin-aarch64
build_one x86_64-apple-darwin agent-meter-collector agent-meter-collector agent-meter-darwin-x86_64
build_one aarch64-apple-darwin agent-meter-proxy agent-meter-proxy agent-meter-proxy-darwin-aarch64
build_one x86_64-apple-darwin agent-meter-proxy agent-meter-proxy agent-meter-proxy-darwin-x86_64

(
  cd "$DIST"
  shasum -a 256 *.tar.gz > SHA256SUMS
)

for archive in "$DIST"/agent-meter-darwin-*.tar.gz "$DIST"/agent-meter-proxy-darwin-*.tar.gz; do
  [[ -f "$archive" ]] || continue
  name="$(basename "$archive" .tar.gz)"
  work="$(mktemp -d)"
  tar -xzf "$archive" -C "$work"
  bin="$work/$name"
  if ! "$bin" --help >/dev/null 2>&1; then
    log "ERROR: $name failed --help smoke"
    exit 2
  fi
  log "✓ smoke $archive"
  rm -rf "$work"
done

log "artifacts:"
ls -la "$DIST"
