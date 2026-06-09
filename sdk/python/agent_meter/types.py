"""Data types for Agent Meter spans/events."""

from __future__ import annotations

import time
import uuid
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class ToolCall:
    """A single tool call (span) to be reported."""

    tool_name: str
    started_at_ns: int = field(default_factory=lambda: time.time_ns())
    ended_at_ns: Optional[int] = None
    duration_ms: Optional[int] = None
    ok: bool = True
    error: Optional[str] = None
    model: Optional[str] = None
    mcp_server: Optional[str] = None
    ide: Optional[str] = None
    agent: Optional[str] = None
    conversation_id: Optional[str] = None
    task_id: Optional[str] = None
    trace_id: str = field(default_factory=lambda: uuid.uuid4().hex)
    span_id: str = field(default_factory=lambda: uuid.uuid4().hex[:16])
    parent_span_id: Optional[str] = None
    estimated_input_tokens: Optional[int] = None
    estimated_output_tokens: Optional[int] = None
    usd_cost: Optional[float] = None

    def finish(self, ok: bool = True, error: Optional[str] = None) -> None:
        """Mark the tool call as finished."""
        self.ended_at_ns = time.time_ns()
        self.duration_ms = (self.ended_at_ns - self.started_at_ns) // 1_000_000
        self.ok = ok
        if error:
            self.error = error

    def to_dict(self) -> dict:
        """Serialize to the ingest payload format."""
        d: dict = {
            "tool_name": self.tool_name,
            "started_at": self.started_at_ns,
            "ok": self.ok,
            "trace_id": self.trace_id,
            "span_id": self.span_id,
        }
        if self.ended_at_ns:
            d["ended_at"] = self.ended_at_ns
        if self.duration_ms is not None:
            d["duration_ms"] = self.duration_ms
        if self.error:
            d["error"] = self.error
        if self.model:
            d["model"] = self.model
        if self.mcp_server:
            d["mcp_server"] = self.mcp_server
        if self.ide:
            d["ide"] = self.ide
        if self.agent:
            d["agent"] = self.agent
        if self.conversation_id:
            d["conversation_id"] = self.conversation_id
        if self.task_id:
            d["task_id"] = self.task_id
        if self.parent_span_id:
            d["parent_span_id"] = self.parent_span_id
        if self.estimated_input_tokens is not None:
            d["estimated_input_tokens"] = self.estimated_input_tokens
        if self.estimated_output_tokens is not None:
            d["estimated_output_tokens"] = self.estimated_output_tokens
        if self.usd_cost is not None:
            d["usd_cost"] = self.usd_cost
        return d
