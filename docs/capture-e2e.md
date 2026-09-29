# Capture e2e — contracts + LIVE path

## Layers (what is / isn't real)

| Layer | Real? | What it proves |
|-------|-------|----------------|
| Fixture replay (`capture-e2e.sh`) | Contract | Known OTLP shapes → `ide`/`tool`/`model`/`conversation_id` + **sha256** |
| Proxy-shaped (`capture-proxy-e2e.sh`) | Unit+shape | Synthetic spans matching proxy JSON schema |
| Capture-record smoke | Tooling | `capture-record.sh` parses a real fixture dump |
| **LIVE MITM** (`capture-live-e2e.sh` mitm) | **Yes** | Real `agent-meter-proxy` MITM → real TLS to AI hosts → real OTLP → collector. Asserts `ide` + `llm_chat` + `conversation_id` + `model` + timestamps. Fake API key OK (401 still captures). |
| **LIVE CLI** (`capture-live-e2e.sh` cli) | **Yes** | Real CLIs via `agent-meter-proxy wrap` when binary + API key exist |
| GUI Electron (Cursor / Antigravity / VS Code) | No in CI | Covered by LIVE MITM with matching User-Agent |

## Required IDEs

`cursor` · `antigravity` · `codex` · `claude-code` · `opencode` ·
`copilot-vscode` · `copilot-cli`

Every fixture for a required IDE must set `expect_conversation_ids` + `expect_models_any`.
Fixture bytes must match `sha256` in the manifest (catches silent edits).

## CI wiring

| Gate | Where |
|------|-------|
| Fixtures + proxy-shaped + capture-record smoke | Every PR (GHA + Jenkins) |
| LIVE MITM (`CAPTURE_LIVE_REQUIRED=1`) | Every PR + nightly + Jenkins |
| LIVE CLI (`proxy wrap`) | Nightly **only if** provider secrets exist |
| Coverage floor | Jenkins `COVERAGE_MIN_LINES=90` |

```bash
# local — MITM only (no API keys)
CAPTURE_LIVE_MODE=mitm CAPTURE_LIVE_REQUIRED=1 bash scripts/ci/capture-live-e2e.sh

# local — CLIs too (needs keys + binaries)
CAPTURE_LIVE_MODE=all CAPTURE_LIVE_REQUIRED=0 bash scripts/ci/capture-live-e2e.sh
```

## Refresh fixtures after IDE shape change

```bash
bash scripts/capture-record.sh /tmp/otlp-dump.json --ua 'cursor/0.48.0'
# copy into crates/collector/tests/fixtures/, edit expect_ide, then:
# update sha256 in manifest.json (or re-run a digest helper), then:
bash scripts/ci/capture-e2e.sh
bash scripts/ci/capture-live-e2e.sh
```

## Proxy UA forwarding

`agent-meter-proxy` stores the client `User-Agent`, sets `service.name` from UA+host,
embeds `user_agent` / `browser.user_agent` on the OTLP **resource**, and forwards the
HTTP UA on OTLP POST. Collector prefers resource UA over the proxy's default
`agent-meter-proxy/*` header so IDE attribution stays correct.

Intercept hosts also include Gemini/Google, OpenRouter, DeepSeek, Groq, Mistral, Fireworks.
