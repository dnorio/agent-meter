# agent-meter

**Observability & FinOps for AI-powered development — in a single self-hosted binary.**

Track every LLM call, tool invocation, conversation and token spent across all your
IDEs and AI agents. No accounts, no cloud, no database to provision: one binary, a
local SQLite file, and a dashboard on `localhost`.

<p align="center">
  <img src="docs/assets/screenshot-dashboard.png" alt="agent-meter dashboard" width="720">
</p>

---

## Quick start

### Try it in 10 seconds (with synthetic data)

```bash
agent-meter demo
# → seeds synthetic conversations and opens http://127.0.0.1:8081
```

`demo` is the fastest way to see what agent-meter looks like — it generates
realistic activity (multiple agents, models, tools, errors) so every page has
something to show. It never collects real data and won't re-seed if a database
already has data (use `--force` to reseed).

### Run for real

```bash
agent-meter serve
# → http://127.0.0.1:8081  (UI + REST API)
# → http://127.0.0.1:4318  (OTLP receiver)
```

State is stored in `agent-meter.db` (SQLite) in the working directory — zero
configuration required.

### Install

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/dnorio/agent-meter/main/install.sh | bash

# Windows (PowerShell)
irm https://raw.githubusercontent.com/dnorio/agent-meter/main/install.ps1 | iex
```

### Build from source

```bash
# Prerequisites: Rust 1.75+
git clone https://github.com/dnorio/agent-meter.git
cd agent-meter
cargo run -p agent-meter-collector -- demo   # or: serve
```

---

## Sending data

agent-meter accepts telemetry three ways:

| Method | Source | How |
|--------|--------|-----|
| **REST** | Any agent / script | `POST /events/tool-call` (JSON) |
| **OTLP** | VS Code (GitHub Copilot), any OTel exporter | Point the OTLP HTTP exporter at `:4318` |
| **HTTPS proxy** | Cursor, Eclipse, Claude Code, Codex CLI | mitmproxy addons in [`eclipse-proxy/`](eclipse-proxy/) |

### Example — REST ingest

```bash
curl -X POST http://127.0.0.1:8081/events/tool-call \
  -H "Content-Type: application/json" \
  -d '{
    "event_id": "11111111-1111-4111-8111-111111111111",
    "tool_name": "read_file",
    "mcp_server": "filesystem",
    "agent": "cursor",
    "model": "gpt-4o",
    "conversation_id": "demo-1",
    "started_at": "2026-01-15T10:00:00Z",
    "ended_at": "2026-01-15T10:00:01Z",
    "ok": true,
    "request_bytes": 1200,
    "response_bytes": 30000
  }'
```

### Example — VS Code (OTLP)

```json
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.otlpEndpoint": "http://127.0.0.1:4318"
}
```

---

## Pages

| Page | What it shows |
|------|---------------|
| **Dashboard** (`/`) | KPIs, activity over time, top tools/models |
| **Conversations** (`/conversations`) | Sessions grouped by `conversation_id`, with drill-down |
| **Timeline** (`/conversations/:id/timeline`) | Per-conversation waterfall of tool calls |
| **Reports** (`/reports`) | Top tools, MCP servers, usage breakdowns |
| **Cost** (`/cost`) | Token usage and cost summary |

---

## API reference

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/events/tool-call` | Ingest a tool-call event |
| `POST` | `/v1/traces` | OTLP trace ingest (port `:4318`) |
| `GET` | `/api/conversations` | List conversations (paginated) |
| `GET` | `/api/conversations/:id/timeline` | Conversation timeline (events + summary) |
| `GET` | `/reports/top-tools` | Most-used tools |
| `GET` | `/reports/top-mcp-servers` | Most active MCP servers |
| `GET` | `/api/cost/summary` | Token & cost summary |
| `GET` | `/health` | Health check |

---

## Configuration

All settings have sensible defaults; override via environment variables or a TOML
file (`--config agent-meter.toml`, see [`agent-meter.example.toml`](agent-meter.example.toml)).

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_METER_HOST` | `127.0.0.1` | Bind address |
| `AGENT_METER_PORT` | `8081` | UI + REST port |
| `AGENT_METER_OTLP_PORT` | `4318` | OTLP receiver port |
| `DATABASE_URL` | `sqlite://agent-meter.db` | SQLite by default; `postgres://…` also supported |
| `AGENT_METER_NO_OPEN` | _(unset)_ | Set to skip auto-opening the browser on `serve` |
| `RUST_LOG` | `info` | Log level |

> **PostgreSQL is optional.** Point `DATABASE_URL` at a `postgres://` URL if you
> want a shared/server deployment; SQLite is the zero-config default.

---

## Project structure

```
agent-meter/
├── crates/
│   ├── collector/          # Axum HTTP server (REST API + OTLP + embedded Web UI)
│   │   ├── src/
│   │   │   ├── routes/      # API + page handlers
│   │   │   ├── otlp/        # OTLP receiver + IDE detection
│   │   │   ├── services/    # Event mapping, ingest buffer
│   │   │   └── demo.rs      # Synthetic data generator (`demo` command)
│   │   └── ui/              # HTML pages + static assets (embedded in the binary)
│   ├── db/                  # Database trait + SQLite & Postgres implementations
│   ├── cli/                 # CLI client
│   ├── proxy/               # HTTPS proxy helpers
│   └── mcp-wrapper/         # MCP wrapper proxy
├── eclipse-proxy/           # mitmproxy addon (Eclipse + Copilot CLI)
├── sdk/                     # Client SDKs (Node, Python)
├── migrations/              # Postgres migrations (optional backend)
├── docs/                    # Documentation
├── install.sh / install.ps1 # Install scripts
└── docker-compose.standalone.yml
```

---

## Privacy

- **Local-first.** Binds to `127.0.0.1` and stores everything in a local SQLite
  file. No external telemetry, no phone-home.
- **No auth tokens stored.** The collector records tool-call metadata, not your
  API credentials.

---

## License

[MIT](LICENSE) © agent-meter contributors
