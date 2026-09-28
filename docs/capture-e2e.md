# Capture e2e — contracts + LIVE path

## Layers (what is / isn't real)

| Layer | Real? | What it proves |
|-------|-------|----------------|
| Fixture replay (`capture-e2e.sh`) | Contract | Known OTLP shapes → `ide`/`tool`/`model`/`conversation_id` |
| Proxy-shaped (`capture-proxy-e2e.sh`) | Unit+shape | Synthetic spans matching proxy JSON schema |
| **LIVE MITM** (`capture-live-e2e.sh` mitm) | **Yes** | Real `agent-meter-proxy` MITM → real TLS to AI hosts → real OTLP → collector. Asserts `ide` + `llm_chat` + `conversation_id` + `model` + timestamps. Fake API key OK (401 still captures). |
| **LIVE CLI** (`capture-live-e2e.sh` cli) | **Yes** | Real CLIs via `agent-meter-proxy wrap` when binary + API key exist |
| GUI Electron (Cursor / Antigravity / VS Code) | No in CI | Covered by LIVE MITM with matching User-Agent |

## Required IDEs

`cursor` · `antigravity` · `codex` · `claude-code` · `opencode` ·
`copilot-vscode` · `copilot-cli`

Every fixture for a required IDE must set `expect_conversation_ids` + `expect_models_any`.

## CI wiring

| Gate | Where |
|------|-------|
| Fixtures + proxy-shaped + regression | Every PR (GHA + Jenkins) |
| LIVE MITM (`CAPTURE_LIVE_REQUIRED=1`) | Every PR + nightly + Jenkins |
| LIVE CLI (`proxy wrap`) | Nightly when secrets present (hard-fail those CLIs) |

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
bash scripts/ci/capture-e2e.sh
bash scripts/ci/capture-live-e2e.sh
```

## Proxy UA forwarding

`agent-meter-proxy` stores the client `User-Agent`, sets `service.name` from UA+host,
and forwards that UA on OTLP POST so collector `infer_ide` attributes correctly
(codex/opencode/copilot-cli sharing `api.openai.com`).

Intercept hosts also include Gemini/Google, OpenRouter, DeepSeek, Groq, Mistral, Fireworks.
