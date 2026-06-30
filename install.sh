#!/usr/bin/env bash
# agent-meter-proxy installer — full auto when piped (curl | sh)
#
# Usage:
#   curl -fsSL http://localhost:8081/api/setup/bootstrap.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter-worktree/main/apps/agent-meter/install.sh | sh
#
# Environment:
#   AGENT_METER_AUTO=0     — só baixa o binário (comportamento legado)
#   AGENT_METER_VERSION    — tag do release (default: latest)
#   AGENT_METER_DIR        — diretório de instalação (default: ~/.local/bin)
#   AGENT_METER_BASE_URL   — URL do collector (default: http://localhost:8081)

set -e

REPO="dnorio/agent-meter"
BINARY="agent-meter-proxy"
INSTALL_DIR="${AGENT_METER_DIR:-$HOME/.local/bin}"
BASE_URL="${AGENT_METER_BASE_URL:-http://localhost:8081}"
AUTO="${AGENT_METER_AUTO:-1}"

# Quando piped, default é instalação completa via bootstrap
if [ ! -t 0 ] && [ "$AUTO" = "1" ] && [ -z "${AGENT_METER_SKIP_BOOTSTRAP:-}" ]; then
  echo "==> Instalação automática (proxy + CA + serviço + Cursor)..."
  exec bash -c "curl -fsSL '${BASE_URL}/api/setup/bootstrap.sh' | bash"
fi

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

if [ -z "$AGENT_METER_VERSION" ]; then
  AGENT_METER_VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "\(.*\)".*/\1/')
  if [ -z "$AGENT_METER_VERSION" ]; then
    echo "Error: Could not determine latest version" >&2
    exit 1
  fi
fi

echo "Installing ${BINARY} ${AGENT_METER_VERSION} for ${OS}/${ARCH}..."

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

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "  ⚠ ${INSTALL_DIR} is not in your PATH."
    echo "  Add: export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

echo ""
echo "✓ ${BINARY} ${AGENT_METER_VERSION} installed!"
echo ""
if [ "$AUTO" = "1" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd || echo "$INSTALL_DIR")"
  if [ -f "${SCRIPT_DIR}/scripts/setup-https-proxy.sh" ]; then
  AGENT_METER_BASE_URL="$BASE_URL" AGENT_METER_COLLECTOR_URL="$BASE_URL" \
    bash "${SCRIPT_DIR}/scripts/setup-https-proxy.sh"
  else
    echo "  Run full setup: curl -fsSL ${BASE_URL}/api/setup/bootstrap.sh | bash"
  fi
else
  echo "  Next: ${BINARY} setup && ${BINARY} start"
fi
