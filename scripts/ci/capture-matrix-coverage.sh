#!/usr/bin/env bash
# capture-matrix-coverage.sh — static gate: required_ides covered in fixture/live/proxy layers.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MANIFEST="$ROOT/crates/collector/tests/fixtures/manifest.json"
LIVE="$ROOT/scripts/ci/capture-live-e2e.sh"
PROXY="$ROOT/scripts/ci/capture-proxy-e2e.sh"
REGRESSION="$ROOT/crates/collector/tests/otlp_regression.rs"

python3 - "$MANIFEST" "$LIVE" "$PROXY" "$REGRESSION" <<'PY'
import json, re, sys
from pathlib import Path

manifest_path, live_path, proxy_path, regression_path = map(Path, sys.argv[1:5])
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

missing_live = sorted(ide for ide in required if f'"{ide}"' not in live)
missing_proxy = sorted(ide for ide in required if f'"{ide}"' not in proxy)
# otlp_regression uses snake file names; map known exceptions
file_hint = {
    "copilot-vscode": "vscode_copilot",
    "copilot-cli": "copilot_cli",
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
]
missing_hosts = [h for h in host_needles if h not in live]

problems = []
if missing_live:
    problems.append(f"live MITM missing ides: {missing_live}")
if missing_proxy:
    problems.append(f"proxy e2e missing ides: {missing_proxy}")
if missing_reg:
    problems.append(f"otlp_regression missing ides: {missing_reg}")
if missing_hosts:
    problems.append(f"live MITM missing provider hosts: {missing_hosts}")

if problems:
    raise SystemExit("; ".join(problems))

print(f"[capture-matrix-coverage] OK — {len(required)} required_ides + {len(host_needles)} provider hosts aligned")
PY
