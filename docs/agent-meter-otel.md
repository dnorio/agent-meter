# agent-meter — OTEL & Telemetry Integration (OSS)

## Architecture

```
┌─────────────────┐     ┌──────────────────────┐     ┌───────────────┐
│   Agent         │────>│  agent-meter          │────>│  SQLite       │
│ (OpenCode,      │POST │  collector             │     │  agent-meter  │
│  Cursor, etc.)  │JSON  │  :8081 (REST + UI)    │     │  .db (local)  │
│                 │     │                       │     └───────────────┘
│ AGENT_METER_    │     │  OTLP :4318           │
│ COLLECTOR_URL   │     └──────────────────────┘
└─────────────────┘

┌─────────────────┐     ┌──────────────────────┐
│   VSCode        │────>│  agent-meter          │
│ (Copilot Chat)  │OTLP │  :4318/v1/traces      │
└─────────────────┘     └──────────────────────┘
```

Three concerns:

1. **REST API** — `POST /events/tool-call` (port `8081`)
2. **OTLP receiver** — VS Code Copilot → `POST /v1/traces` (port `4318`)
3. **OTEL export (optional)** — collector can export its own debug spans to Jaeger/Tempo

Default storage is **SQLite** (`agent-meter.db`). Set `DATABASE_URL=postgres://…` only
if you need a shared backend.

---

## 1. OTLP Receiver (VS Code Copilot)

```json
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.otlpEndpoint": "http://127.0.0.1:4318",
  "github.copilot.chat.otel.captureContent": false
}
```

Start the collector:

```bash
agent-meter serve
# UI + REST → http://127.0.0.1:8081
# OTLP       → http://127.0.0.1:4318
```

### WSL

If VS Code runs in WSL and the collector runs on the same machine, use
`http://127.0.0.1:4318`. See [QUICKSTART-WSL-COPILOT.md](QUICKSTART-WSL-COPILOT.md).

---

## 2. REST API (other agents)

```bash
export AGENT_METER_COLLECTOR_URL=http://127.0.0.1:8081
export AGENT_METER_TASK_ID=task-abc-123
export AGENT_METER_REPO=my-app
export AGENT_METER_BRANCH=feat/x
export AGENT_METER_IDE=opencode
export AGENT_METER_AGENT=opencode
```

```bash
curl -X POST http://127.0.0.1:8081/events/tool-call \
  -H "content-type: application/json" \
  -d '{
    "event_id": "'$(python3 -c "import uuid; print(uuid.uuid4())")'",
    "task_id": "task-abc",
    "repo": "my-app",
    "branch": "feat/x",
    "ide": "opencode",
    "agent": "opencode",
    "tool_name": "search_code",
    "started_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "ended_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "ok": true
  }'
```

---

## 3. Task lifecycle (CLI)

```bash
agent-meter task start \
  --task-id "session-$(date +%s)" \
  --repo "my-app" \
  --branch "feat/x" \
  --ide "opencode" \
  --agent "opencode"

agent-meter task list
agent-meter task end --task-id "session-1744819200"
```

---

## 4. Viewing data

Dashboard: `http://127.0.0.1:8081/`

| Endpoint | Description |
|----------|-------------|
| `GET /reports/top-tools` | Top tools by call count |
| `GET /reports/top-tasks` | Top tasks by duration |
| `GET /api/conversations` | Grouped agent sessions |

---

## 5. Collector OTEL export (optional)

| Env Var | Default | Description |
|---------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | (none) | Export collector debug spans |
| `OTEL_SERVICE_NAME` | `agent-meter` | Service name |
| `RUST_LOG` | `info` | Log level |

---

## 6. Smoke test

```bash
agent-meter serve &
bash scripts/smoke-otel.sh
```

See [capture-setup.md](capture-setup.md) for IDE-specific capture guides.
