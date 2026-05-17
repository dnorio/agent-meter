# agent-meter — OTEL & Telemetry Integration

## Architecture

```
┌─────────────────┐     ┌──────────────────────┐     ┌───────────────┐
│   Agent         │────>│  agent-meter          │────>│  PostgreSQL    │
│ (OpenCode,      │POST │  collector             │     │  (events,      │
│  Cursor, etc.)  │JSON  │  :3000                │     │   tasks)       │
│                 │     │                       │     └───────────────┘
│ AGENT_METER_    │     │  ┌─────────────────┐  │     ┌───────────────┐
│ COLLECTOR_URL   │     │  │ OTEL Exporter   │──┼────>│  Jaeger/Tempo │
│                 │     │  │ (opentelemetry-  │  │     │  (opcional)   │
└─────────────────┘     │  │  otlp)          │  │     └───────────────┘
                        │  └─────────────────┘  │
                        └──────────────────────┘
```

There are two separate concerns:

1. **Events ingestion (REST API)** — agents send tool-call data to the collector
2. **OTEL export** — the collector *exports* its own spans (for debugging the collector)

The collector currently **does not** run an OTLP receiver. To send OTEL spans *from agents* to the collector, an OTLP HTTP receiver must be added (see [Future: OTLP Receiver](#future-otlp-receiver)).

---

## 1. Sending Tool-Call Data (REST API)

All agents should send tool-call events via the existing HTTP API.

### Using the CLI

```bash
# Set the collector URL (default: http://localhost:8081)
export AGENT_METER_COLLECTOR_URL=http://agent-meter:3000
export AGENT_METER_TASK_ID=task-abc-123
export AGENT_METER_REPO=agent-meter-worktree
export AGENT_METER_BRANCH=feat/minha-feature
export AGENT_METER_IDE=cursor
export AGENT_METER_AGENT=cursor-agent
export AGENT_METER_SKILL=code-review

# Send a tool-call event
agent-meter event tool-call \
  --tool-name search_code \
  --mcp-server github \
  --ok \
  --request-bytes 250 \
  --response-bytes 12000 \
  --duration-ms 3400
```

### Environment Variables Per Agent

| Agent | `AGENT_METER_IDE` | `AGENT_METER_AGENT` | Notes |
|-------|------------------|---------------------|-------|
| OpenCode | `opencode` | `opencode` | CLI na worktree `~/REDACTED-worktree` |
| Cursor | `cursor` | `cursor` | CLI na worktree `~/REDACTED-worktree` |
| Copilot/VSCode | `copilot-vscode` | `copilot` | CLI na worktree `~/REDACTED-worktree` |
| Antigravity | `antigravity` | `antigravity` | CLI na worktree `~/REDACTED-worktree` |
| Codex | `rust-rover` | `codex` | CLI na worktree `~/REDACTED-worktree-claude` |

### Direct HTTP (without CLI)

```bash
curl -X POST http://agent-meter:3000/events/tool-call \
  -H "content-type: application/json" \
  -d '{
    "event_id": "'$(python3 -c "import uuid; print(uuid.uuid4())")'",
    "task_id": "task-abc",
    "repo": "agent-meter-worktree",
    "branch": "feat/x",
    "ide": "opencode",
    "agent": "opencode",
    "skill": "tool-use",
    "mcp_server": "github",
    "tool_name": "search_code",
    "started_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "ended_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "ok": true,
    "request_bytes": 250,
    "response_bytes": 12000
  }'
```

---

## 2. Task Lifecycle

Agents should create a task at session start and end it at session end:

```bash
# Start a task
agent-meter task start \
  --task-id "session-$(date +%s)" \
  --repo "my-app" \
  --branch "feat/x" \
  --ide "opencode" \
  --agent "opencode" \
  --skill "coding"

# List active tasks
agent-meter task list

# End a task
agent-meter task end --task-id "session-1744819200"
```

---

## 3. Viewing Data

The embedded dashboard is at `GET /` on the collector:

```bash
open http://agent-meter:3000/
# or via kubectl port-forward:
kubectl port-forward svc/agent-meter 3000:3000
```

Reports API:

| Endpoint | Description |
|----------|-------------|
| `GET /reports/top-tools` | Top tools by call count |
| `GET /reports/top-tasks` | Top tasks by duration |
| `GET /reports/top-mcp-servers` | Top MCP servers by call count |
| `GET /tasks` | All tasks (active + ended) |

OTEL spans (if OTEL exporter is configured on the collector):

```bash
# collector exports spans to Tempo/Jaeger when this is set:
export OTEL_EXPORTER_OTLP_ENDPOINT=http://tempo:4318
```

---

## 4. Collector OTEL Configuration

The collector uses `tracing-opentelemetry` with a conditional layer.

| Env Var | Default | Description |
|---------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | (none) | OTLP endpoint to export spans. Unset = no OTEL export. |
| `OTEL_SERVICE_NAME` | `agent-meter` | Service name for OTEL traces |
| `RUST_LOG` | `info` | Log level |

If `OTEL_EXPORTER_OTLP_ENDPOINT` is unset, the collector runs normally without OTEL (zero-crash, zero-overhead).

Spans created per tool-call:

| Span name | Attributes |
|-----------|------------|
| `agent.tool_call` | `event_id`, `task_id`, `tool_name`, `mcp_server`, `duration_ms`, `ok`, `request_bytes`, `response_bytes`, `input_tokens`, `output_tokens`, `total_tokens`, `repo`, `branch`, `ide`, `agent`, `skill` |

---

## 5. Future: OTLP Receiver

To allow agents to send OTEL spans directly to the collector (instead of using the REST API), an OTLP HTTP receiver can be added to the collector:

- Endpoint: `POST /v1/traces` (OTLP HTTP protobuf)
- Integration: `opentelemetry-proto` crate for deserialization
- Mapping: OTLP spans → `agent_tool_calls` table

This is tracked as part of the collector roadmap and would allow agents to just set `OTEL_EXPORTER_OTLP_ENDPOINT=http://agent-meter:4318` without needing the CLI or direct HTTP calls.

---

## 6. Smoke Test

The `scripts/smoke-otel.sh` script validates the end-to-end pipeline:

```bash
# Requires: agent-meter collector running, PostgreSQL accessible
./scripts/smoke-otel.sh
```

The script:
1. Sends a tool-call event via the REST API
2. Queries reports to confirm ingestion
3. Display estimated tokens and duration

See `scripts/smoke-otel.sh` for details.
