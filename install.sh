#!/usr/bin/env bash
# agent-meter installer (Linux / macOS)
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install.sh | bash
#
# It installs a single `agent-meter` binary to ~/.local/bin. If a prebuilt
# release binary exists for your platform it is downloaded; otherwise the
# binary is built from source (requires Rust + git).
#
# Environment:
#   AGENT_METER_DIR       install directory (default: ~/.local/bin)
#   AGENT_METER_VERSION   release tag to install (default: latest release)
#   AGENT_METER_FROM_SOURCE=1   skip release download and always build from source
#   AGENT_METER_SRC       source checkout dir (default: ~/.cache/agent-meter/src)

set -euo pipefail

REPO="dnorio/agent-meter"
BINARY="agent-meter"
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

# ── 1) Try a prebuilt release binary ────────────────────────────────────────
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

# ── 2) Fallback: build from source ──────────────────────────────────────────
build_from_source() {
  step "No prebuilt binary available — building from source."

  if ! command -v cargo >/dev/null 2>&1; then
    err "Rust (cargo) is required to build from source."
    info "Install it with:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    info "then re-run this installer."
    exit 1
  fi
  if ! command -v git >/dev/null 2>&1; then
    err "git is required to build from source."
    exit 1
  fi

  if [ -d "${SRC_DIR}/.git" ]; then
    info "Updating source in ${SRC_DIR}..."
    git -C "$SRC_DIR" pull --ff-only origin main
  else
    info "Cloning ${REPO} into ${SRC_DIR}..."
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --depth 1 "https://github.com/${REPO}.git" "$SRC_DIR"
  fi

  info "Compiling (release) — this may take a few minutes..."
  ( cd "$SRC_DIR" && cargo build --release -p agent-meter-collector )
  cp "${SRC_DIR}/target/release/agent-meter-collector" "$DEST"
  chmod +x "$DEST"
}

if try_release; then
  info "Installed prebuilt binary."
else
  build_from_source
fi

info "Installed to ${DEST}"

# ── PATH hint + next steps ──────────────────────────────────────────────────
case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    printf '\n'
    info "⚠ ${INSTALL_DIR} is not in your PATH. Add this to your shell profile:"
    info "    export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

cat <<EOF

✓ agent-meter installed!

  Try it instantly (synthetic data):
    ${BINARY} demo

  Run for real (ingest your own events):
    ${BINARY} serve
    # UI + REST  → http://127.0.0.1:8081
    # OTLP        → http://127.0.0.1:4318/v1/traces

  Docs: https://github.com/${REPO}
EOF
