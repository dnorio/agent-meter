#!/usr/bin/env bash
# node-tarball-smoke.sh — install packed Node SDK in clean temp project
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SDK="$ROOT/sdk/node"

cd "$SDK"
npm ci
npm run build
TARBALL="$(npm pack --silent)"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
npm init -y >/dev/null
npm install --no-save "$SDK/$TARBALL"

node --input-type=module <<'EOF'
import { AgentMeter } from "@agent-meter/sdk";
if (typeof AgentMeter !== "function") {
  throw new Error("AgentMeter export missing from packed tarball");
}
console.log("✓ node tarball smoke OK");
EOF
