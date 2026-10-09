#!/usr/bin/env bash
# capture-matrix-coverage.sh — static gate: required_ides covered in fixture/live/proxy layers.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MANIFEST="$ROOT/crates/collector/tests/fixtures/manifest.json"
LIVE="$ROOT/scripts/ci/capture-live-e2e.sh"
PROXY="$ROOT/scripts/ci/capture-proxy-e2e.sh"
REGRESSION="$ROOT/crates/collector/tests/otlp_regression.rs"
INTERCEPTOR="$ROOT/crates/proxy/src/interceptor.rs"

python3 - "$MANIFEST" "$LIVE" "$PROXY" "$REGRESSION" "$INTERCEPTOR" <<'PY'
import json, sys
from pathlib import Path

manifest_path, live_path, proxy_path, regression_path, interceptor_path = map(
    Path, sys.argv[1:6]
)
m = json.loads(manifest_path.read_text())
required = set(m.get("required_ides") or [])
if not required:
    raise SystemExit("manifest required_ides empty")

fixtures = m.get("fixtures") or []
covered = {fx.get("expect_ide") for fx in fixtures if fx.get("expect_ide")}
missing_fx = sorted(required - covered)
if missing_fx:
    raise SystemExit(f"fixtures missing required_ides: {missing_fx}")

live = live_path.read_text()
proxy = proxy_path.read_text()
regression = regression_path.read_text()
interceptor = interceptor_path.read_text()

missing_live = sorted(ide for ide in required if f'"{ide}"' not in live)
missing_proxy = sorted(ide for ide in required if f'"{ide}"' not in proxy)
# otlp_regression uses snake file names; map known exceptions
file_hint = {
    "copilot-vscode": "vscode_copilot",
    "copilot-cli": "copilot_cli",
    "copilot-jetbrains": "copilot_jetbrains",
    "copilot-eclipse": "eclipse_copilot",
    "claude-code": "claude_code",
    "codex": "codex_cli",
    "rust-rover": "rust_rover",
}
missing_reg = []
for ide in sorted(required):
    hint = file_hint.get(ide, ide.replace("-", "_"))
    if hint not in regression and ide.replace("-", "_") not in regression:
        missing_reg.append(ide)

host_needles = [
    "host-openrouter",
    "host-deepseek",
    "host-groq",
    "host-gemini",
    "host-mistral",
    "host-fireworks",
    "host-xai",
    "host-together",
    "host-perplexity",
]
missing_hosts = [h for h in host_needles if h not in live]

# Proxy must intercept the DNS names behind host_* cases (drift guard).
proxy_host_needles = [
    "openrouter.ai",
    "api.deepseek.com",
    "api.groq.com",
    "generativelanguage.googleapis.com",
    "api.mistral.ai",
    "api.fireworks.ai",
    "api.x.ai",
    "api.together.xyz",
    "api.perplexity.ai",
]
missing_proxy_hosts = [h for h in proxy_host_needles if f'"{h}"' not in interceptor]

problems = []
if missing_live:
    problems.append(f"live MITM missing ides: {missing_live}")
if missing_proxy:
    problems.append(f"proxy e2e missing ides: {missing_proxy}")
if missing_reg:
    problems.append(f"otlp_regression missing ides: {missing_reg}")
if missing_hosts:
    problems.append(f"live MITM missing provider hosts: {missing_hosts}")
if missing_proxy_hosts:
    problems.append(f"proxy AI_HOSTS missing: {missing_proxy_hosts}")

if problems:
    raise SystemExit("; ".join(problems))

print(
    f"[capture-matrix-coverage] OK — {len(required)} required_ides + "
    f"{len(host_needles)} provider hosts + {len(proxy_host_needles)} proxy AI_HOSTS aligned"
)
PY
