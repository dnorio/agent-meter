#!/usr/bin/env bash
# agent-meter-release-build.sh — cross-build release artifacts
# Runs on a Linux x86_64 release runner. Builds Linux x86_64 + aarch64 (+ Windows if mingw/cross present).
# macOS: not cross-compiled — build locally or self-hosted Mac runner.
set -euo pipefail

log() { printf '[release-build] %s\n' "$*"; }

# Resolve app root: APP_DIR override or repo root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -n "${APP_DIR:-}" && -f "${APP_DIR}/Cargo.toml" ]]; then
  :
elif [[ -f "$SCRIPT_DIR/../../Cargo.toml" ]]; then
  APP_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
else
  log "ERROR: cannot locate agent-meter Cargo.toml (APP_DIR=${APP_DIR:-unset})"
  exit 2
fi

cd "$APP_DIR"
DIST="${DIST_DIR:-$APP_DIR/dist}"
mkdir -p "$DIST"
rm -rf "$DIST"/*

TAG="${TAG_NAME:-${GIT_TAG:-${RELEASE_TAG:-}}}"
VERSION="${RELEASE_VERSION:-}"
if [[ -z "$VERSION" && -n "$TAG" ]]; then
  VERSION="${TAG#agent-meter-v}"
  VERSION="${VERSION#v}"
fi
[[ -n "$VERSION" ]] || VERSION="$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\([^"]*\)".*/\1/')"
log "version=$VERSION app=$APP_DIR dist=$DIST"

export CARGO_TERM_COLOR=always
export CARGO_INCREMENTAL=0

USE_CROSS="${USE_CROSS:-auto}"
have_cross=false
if command -v cross >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  have_cross=true
fi

install_cross_toolchains() {
  if ! command -v apt-get >/dev/null 2>&1; then
    return 1
  fi
  if [[ "$(id -u)" -ne 0 ]] && ! command -v sudo >/dev/null 2>&1; then
    log "skip apt toolchains (no root/sudo)"
    return 1
  fi
  local apt_cmd=(apt-get)
  if [[ "$(id -u)" -ne 0 ]]; then
    apt_cmd=(sudo apt-get)
  fi
  export DEBIAN_FRONTEND=noninteractive
  "${apt_cmd[@]}" update -qq
  "${apt_cmd[@]}" install -y -qq --no-install-recommends \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross \
    ${INSTALL_MINGW:+mingw-w64} \
    ca-certificates \
    >/dev/null
}

should_use_cross() {
  local target="$1"
  case "$USE_CROSS" in
    1|true|yes) return 0 ;;
    0|false|no) return 1 ;;
  esac
  [[ "$have_cross" == true ]] || return 1
  case "$target" in
    x86_64-unknown-linux-gnu) return 1 ;;
    *) return 0 ;;
  esac
}

make_zip() {
  local dir="$1" asset="$2"
  if command -v zip >/dev/null 2>&1; then
    (cd "$dir" && zip -q "${asset}.zip" "$asset")
  elif command -v python3 >/dev/null 2>&1; then
    python3 - "$dir" "$asset" <<'PY'
import sys, zipfile
from pathlib import Path
work, name = Path(sys.argv[1]), sys.argv[2]
with zipfile.ZipFile(work / f"{name}.zip", "w", zipfile.ZIP_DEFLATED) as zf:
    zf.write(work / name, arcname=name)
PY
  else
    log "ERROR: need zip or python3 to create Windows archive"
    exit 2
  fi
}

build_one() {
  local target="$1" asset="$2" archive="$3"
  log "building $target → $asset"
  rustup target add "$target" >/dev/null 2>&1 || true

  if should_use_cross "$target"; then
    cross build --release -p agent-meter-collector --target "$target"
  else
    case "$target" in
      aarch64-unknown-linux-gnu)
        if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
          log "ERROR: aarch64-linux-gnu-gcc missing (install toolchains or set USE_CROSS=1 with docker)"
          exit 2
        fi
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
        ;;
      x86_64-pc-windows-gnu)
        if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
          log "ERROR: x86_64-w64-mingw32-gcc missing (INSTALL_MINGW=1 or USE_CROSS=1)"
          exit 2
        fi
        export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
        ;;
    esac
    cargo build --release -p agent-meter-collector --target "$target"
  fi

  local bin_path="target/${target}/release/agent-meter-collector"
  [[ -f "${bin_path}.exe" ]] && bin_path="${bin_path}.exe"

  cp "$bin_path" "$DIST/$asset"
  chmod +x "$DIST/$asset" 2>/dev/null || true

  if [[ "$archive" == "tar.gz" ]]; then
    tar -czf "$DIST/${asset}.tar.gz" -C "$DIST" "$asset"
  elif [[ "$archive" == "zip" ]]; then
    make_zip "$DIST" "$asset"
  fi
  log "✓ $asset"
}

if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  install_cross_toolchains || true
fi

build_one x86_64-unknown-linux-gnu agent-meter-linux-x86_64 tar.gz
build_one aarch64-unknown-linux-gnu agent-meter-linux-arm64 tar.gz

if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || should_use_cross x86_64-pc-windows-gnu; then
  build_one x86_64-pc-windows-gnu agent-meter-windows-x86_64.exe zip || log "WARN: windows cross-build failed (non-fatal)"
else
  log "skip windows (mingw-w64 not installed; set INSTALL_MINGW=1 or USE_CROSS=1)"
fi

shopt -s nullglob
checksum_inputs=("$DIST"/*.tar.gz "$DIST"/*.zip)
shopt -u nullglob
if [[ ${#checksum_inputs[@]} -gt 0 ]]; then
  log "generating SHA256SUMS"
  (
    cd "$DIST"
    sha256sum "${checksum_inputs[@]##*/}" > SHA256SUMS
  )
  log "✓ SHA256SUMS"
else
  log "WARN: no release archives found; skipping SHA256SUMS"
fi

log "artifacts:"
ls -la "$DIST"
