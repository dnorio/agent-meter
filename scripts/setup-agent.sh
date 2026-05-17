#!/usr/bin/env bash
# ===============================================================
# setup-agent.sh — Universal agent-meter integration for all agents
#
# Usage:
#   ./setup-agent.sh --agent opencode
#   ./setup-agent.sh --agent cursor --mcp-wrapper
#   ./setup-agent.sh --agent antigravity --mcp-wrapper
#   ./setup-agent.sh --agent codex
#   ./setup-agent.sh --agent copilot --mcp-wrapper
#
# Options:
#   --agent NAME        Agent ID (opencode|cursor|copilot|antigravity|codex)
#   --collector URL     Collector base URL (default: http://agent-meter:3000)
#   --mcp-wrapper       Also build & configure MCP wrapper binary
#   --install-prefix    Binary install path (default: ~/.local/bin)
#   --help              Show this help
# ===============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$APP_DIR/../.." && pwd)"

# ---- defaults ----
AGENT=""
COLLECTOR_URL="http://agent-meter:3000"
INSTALL_PREFIX="${HOME}/.local/bin"
MCP_WRAPPER=false

# ---- parse args ----
while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent)        AGENT="$2"; shift 2 ;;
    --collector)    COLLECTOR_URL="$2"; shift 2 ;;
    --mcp-wrapper)  MCP_WRAPPER=true; shift ;;
    --install-prefix) INSTALL_PREFIX="$2"; shift 2 ;;
    --help)         head -30 "$0" | grep '^#' | sed 's/^# \?//'; exit 0 ;;
    *)              echo "unknown: $1"; exit 1 ;;
  esac
done

if [[ -z "$AGENT" ]]; then
  echo "ERROR: --agent is required. One of: opencode, cursor, copilot, antigravity, codex"
  exit 1
fi

# ---- agent config ----
declare -A AGENT_IDE AGENT_NAME
AGENT_IDE=( [opencode]=opencode [cursor]=cursor [copilot]=copilot-vscode [antigravity]=antigravity [codex]=rust-rover )
AGENT_NAME=( [opencode]=opencode [cursor]=cursor [copilot]=copilot [antigravity]=antigravity [codex]=codex )

IDE="${AGENT_IDE[$AGENT]}"
AGENT_LABEL="${AGENT_NAME[$AGENT]}"

if [[ -z "$IDE" ]]; then
  echo "ERROR: unknown agent '$AGENT'. Valid: opencode, cursor, copilot, antigravity, codex"
  exit 1
fi

echo "==> agent-meter setup for: $AGENT (ide=$IDE, agent=$AGENT_LABEL)"
echo "    collector: $COLLECTOR_URL"
echo "    prefix:    $INSTALL_PREFIX"

# ---- 1. build CLI binary ----
echo ""
echo "==> [1/4] building agent-meter CLI..."
cd "$REPO_ROOT"

if command -v cargo &>/dev/null; then
  cargo build --release -p agent-meter-cli 2>&1 | tail -5
  CLI_SRC="$REPO_ROOT/target/release/agent-meter"
elif command -v docker &>/dev/null; then
  mkdir -p /tmp/agent-meter-build
  docker run --rm -v "$REPO_ROOT:/app" -w /app rust:1.88-slim-bookworm \
    cargo build --release -p agent-meter-cli 2>&1 | tail -5
  CLI_SRC="$REPO_ROOT/target/release/agent-meter"
else
  echo "WARN: neither cargo nor docker available — skipping build. Install agent-meter binary manually."
  CLI_SRC=""
fi

if [[ -n "$CLI_SRC" && -f "$CLI_SRC" ]]; then
  mkdir -p "$INSTALL_PREFIX"
  cp "$CLI_SRC" "$INSTALL_PREFIX/agent-meter"
  chmod +x "$INSTALL_PREFIX/agent-meter"
  echo "    installed: $INSTALL_PREFIX/agent-meter"
else
  echo "    SKIP — binary not found"
fi

# ---- 2. create env config snippet ----
echo ""
echo "==> [2/4] writing env config..."

