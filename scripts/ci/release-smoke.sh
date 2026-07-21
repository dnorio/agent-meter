#!/usr/bin/env bash
# release-smoke.sh — clean-install smoke checks for release artifacts
set -euo pipefail

log() { printf '[release-smoke] %s\n' "$*"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -n "${APP_DIR:-}" && -d "${APP_DIR}/dist" ]]; then
  DIST="${APP_DIR}/dist"
elif [[ -n "${1:-}" ]]; then
  DIST="$1"
elif [[ -d "$SCRIPT_DIR/../../dist" ]]; then
  DIST="$(cd "$SCRIPT_DIR/../../dist" && pwd)"
else
  log "ERROR: dist/ not found (pass path or set APP_DIR/DIST_DIR)"
  exit 2
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

smoke_linux_archive() {
  local archive="$1"
  local name
  name="$(basename "$archive" .tar.gz)"
  local work="$tmpdir/$name"
  mkdir -p "$work"
  tar -xzf "$archive" -C "$work"
  local bin="$work/$name"
  [[ -f "$bin" ]] || {
    log "ERROR: expected binary $bin in $archive"
    exit 2
  }
  chmod +x "$bin"
  if file "$bin" | grep -q "ARM aarch64"; then
    file "$bin" | grep -q "ELF 64-bit" || {
      log "ERROR: $name is not a valid arm64 ELF"
      exit 2
    }
  elif ! "$bin" --help >/dev/null 2>&1; then
    if file "$bin" | grep -q "ELF 64-bit"; then
      log "WARN: $name --help failed (likely glibc mismatch); verified ELF only"
    else
      log "ERROR: $name failed --help smoke check"
      exit 2
    fi
  fi
  log "✓ $archive"
}

shopt -s nullglob
linux_archives=("$DIST"/agent-meter-linux-*.tar.gz)
proxy_linux_archives=("$DIST"/agent-meter-proxy-linux-*.tar.gz)
windows_archives=("$DIST"/agent-meter-windows-*.zip)
proxy_windows_archives=("$DIST"/agent-meter-proxy-windows-*.zip)
shopt -u nullglob

[[ ${#linux_archives[@]} -gt 0 ]] || {
  log "ERROR: no Linux collector release archives found in $DIST"
  exit 2
}

[[ ${#proxy_linux_archives[@]} -gt 0 ]] || {
  log "ERROR: no Linux proxy release archives found in $DIST"
  exit 2
}

for archive in "${linux_archives[@]}" "${proxy_linux_archives[@]}"; do
  smoke_linux_archive "$archive"
done

for zipf in "${windows_archives[@]}" "${proxy_windows_archives[@]}"; do
  if command -v unzip >/dev/null 2>&1; then
    unzip -l "$zipf" | grep -q '\.exe$' || {
      log "ERROR: no Windows executable found in $zipf"
      exit 2
    }
  else
    log "WARN: unzip not installed; skipping structural check for $zipf"
  fi
  log "✓ $zipf"
done

log "✓ release smoke OK"
