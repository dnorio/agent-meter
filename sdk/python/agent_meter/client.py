"""Agent Meter client — batches and flushes tool calls via OTLP JSON."""

from __future__ import annotations

import atexit
import os
import threading
import time
from typing import Optional

import httpx

from agent_meter.types import ToolCall

_DEFAULT_ENDPOINT = "http://localhost:8081"
_FLUSH_INTERVAL = 5.0  # seconds
_MAX_BATCH = 100


class AgentMeter:
    """Lightweight client that buffers tool calls and flushes to agent-meter.

    Usage::

        from agent_meter import AgentMeter

        am = AgentMeter(api_key="am_live_...")
        span = am.track("my_tool", model="gpt-4o")
        # ... do work ...
        span.finish()
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        endpoint: Optional[str] = None,
        ide: Optional[str] = None,
        agent: Optional[str] = None,
        flush_interval: float = _FLUSH_INTERVAL,
    ):
        self._api_key = api_key or os.environ.get("AGENT_METER_API_KEY", "")
        self._endpoint = (
            endpoint or os.environ.get("AGENT_METER_ENDPOINT", _DEFAULT_ENDPOINT)
        ).rstrip("/")
        self._ide = ide or os.environ.get("AGENT_METER_IDE", "python-sdk")
        self._agent = agent or os.environ.get("AGENT_METER_AGENT")
        self._buffer: list[ToolCall] = []
        self._lock = threading.Lock()
        self._flush_interval = flush_interval
        self._closed = False

        # Background flush thread
        self._timer: Optional[threading.Timer] = None
        self._schedule_flush()
        atexit.register(self.shutdown)

    def track(
        self,
        tool_name: str,
        *,
        model: Optional[str] = None,
        mcp_server: Optional[str] = None,
        conversation_id: Optional[str] = None,
        task_id: Optional[str] = None,
        parent_span_id: Optional[str] = None,
    ) -> ToolCall:
        """Start tracking a tool call. Call .finish() when done."""
        span = ToolCall(
            tool_name=tool_name,
            model=model,
            mcp_server=mcp_server,
            ide=self._ide,
            agent=self._agent,
            conversation_id=conversation_id,
            task_id=task_id,
            parent_span_id=parent_span_id,
        )
        with self._lock:
            self._buffer.append(span)
        return span

    def flush(self) -> int:
        """Flush buffered spans to the server. Returns count sent."""
        with self._lock:
            if not self._buffer:
                return 0
            batch = self._buffer[:_MAX_BATCH]
            self._buffer = self._buffer[_MAX_BATCH:]

        payload = self._build_otlp_payload(batch)
        try:
            resp = httpx.post(
                f"{self._endpoint}/v1/traces",
                json=payload,
                headers=self._headers(),
                timeout=10.0,
            )
            resp.raise_for_status()
        except httpx.HTTPError:
            # Re-queue on failure (best-effort)
            with self._lock:
                self._buffer = batch + self._buffer
            return 0
        return len(batch)

    def shutdown(self) -> None:
        """Flush remaining spans and stop background thread."""
        self._closed = True
        if self._timer:
            self._timer.cancel()
        # Flush all remaining
        while True:
            sent = self.flush()
            if sent == 0:
                break

    def _schedule_flush(self) -> None:
        if self._closed:
            return
        self._timer = threading.Timer(self._flush_interval, self._tick)
        self._timer.daemon = True
        self._timer.start()

    def _tick(self) -> None:
        self.flush()
        self._schedule_flush()

    def _headers(self) -> dict[str, str]:
        h: dict[str, str] = {"Content-Type": "application/json"}
        if self._api_key:
            h["Authorization"] = f"Bearer {self._api_key}"
        return h

    def _build_otlp_payload(self, spans: list[ToolCall]) -> dict:
        """Build OTLP-compatible JSON payload."""
        otlp_spans = []
        for s in spans:
            attrs = [{"key": "tool.name", "value": {"stringValue": s.tool_name}}]
            if s.model:
                attrs.append({"key": "gen_ai.request.model", "value": {"stringValue": s.model}})
            if s.mcp_server:
                attrs.append({"key": "mcp.server", "value": {"stringValue": s.mcp_server}})
            if s.ide:
                attrs.append({"key": "ide", "value": {"stringValue": s.ide}})
            if s.agent:
                attrs.append({"key": "agent", "value": {"stringValue": s.agent}})
            if s.conversation_id:
                attrs.append({"key": "session.id", "value": {"stringValue": s.conversation_id}})
            if s.task_id:
                attrs.append({"key": "task.id", "value": {"stringValue": s.task_id}})
            if s.estimated_input_tokens is not None:
                attrs.append({"key": "gen_ai.usage.input_tokens", "value": {"intValue": str(s.estimated_input_tokens)}})
            if s.estimated_output_tokens is not None:
                attrs.append({"key": "gen_ai.usage.output_tokens", "value": {"intValue": str(s.estimated_output_tokens)}})
            if s.usd_cost is not None:
                attrs.append({"key": "cost.usd", "value": {"doubleValue": s.usd_cost}})
            if s.error:
                attrs.append({"key": "error.message", "value": {"stringValue": s.error}})

            end_ns = s.ended_at_ns or s.started_at_ns
            status_code = 1 if s.ok else 2
            status_msg = s.error or "" if not s.ok else ""

            otlp_spans.append({
                "traceId": s.trace_id,
                "spanId": s.span_id,
                "parentSpanId": s.parent_span_id or "",
                "name": s.tool_name,
                "kind": 3,  # INTERNAL
                "startTimeUnixNano": str(s.started_at_ns),
                "endTimeUnixNano": str(end_ns),
                "attributes": attrs,
                "status": {"code": status_code, "message": status_msg},
            })

        return {
            "resourceSpans": [
                {
                    "resource": {
                        "attributes": [
                            {"key": "service.name", "value": {"stringValue": "agent-meter-sdk-python"}},
                        ]
                    },
                    "scopeSpans": [
                        {
                            "scope": {"name": "agent-meter-python", "version": "0.1.0"},
                            "spans": otlp_spans,
                        }
                    ],
                }
            ]
        }
