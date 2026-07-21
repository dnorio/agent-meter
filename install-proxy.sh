#!/usr/bin/env bash
# agent-meter-proxy installer (Linux / macOS)
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install-proxy.sh | bash
#
# Installs `agent-meter-proxy` to ~/.local/bin (HTTPS capture for Cursor/CLIs).

set -euo pipefail

REPO="dnorio/agent-meter"
BINARY="agent-meter-proxy"
INSTALL_DIR="${AGENT_METER_DIR:-$HOME/.local/bin}"
SRC_DIR="${AGENT_METER_SRC:-$HOME/.cache/agent-meter/src}"

info()  { printf '  %s\n' "$*"; }
step()  { printf '\n==> %s\n' "$*"; }
err()   { printf 'Error: %s\n' "$*" >&2; }

detect_os() {
  case "$(uname -s)" in
    Linux*)  echo "linux" ;;
    Darwin*) echo "darwin" ;;
    *)       echo "unknown" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)  echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *)             echo "unknown" ;;
  esac
}

OS="$(detect_os)"
ARCH="$(detect_arch)"

if [ "$OS" = "unknown" ] || [ "$ARCH" = "unknown" ]; then
  err "unsupported platform $(uname -s) / $(uname -m). Build from source instead."
  exit 1
fi

mkdir -p "$INSTALL_DIR"
DEST="${INSTALL_DIR}/${BINARY}"

try_release() {
  [ "${AGENT_METER_FROM_SOURCE:-0}" = "1" ] && return 1
  command -v curl >/dev/null 2>&1 || return 1

  local version="${AGENT_METER_VERSION:-}"
  if [ -z "$version" ]; then
    version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
      | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
  fi
  [ -z "$version" ] && return 1

  local asset_base="${BINARY}-${OS}-${ARCH}"
  local tmpdir url archive

  if [ "$OS" = "linux" ] || [ "$OS" = "darwin" ]; then
    archive="${asset_base}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${version}/${archive}"
    step "Downloading prebuilt ${BINARY} ${version} (${OS}/${ARCH})..."
    info "$url"
    tmpdir="$(mktemp -d)"
    if curl -fSL "$url" -o "$tmpdir/archive.tar.gz" 2>/dev/null; then
      if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
        err "sha256sum or shasum required to verify release artifacts"
        rm -rf "$tmpdir"
        return 1
      fi
      sums_url="https://github.com/${REPO}/releases/download/${version}/SHA256SUMS"
      if ! curl -fSL "$sums_url" -o "$tmpdir/SHA256SUMS" 2>/dev/null; then
        err "SHA256SUMS unavailable for ${version}"
        rm -rf "$tmpdir"
        return 1
      fi
      expected="$(grep "  ${archive}$" "$tmpdir/SHA256SUMS" | awk '{print $1}' || true)"
      if [ -z "$expected" ]; then
        err "no SHA256 entry for ${archive}"
        rm -rf "$tmpdir"
        return 1
      fi
      if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$tmpdir/archive.tar.gz" | awk '{print $1}')"
      else
        actual="$(shasum -a 256 "$tmpdir/archive.tar.gz" | awk '{print $1}')"
      fi
      if [ "$expected" != "$actual" ]; then
        err "checksum mismatch for ${archive}"
        rm -rf "$tmpdir"
        return 1
      fi
      info "✓ SHA256 verified"
      tar -xzf "$tmpdir/archive.tar.gz" -C "$tmpdir"
      cp "$tmpdir/$asset_base" "$DEST"
      chmod +x "$DEST"
      rm -rf "$tmpdir"
      return 0
    fi
    rm -rf "$tmpdir" 2>/dev/null || true
    return 1
  fi

  return 1
}

build_from_source() {
  step "No prebuilt binary available — building from source."

  if ! command -v cargo >/dev/null 2>&1; then
    err "Rust (cargo) is required to build from source."
    exit 1
  fi
  if ! command -v git >/dev/null 2>&1; then
    err "git is required to build from source."
    exit 1
  fi

  if [ -d "${SRC_DIR}/.git" ]; then
    git -C "$SRC_DIR" pull --ff-only origin main
  else
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --depth 1 "https://github.com/${REPO}.git" "$SRC_DIR"
  fi

  info "Compiling agent-meter-proxy (release)..."
  ( cd "$SRC_DIR" && cargo build --release -p agent-meter-proxy )
  cp "${SRC_DIR}/target/release/agent-meter-proxy" "$DEST"
  chmod +x "$DEST"
}

if try_release; then
  info "Installed prebuilt binary."
else
  build_from_source
fi

info "Installed to ${DEST}"

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    printf '\n'
    info "⚠ ${INSTALL_DIR} is not in your PATH. Add:"
    info "    export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

cat <<EOF

✓ agent-meter-proxy installed!

  One-time CA setup:
    ${BINARY} setup

  Start proxy + launch Cursor:
    ${BINARY} start --collector http://127.0.0.1:8081
    ${BINARY} wrap cursor .

  Docs: https://github.com/${REPO}/blob/main/docs/capture-setup.md
EOF
