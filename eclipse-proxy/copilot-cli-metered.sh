#!/bin/bash
# copilot-cli-metered.sh — Wrapper para gh copilot com telemetria via agent-meter
#
# Roteia todas as chamadas HTTP do Copilot CLI pelo mitmproxy interceptor
# que captura: model, tokens, prompt, response, tool_calls, duration.
#
# Requisitos:
#   - mitmproxy rodando em localhost:8899 com copilot_interceptor.py
#   - Certificado mitmproxy (~/.mitmproxy/mitmproxy-ca-cert.pem)
#
# Usage:
#   ./copilot-cli-metered.sh -p "como listar pods no k8s"
#   ./copilot-cli-metered.sh -i "explain kubectl get pods -A"
#
# Testado: gh 2.91.0 + Copilot CLI → gpt-5.4, interceptação 100%

set -euo pipefail

PROXY_HOST="${COPILOT_PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${COPILOT_PROXY_PORT:-8899}"
MITMPROXY_CA="${HOME}/.mitmproxy/mitmproxy-ca-cert.pem"

# Verificar que proxy está rodando
if ! ss -tlnp 2>/dev/null | grep -q ":${PROXY_PORT}" && ! netstat -tlnp 2>/dev/null | grep -q ":${PROXY_PORT}"; then
    echo "⚠️  Proxy não encontrado em ${PROXY_HOST}:${PROXY_PORT}"
    echo "   Iniciando: cd apps/agent-meter/eclipse-proxy && mitmdump -p 8899 -s copilot_interceptor.py"
    exit 1
fi

# Configurar proxy e CA para o gh copilot
export HTTPS_PROXY="http://${PROXY_HOST}:${PROXY_PORT}"
export HTTP_PROXY="http://${PROXY_HOST}:${PROXY_PORT}"

# Go e Node respeitam estas variáveis para certificados custom
if [[ -f "$MITMPROXY_CA" ]]; then
    export SSL_CERT_FILE="$MITMPROXY_CA"
    export NODE_EXTRA_CA_CERTS="$MITMPROXY_CA"
    export REQUESTS_CA_BUNDLE="$MITMPROXY_CA"
fi

# Executar gh copilot com todas as flags originais
exec gh copilot "$@"
