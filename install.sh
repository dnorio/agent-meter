#!/bin/sh
# agent-meter-proxy installer
# Works on: bash, zsh, sh, Git Bash, WSL
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install.sh | sh
#   wget -qO- https://raw.githubusercontent.com/dnorio/agent-meter/main/install.sh | sh
#
# Environment variables:
#   AGENT_METER_VERSION  — specific version (default: latest)
#   AGENT_METER_DIR      — install directory (default: ~/.local/bin)

set -e

REPO="dnorio/agent-meter"
BINARY="agent-meter-proxy"
INSTALL_DIR="${AGENT_METER_DIR:-$HOME/.local/bin}"

# --- Detect platform ---

detect_os() {
  case "$(uname -s)" in
    Linux*)   echo "linux";;
    Darwin*)  echo "darwin";;
    CYGWIN*|MINGW*|MSYS*) echo "windows";;
    *)        echo "unknown";;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)  echo "x86_64";;
    aarch64|arm64) echo "aarch64";;
    *)             echo "unknown";;
  esac
}

OS=$(detect_os)
ARCH=$(detect_arch)

if [ "$OS" = "unknown" ] || [ "$ARCH" = "unknown" ]; then
  echo "Error: Unsupported platform $(uname -s) / $(uname -m)" >&2
  exit 1
fi

# --- Determine version ---

if [ -z "$AGENT_METER_VERSION" ]; then
  AGENT_METER_VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "\(.*\)".*/\1/')
  if [ -z "$AGENT_METER_VERSION" ]; then
    echo "Error: Could not determine latest version" >&2
    exit 1
  fi
fi

echo "Installing ${BINARY} ${AGENT_METER_VERSION} for ${OS}/${ARCH}..."

# --- Download ---

EXT=""
if [ "$OS" = "windows" ]; then EXT=".exe"; fi

ASSET="${BINARY}-${OS}-${ARCH}${EXT}"
URL="https://github.com/${REPO}/releases/download/${AGENT_METER_VERSION}/${ASSET}"

mkdir -p "$INSTALL_DIR"
DEST="${INSTALL_DIR}/${BINARY}${EXT}"

echo "  Downloading ${URL}..."
curl -fsSL "$URL" -o "$DEST"
chmod +x "$DEST"

echo "  Installed to ${DEST}"

# --- Check PATH ---

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "  ⚠ ${INSTALL_DIR} is not in your PATH."
    echo "  Add this to your shell profile:"
    echo ""
    echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
    ;;
esac

echo ""
echo "✓ ${BINARY} ${AGENT_METER_VERSION} installed successfully!"
echo ""
echo "  Next steps:"
echo "    ${BINARY} setup          # Generate & install CA certificate"
echo "    ${BINARY} start          # Start the proxy"
echo "    ${BINARY} wrap cursor .  # Launch Cursor with telemetry capture"
