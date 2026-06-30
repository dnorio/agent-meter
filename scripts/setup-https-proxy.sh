#!/usr/bin/env bash
# setup-https-proxy.sh — Instalação automática do agent-meter-proxy (Linux / macOS / WSL)
#
# Faz tudo em um comando:
#   - baixa o binário certo para sua arquitetura
#   - gera e instala o certificado CA no sistema
#   - sobe o proxy em :8898 (systemd user service no Linux/WSL)
#   - configura o Cursor para usar o proxy (sem poluir HTTP_PROXY global)
#
# Uso:
#   curl -fsSL http://localhost:8081/api/setup/bootstrap.sh | bash
#   ./setup-https-proxy.sh
#   ./setup-https-proxy.sh --ensure-only   # só garante proxy rodando (idempotente)
#
set -euo pipefail

SCRIPT_NAME="setup-https-proxy.sh"
BASE_URL="${AGENT_METER_BASE_URL:-http://localhost:8081}"
COLLECTOR_URL="${AGENT_METER_COLLECTOR_URL:-$BASE_URL}"
INSTALL_DIR="${AGENT_METER_DIR:-${HOME}/.local/bin}"
PROXY_PORT="${AGENT_METER_PROXY_PORT:-8898}"
ENSURE_ONLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ensure-only) ENSURE_ONLY=true; shift ;;
    --base-url)    BASE_URL="$2"; COLLECTOR_URL="${AGENT_METER_COLLECTOR_URL:-$BASE_URL}"; shift 2 ;;
    --collector)   COLLECTOR_URL="$2"; shift 2 ;;
    --help)
      sed -n '2,14p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *) echo "unknown: $1" >&2; exit 1 ;;
  esac
done

log()  { echo "==> $*"; }
ok()   { echo "    ✓ $*"; }
warn() { echo "    ⚠ $*"; }

detect_os() {
  case "$(uname -s)" in
    Linux*)  echo "linux" ;;
    Darwin*) echo "mac" ;;
    *)       echo "linux" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)  echo "x64" ;;
    aarch64|arm64) echo "arm64" ;;
    *)             echo "x64" ;;
  esac
}

OS="$(detect_os)"
ARCH="$(detect_arch)"
PROXY_BIN="${INSTALL_DIR}/agent-meter-proxy"
SYSTEMD_UNIT="${HOME}/.config/systemd/user/agent-meter-proxy.service"
CURSOR_SETTINGS="${HOME}/.config/Cursor/User/settings.json"

mkdir -p "$INSTALL_DIR" "${HOME}/.config/agent-meter"

# ── 1. Binário ────────────────────────────────────────────────────────────────
if [[ "$ENSURE_ONLY" != true ]] || [[ ! -x "$PROXY_BIN" ]]; then
  log "baixando agent-meter-proxy (${OS}/${ARCH})..."
  TMP="$(mktemp)"
  curl -fsSL "${BASE_URL}/api/setup/proxy?os=${OS}&format=${ARCH}" -o "$TMP"
  install -m 755 "$TMP" "$PROXY_BIN"
  rm -f "$TMP"
  ok "instalado em $PROXY_BIN"
fi

# ── 2. CA ─────────────────────────────────────────────────────────────────────
if [[ "$ENSURE_ONLY" != true ]]; then
  if ! "$PROXY_BIN" ca-info 2>/dev/null | grep -q '✓ exists'; then
    log "gerando e instalando certificado CA..."
    "$PROXY_BIN" setup
    ok "CA instalado"
  else
    ok "CA já existe"
  fi
fi

# ── 3. Serviço (Linux/WSL) ────────────────────────────────────────────────────
if [[ "$(uname -s)" == "Linux" ]] && command -v systemctl >/dev/null 2>&1; then
  mkdir -p "${HOME}/.config/systemd/user"
  cat > "$SYSTEMD_UNIT" <<EOF
[Unit]
Description=agent-meter HTTPS proxy (telemetry capture)
After=network-online.target

