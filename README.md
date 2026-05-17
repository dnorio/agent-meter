# agent-meter

Lightweight, open-source observability and FinOps collector for agentic development workflows.

Tracks MCP tool calls, estimated token usage, payload size, latency, errors and task-level cost across tools like Cursor, VS Code Copilot, Antigravity, Claude Code, OpenCode and custom agents.

## Quick start

```bash
# Start PostgreSQL
docker compose up -d postgres

# Run migrations
cargo sqlx migrate run

# Start collector
cargo run -p collector
```

## Send a test event

```bash
curl -X POST http://localhost:8081/events/tool-call \
  -H "Content-Type: application/json" \
  -d '{
    "tool_name": "read_file",
    "mcp_server": "filesystem",
    "started_at": "2026-05-17T12:00:00Z",
    "ended_at": "2026-05-17T12:00:01Z",
    "ok": true,
    "request_bytes": 1200,
    "response_bytes": 30000
  }'
```

## Reports

```bash
curl http://localhost:8081/reports/top-tools
curl http://localhost:8081/reports/top-tasks
curl http://localhost:8081/reports/top-mcp-servers
```

## Deploy

```bash
./deploy.sh
```

## Project structure

```
apps/agent-meter/
├── crates/collector/   # HTTP collector API (Axum)
├── crates/cli/          # CLI client (WIP)
├── crates/mcp-wrapper/  # MCP wrapper proxy (WIP)
├── migrations/          # SQLx migrations
├── docker-compose.yml   # Local dev
├── Dockerfile           # ARM64 build
└── deploy.sh            # OCI cluster deploy
```

## Security

- No full prompts or responses stored by default
- Only SHA-256 hashes, byte sizes, and metadata
- No secrets in metadata
- Local-first by design
