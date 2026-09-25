# Capture e2e — CI contracts vs live IDEs

## Does it make sense?

Yes. Capture must not break when Copilot / Cursor / Antigravity / Eclipse /
Claude / Codex change OTLP shapes. That needs **continuous regression**, not
manual eyeballing.

## What Jenkins should NOT do

Spinning real VS Code, Cursor, Eclipse, or Antigravity GUIs in Jenkins is a
bad fit for this fleet:

- No display / GPU; Electron + Eclipse are heavy and flaky in containers
- Licensing / marketplace extensions / login flows
- OCI/Jenkins agents are resource-constrained (ARM, small RAM)
- Result: slow, non-deterministic, expensive — wrong layer for every PR

## Three layers (what we do instead)

| Layer | Where | What |
|-------|--------|------|
| **1. Fixture replay (every PR)** | GHA + Jenkins | POST recorded OTLP JSON → assert `tool_name` + `ide` via HTTP |
| **2. Rust regression** | `cargo test --test otlp_regression` | Same fixtures, in-process collector |
| **3. Live refresh (human / desktop)** | WSL / workstation | Capture fresh spans from real harness → commit fixture + bump manifest |

Layer 1 script: [`scripts/ci/capture-e2e.sh`](../scripts/ci/capture-e2e.sh)  
Contracts: [`crates/collector/tests/fixtures/manifest.json`](../crates/collector/tests/fixtures/manifest.json)

## How to refresh a fixture after an IDE update

1. Run collector locally (`agent-meter serve`).
2. Point the IDE/CLI OTLP endpoint at `http://127.0.0.1:4318` (see [capture-setup.md](capture-setup.md)).
3. Perform one tool call + one chat (or the path that broke).
4. Dump the OTLP body (mitmproxy, browser DevTools, or a temporary dump middleware) into  
   `crates/collector/tests/fixtures/<harness>_….json`.
5. Update `manifest.json` expectations (`expect_tool_names`, `expect_ide`, `user_agent`).
6. Run locally:

```bash
bash scripts/ci/capture-e2e.sh
cargo test -p agent-meter-collector --test otlp_regression
```

7. Open PR — Jenkins/GHA gate the contract.

## Harnesses covered today

- VS Code Copilot (tool + chat)
- Eclipse Copilot
- Cursor
- Antigravity
- Claude Code
- Codex CLI
- MCP OTel semconv (`tools/call`)

## Future (optional, not blocking)

- Nightly job on a **desktop/WSL agent** that drives code-server or CLI wraps and
  fails if fixture drift exceeds a threshold — still not on the OCI Jenkins
  PR path.
- Proxy path e2e (`agent-meter-proxy wrap …`) with synthetic HTTPS peers.