CONFIG_DIR="${HOME}/.config/agent-meter"
mkdir -p "$CONFIG_DIR"

cat > "$CONFIG_DIR/env.sh" <<ENVEOF
# agent-meter — ${AGENT}
export AGENT_METER_COLLECTOR_URL="${COLLECTOR_URL}"
export AGENT_METER_IDE="${IDE}"
export AGENT_METER_AGENT="${AGENT_LABEL}"
export AGENT_METER_REPO="my-app"
ENVEOF

echo "    wrote: $CONFIG_DIR/env.sh"

# ---- 3. source hint in bashrc ----
echo ""
echo "==> [3/4] adding to ~/.bashrc..."

BASHRC_SNIPPET="# agent-meter (${AGENT})
if [ -f \"\${HOME}/.config/agent-meter/env.sh\" ]; then
  source \"\${HOME}/.config/agent-meter/env.sh\"
fi"

if ! grep -q "agent-meter" "$HOME/.bashrc" 2>/dev/null; then
  echo "" >> "$HOME/.bashrc"
  echo "$BASHRC_SNIPPET" >> "$HOME/.bashrc"
  echo "    added source block to ~/.bashrc"
else
  echo "    ~/.bashrc already sources agent-meter — skipped"
fi

# ---- 4. build MCP wrapper (optional) ----
if [[ "$MCP_WRAPPER" = true ]]; then
  echo ""
  echo "==> [4/4] building MCP wrapper..."

  cd "$REPO_ROOT"
  if command -v cargo &>/dev/null; then
    cargo build --release -p agent-meter-mcp-wrapper 2>&1 | tail -5
    WRP_SRC="$REPO_ROOT/target/release/agent-meter-mcp-wrapper"
  else
    docker run --rm -v "$REPO_ROOT:/app" -w /app rust:1.88-slim-bookworm \
      cargo build --release -p agent-meter-mcp-wrapper 2>&1 | tail -5
    WRP_SRC="$REPO_ROOT/target/release/agent-meter-mcp-wrapper"
  fi

  if [[ -f "$WRP_SRC" ]]; then
    cp "$WRP_SRC" "$INSTALL_PREFIX/agent-meter-mcp-wrapper"
    chmod +x "$INSTALL_PREFIX/agent-meter-mcp-wrapper"
    echo "    installed: $INSTALL_PREFIX/agent-meter-mcp-wrapper"
  fi

  # Config snippet
  cat >> "$CONFIG_DIR/env.sh" <<MCPEOF

# MCP wrapper
export MCP_WRAPPER_LISTEN=":3001"
export MCP_UPSTREAM_URL="http://localhost:3002"
# export MCP_UPSTREAM_URL="http://mcp-server:3000"   # in-cluster
MCPEOF
  echo "    appended MCP wrapper vars to $CONFIG_DIR/env.sh"
else
  echo ""
  echo "==> [4/4] SKIP (MCP wrapper not requested)"
fi

# ---- 5. smoke test ----
echo ""
echo "==> verifying installation..."

if command -v agent-meter &>/dev/null; then
  echo "    agent-meter CLI: OK ($(agent-meter --help 2>&1 | head -1))"
else
  echo "    agent-meter CLI: not in PATH (remember to add $INSTALL_PREFIX to PATH)"
fi

if curl -sf "${COLLECTOR_URL}/health" &>/dev/null; then
  echo "    collector reachable: OK"
else
  echo "    collector reachable: FAIL (expected if cluster is not connected)"
fi

echo ""
echo "=============================================="
echo "  agent-meter setup complete for: ${AGENT}"
echo "=============================================="
echo ""
echo "  Next steps:"
echo "    source ${HOME}/.bashrc  (or open new shell)"
echo "    agent-meter event tool-call --tool-name test --ok"
echo ""
echo "  Env:"
echo "    AGENT_METER_COLLECTOR_URL=${COLLECTOR_URL}"
echo "    AGENT_METER_IDE=${IDE}"
echo "    AGENT_METER_AGENT=${AGENT_LABEL}"
echo ""
