# agent-meter

Lightweight Python SDK to track AI agent tool calls, costs, and latency.

## Install

```bash
# Until the first PyPI release is published:
cd sdk/python
python3 -m pip install -e .

# After publication:
pip install agent-meter
```

## Quick Start (60 seconds)

```python
from agent_meter import AgentMeter

# Initialize (picks up AGENT_METER_API_KEY from env)
am = AgentMeter(api_key="am_live_...")

# Track a tool call
span = am.track("web_search", model="gpt-4o")
result = do_web_search(query)
span.finish()

# Track with tokens/cost
span = am.track("code_review", model="claude-sonnet-4-20250514")
span.estimated_input_tokens = 2000
span.estimated_output_tokens = 500
span.usd_cost = 0.012
span.finish()

# Flush happens automatically every 5s, or on exit
# Force flush:
am.flush()
```

## Configuration

| Env Variable | Description | Default |
|---|---|---|
| `AGENT_METER_API_KEY` | API key (`am_live_...`) | — |
| `AGENT_METER_ENDPOINT` | Server URL | `http://localhost:8081` |
| `AGENT_METER_IDE` | IDE identifier | `python-sdk` |
| `AGENT_METER_AGENT` | Agent name | — |

Or pass directly:

```python
am = AgentMeter(
    api_key="am_live_...",
    endpoint="http://localhost:3000",
    ide="my-custom-agent",
    agent="research-bot",
)
```

## Context Manager

```python
import contextlib

@contextlib.contextmanager
def tracked(am, tool_name, **kwargs):
    span = am.track(tool_name, **kwargs)
    try:
        yield span
    except Exception as e:
        span.finish(ok=False, error=str(e))
        raise
    else:
        span.finish()

# Usage
with tracked(am, "database_query", model="gpt-4o") as span:
    results = db.execute(sql)
    span.estimated_input_tokens = len(sql) // 4
```

## Conversations & Tasks

Group spans into conversations (sessions) and tasks:

```python
span = am.track(
    "generate_code",
    model="claude-sonnet-4-20250514",
    conversation_id="conv-abc123",
    task_id="implement-feature-x",
)
```

## Protocol

The SDK sends spans as OTLP-compatible JSON to `POST /v1/traces`. This is the
same format used by OpenTelemetry, making it compatible with any OTLP collector.
