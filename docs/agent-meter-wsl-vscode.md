# agent-meter — WSL + VS Code

Este guia mostra como rodar o collector no WSL2 e configurar o VS Code Remote -
WSL para enviar telemetria do GitHub Copilot Chat via OTLP.

```text
VS Code Remote - WSL
  GitHub Copilot Chat
        |
        | OTLP/HTTP :4318
        v
agent-meter serve
        |
        v
SQLite + dashboard :8081
```

## Prerequisites

- WSL2 with Ubuntu ou outra distribuicao Linux.
- VS Code com Remote - WSL e GitHub Copilot Chat.
- Rust toolchain se construindo a partir do codigo.
- Portas `8081` e `4318` disponiveis no ambiente WSL.

## Start the Collector

From a checkout:

```bash
cargo run -p agent-meter-collector -- serve
```

Or with the installed binary:

```bash
agent-meter serve
```

The default endpoints are:

- Dashboard and REST API: `http://127.0.0.1:8081`
- OTLP receiver: `http://127.0.0.1:4318/v1/traces`

## Configure VS Code

Open VS Code through Remote - WSL so Copilot runs inside the same WSL network
namespace as the collector. Then add:

```jsonc
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.otlpEndpoint": "http://127.0.0.1:4318",
  "github.copilot.chat.otel.captureContent": true
}
```

Use `captureContent: true` for prompt-inclusive observability. Set it to
`false` when you only want metadata and token attributes from VS Code.

## Verify Capture

Use Copilot Chat once, then check:

```bash
curl -s http://127.0.0.1:8081/health
curl -s http://127.0.0.1:8081/api/conversations?limit=5 | jq
```

Open the dashboard at `http://127.0.0.1:8081`.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| No Copilot spans appear | VS Code is running on Windows, not Remote - WSL | Reopen the folder with Remote - WSL |
| `127.0.0.1:4318` is refused | Collector is not running or OTLP port changed | Start `agent-meter serve` or update `otel.otlpEndpoint` |
| Windows browser cannot open the dashboard | WSL port mirroring is unavailable | Open the URL from WSL, or bind intentionally with `AGENT_METER_HOST=0.0.0.0` |
| Prompt text is missing | `captureContent` is disabled | Set `github.copilot.chat.otel.captureContent` to `true` |

## Notes

- Keep the collector bound to `127.0.0.1` for local development.
- If exposing it to another machine, add your normal network and authentication
  controls in front of the collector.
- SQLite is the default storage engine. Set `DATABASE_URL` when using Postgres.
