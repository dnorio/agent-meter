#!/usr/bin/env bash
# agent-meter full stack installer (collector + HTTPS proxy)
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install-full.sh | bash
#
# Same env vars as install.sh / install-proxy.sh (AGENT_METER_DIR, AGENT_METER_VERSION, …).

set -euo pipefail

REPO="dnorio/agent-meter"
BASE="https://raw.githubusercontent.com/${REPO}/main"

curl -fsSL "${BASE}/install.sh" | bash
curl -fsSL "${BASE}/install-proxy.sh" | bash

cat <<EOF

✓ Full stack installed (agent-meter + agent-meter-proxy)

  Demo data:
    agent-meter demo

  Serve collector:
    agent-meter serve

  Proxy + Cursor:
    agent-meter-proxy setup
    agent-meter-proxy start --collector http://127.0.0.1:8081
    agent-meter-proxy wrap cursor .

  Docs: https://github.com/${REPO}/blob/main/docs/capture-setup.md
EOF
