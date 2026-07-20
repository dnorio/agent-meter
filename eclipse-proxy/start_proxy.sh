#!/bin/bash
# Eclipse Copilot Proxy Interceptor - Setup & Launch
# Captures GitHub Copilot API traffic from Eclipse and sends telemetry to agent-meter
#
# Usage: ./start_proxy.sh [--setup] [--host 127.0.0.1] [--port 8899]
#
# --setup: First-time setup (generate CA, import to Windows, configure Eclipse)
# --host:  Proxy listen host (default: 127.0.0.1; use 0.0.0.0 only for WSL/Windows bridging)
# --port:  Proxy port (default: 8899)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-8899}"
MITMPROXY_CONFDIR="$HOME/.mitmproxy"
ECLIPSE_INI="/mnt/c/Users/dnorio/AppData/Local/eclipse/eclipse.ini"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*" >&2; }

setup_ca() {
    info "Generating mitmproxy CA certificate..."
    # mitmdump generates CA on first run
    timeout 3 mitmdump --set confdir="$MITMPROXY_CONFDIR" -p "$PROXY_PORT" || true

    if [[ ! -f "$MITMPROXY_CONFDIR/mitmproxy-ca-cert.pem" ]]; then
        error "CA cert not generated!"
        exit 1
    fi

    info "CA cert: $MITMPROXY_CONFDIR/mitmproxy-ca-cert.pem"

    # Copy to Windows
    cp "$MITMPROXY_CONFDIR/mitmproxy-ca-cert.pem" /mnt/c/Users/dnorio/mitmproxy-ca.crt
    info "CA copied to C:\\Users\\dnorio\\mitmproxy-ca.crt"

    # Import to Windows trusted root store
    info "Importing CA into Windows certificate store..."
    powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "
        Import-Certificate -FilePath 'C:\Users\dnorio\mitmproxy-ca.crt' -CertStoreLocation 'Cert:\CurrentUser\Root' -ErrorAction Stop
    " 2>&1 && info "CA imported to Windows CurrentUser\\Root" || warn "CA import failed (may need admin)"

    # Also set NODE_EXTRA_CA_CERTS for the copilot-language-server
    info "CA setup complete"
}

configure_eclipse_proxy() {
    info "Configuring Eclipse network proxy..."

    # Get WSL IP that Windows can reach
    local wsl_ip
    wsl_ip=$(hostname -I | awk '{print $1}')
    info "WSL IP: $wsl_ip (proxy will listen here)"

    # Eclipse proxy is configured via preferences, not eclipse.ini
    # We need to set it in the workspace preferences
    local prefs_dir="/mnt/c/Users/dnorio/eclipse-workspace/.metadata/.plugins/org.eclipse.core.net"
    mkdir -p "$prefs_dir"

    cat > "$prefs_dir/prefs.ini" << EOF
eclipse.preferences.version=1
org.eclipse.core.net/proxyData/HTTP/hasAuth=false
org.eclipse.core.net/proxyData/HTTP/host=$wsl_ip
org.eclipse.core.net/proxyData/HTTP/port=$PROXY_PORT
org.eclipse.core.net/proxyData/HTTPS/hasAuth=false
org.eclipse.core.net/proxyData/HTTPS/host=$wsl_ip
org.eclipse.core.net/proxyData/HTTPS/port=$PROXY_PORT
org.eclipse.core.net/systemProxiesEnabled=false
org.eclipse.core.net/nonProxiedHosts=localhost|127.0.0.1
org.eclipse.core.net/proxiesEnabled=true
EOF
    info "Eclipse proxy preferences written"

    # Also set environment variable for the copilot-language-server subprocess
    # The Undici HTTP client respects HTTPS_PROXY
    if ! grep -q "HTTPS_PROXY" "$ECLIPSE_INI" 2>/dev/null; then
        # Add before the first -XX: line
        sed -i "/^-XX:CompileCommand=quiet/i -Dhttps.proxyHost=$wsl_ip\n-Dhttps.proxyPort=$PROXY_PORT" "$ECLIPSE_INI"
        info "Added proxy JVM args to eclipse.ini"
    fi

    info "Eclipse proxy configured → $wsl_ip:$PROXY_PORT"
}

show_status() {
    echo ""
    echo "═══════════════════════════════════════════════════════"
    echo "  Eclipse Copilot Proxy Interceptor"
    echo "═══════════════════════════════════════════════════════"
    echo "  Port:        $PROXY_PORT"
    echo "  Listen:      $PROXY_HOST"
    echo "  WSL IP:      $(hostname -I | awk '{print $1}')"
    echo "  CA cert:     $MITMPROXY_CONFDIR/mitmproxy-ca-cert.pem"
    echo "  Addon:       $SCRIPT_DIR/copilot_interceptor.py"
    echo "  Agent-meter: http://localhost:8081/v1/traces"
    echo "═══════════════════════════════════════════════════════"
    echo ""
}

start_proxy() {
    show_status
    info "Starting mitmdump on $PROXY_HOST:$PROXY_PORT ..."
    info "Press Ctrl+C to stop"
    echo ""

    exec mitmdump \
        --listen-host "$PROXY_HOST" \
        --listen-port "$PROXY_PORT" \
        --set confdir="$MITMPROXY_CONFDIR" \
        --set ssl_insecure=true \
        -s "$SCRIPT_DIR/copilot_interceptor.py"
}

# Parse args
DO_SETUP=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --setup) DO_SETUP=true; shift ;;
        --host)  PROXY_HOST="$2"; shift 2 ;;
        --port)  PROXY_PORT="$2"; shift 2 ;;
        *)       error "Unknown arg: $1"; exit 1 ;;
    esac
done

if $DO_SETUP; then
    setup_ca
    configure_eclipse_proxy
    echo ""
    info "Setup complete! Restart Eclipse, then run: $0"
    exit 0
fi

start_proxy
