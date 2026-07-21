# agentmeter-obs

Lightweight Node.js/TypeScript SDK to track AI agent tool calls, costs, and latency.

## Install

```bash
# From npm (after publication):
npm install agentmeter-obs

# From a local tarball:
cd sdk/node && npm ci && npm pack
npm install ./agentmeter-obs-0.1.0.tgz
```

## Quick Start (60 seconds)

```typescript
import { AgentMeter } from "agentmeter-obs";

const am = new AgentMeter({ apiKey: "am_live_..." });

// Track a tool call
const span = am.track("web_search", { model: "gpt-4o" });
const result = await doWebSearch(query);
am.finish(span);

// Track with tokens/cost
const span2 = am.track("code_review", { model: "claude-sonnet-4-20250514" });
span2.estimatedInputTokens = 2000;
span2.estimatedOutputTokens = 500;
span2.usdCost = 0.012;
am.finish(span2);

// Flush happens automatically every 5s
// Force flush:
await am.flush();
```

## Configuration

| Env Variable | Description | Default |
|---|---|---|
| `AGENT_METER_API_KEY` | API key (`am_live_...`) | — |
| `AGENT_METER_ENDPOINT` | Server URL | `http://localhost:8081` |
| `AGENT_METER_IDE` | IDE identifier | `node-sdk` |
| `AGENT_METER_AGENT` | Agent name | — |

Or pass via constructor:

```typescript
const am = new AgentMeter({
  apiKey: "am_live_...",
  endpoint: "http://localhost:3000",
  ide: "my-custom-agent",
  agent: "research-bot",
});
```

## Conversations & Tasks

```typescript
const span = am.track("generate_code", {
  model: "claude-sonnet-4-20250514",
  conversationId: "conv-abc123",
  taskId: "implement-feature-x",
});
```

## Protocol

The SDK sends spans as OTLP-compatible JSON to `POST /v1/traces`.