[Service]
Type=simple
ExecStart=${PROXY_BIN} start --listen 127.0.0.1:${PROXY_PORT} --collector ${COLLECTOR_URL}
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
EOF
  systemctl --user daemon-reload 2>/dev/null || true
  systemctl --user enable agent-meter-proxy.service 2>/dev/null || true
  systemctl --user restart agent-meter-proxy.service 2>/dev/null || {
    warn "systemd indisponível — iniciando proxy em background"
    nohup "$PROXY_BIN" start --listen "127.0.0.1:${PROXY_PORT}" --collector "$COLLECTOR_URL" \
      >/tmp/agent-meter-proxy.log 2>&1 &
  }
  ok "proxy em http://127.0.0.1:${PROXY_PORT}"
elif [[ "$(uname -s)" == "Darwin" ]]; then
  if ! "$PROXY_BIN" status 2>/dev/null | grep -q 'running'; then
    log "iniciando proxy em background..."
    "$PROXY_BIN" start --daemon --listen "127.0.0.1:${PROXY_PORT}" --collector "$COLLECTOR_URL" || true
  fi
  ok "proxy em http://127.0.0.1:${PROXY_PORT}"
else
  warn "inicie manualmente: $PROXY_BIN start --collector $COLLECTOR_URL"
fi

# ── 4. Cursor — proxy só no IDE (não HTTP_PROXY global) ─────────────────────
configure_cursor() {
  local ca_path
  ca_path="$("$PROXY_BIN" ca-info 2>/dev/null | awk -F': ' '/Certificate:/{print $2; exit}')"
  [[ -z "$ca_path" || ! -f "$ca_path" ]] && return 0

  mkdir -p "$(dirname "$CURSOR_SETTINGS")"
  if command -v python3 >/dev/null 2>&1; then
    python3 - "$CURSOR_SETTINGS" "$PROXY_PORT" "$ca_path" <<'PY'
import json, os, sys
path, port, ca = sys.argv[1:4]
data = {}
if os.path.isfile(path):
    with open(path) as f:
        try:
            data = json.load(f)
        except json.JSONDecodeError:
            data = {}
data["http.proxy"] = f"http://127.0.0.1:{port}"
data["http.proxyStrictSSL"] = False
with open(path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
    ok "Cursor settings.json → http.proxy=127.0.0.1:${PROXY_PORT}"
  fi

  # Env scoped para sessões Cursor (sem afetar docker/curl global)
  cat > "${HOME}/.config/agent-meter/cursor-proxy.env" <<EOF
# agent-meter — carregado apenas pelo Cursor (não source no .bashrc)
export NODE_EXTRA_CA_CERTS="${ca_path}"
export SSL_CERT_FILE="${ca_path}"
export REQUESTS_CA_BUNDLE="${ca_path}"
EOF
}

if [[ "$ENSURE_ONLY" != true ]]; then
  configure_cursor
fi

# ── 5. Remover HTTP_PROXY global legado (quebrava docker build) ───────────────
scrub_global_proxy() {
  local f
  for f in "${HOME}/.bashrc" "${HOME}/.zshrc" "${HOME}/.profile"; do
    [[ -f "$f" ]] || continue
    if grep -q '127.0.0.1:8898' "$f" 2>/dev/null; then
      sed -i.bak '/127\.0\.0\.1:8898/d' "$f"
      ok "removido HTTP_PROXY global legado de $f"
    fi
  done
}
scrub_global_proxy

# ── 6. Verificação ────────────────────────────────────────────────────────────
if curl -sf "http://127.0.0.1:${PROXY_PORT}/" >/dev/null 2>&1 || \
   "$PROXY_BIN" status 2>/dev/null | grep -q 'running'; then
  ok "proxy ativo"
else
  warn "proxy pode não estar escutando — rode: $PROXY_BIN status"
fi

if curl -sf "${BASE_URL}/health" >/dev/null 2>&1; then
  ok "collector ${BASE_URL} acessível"
else
  warn "collector ${BASE_URL} inacessível daqui (normal em rede isolada)"
fi

echo ""
echo "══════════════════════════════════════════════════"
echo "  agent-meter-proxy pronto"
echo "  Proxy:     http://127.0.0.1:${PROXY_PORT}"
echo "  Collector: ${COLLECTOR_URL}"
echo "  Reinicie o Cursor para aplicar o proxy."
echo "══════════════════════════════════════════════════"
