# Capture e2e — CI contracts vs live IDEs

## Does it make sense?

Yes. Capture must not break when Copilot / Cursor / Antigravity / Eclipse /
Claude / Codex change OTLP shapes.

## What Jenkins / GHA do (every PR)

| Check | Script |
|-------|--------|
| Fixture contracts (strict) | [`scripts/ci/capture-e2e.sh`](../scripts/ci/capture-e2e.sh) |
| Proxy-shaped OTLP + proxy/mcp unit | [`scripts/ci/capture-proxy-e2e.sh`](../scripts/ci/capture-proxy-e2e.sh) |
| In-process regression | `cargo test --test otlp_regression` |
| Nightly schedule | [`.github/workflows/capture-nightly.yml`](../.github/workflows/capture-nightly.yml) |

Contracts live in [`fixtures/manifest.json`](../crates/collector/tests/fixtures/manifest.json)
(`tool_name`, `ide`, `conversation_id`, `model`, orphan-fixture guard).

## What CI does NOT do

Spin real VS Code / Cursor / Eclipse / Antigravity GUIs — no display, licenses,
Electron flakiness, wrong for constrained agents. Live refresh is local/WSL.

## Refresh a fixture after an IDE update

```bash
# 1) dump raw OTLP JSON from a live capture
# 2) suggest manifest entry + sanity-check shape:
bash scripts/capture-record.sh /tmp/otlp-dump.json --ua 'cursor/0.48.0'

# 3) copy into crates/collector/tests/fixtures/, edit expect_ide, then:
bash scripts/ci/capture-e2e.sh
bash scripts/ci/capture-proxy-e2e.sh
cargo test -p agent-meter-collector --test otlp_regression
```

Optional live POST (collector already running):

```bash
CAPTURE_RECORD_BASE=http://127.0.0.1:8081 \
CAPTURE_RECORD_OTLP=http://127.0.0.1:4318/v1/traces \
  bash scripts/capture-record.sh /tmp/otlp-dump.json --ua 'vscode/1.100'
```

## Harnesses covered

VS Code Copilot · Eclipse Copilot · Cursor · Antigravity · Claude Code ·
Codex CLI · MCP OTel semconv · proxy-shaped Cursor/Claude/Codex payloads
