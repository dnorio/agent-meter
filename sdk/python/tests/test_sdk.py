"""Tests for agent_meter SDK."""

import json
from unittest.mock import patch, MagicMock

from agent_meter import AgentMeter, ToolCall


def test_tool_call_finish():
    tc = ToolCall(tool_name="test_tool")
    assert tc.ok is True
    tc.finish(ok=False, error="timeout")
    assert tc.ok is False
    assert tc.error == "timeout"
    assert tc.duration_ms is not None
    assert tc.duration_ms >= 0


def test_tool_call_to_dict():
    tc = ToolCall(tool_name="search", model="gpt-4o")
    tc.finish()
    d = tc.to_dict()
    assert d["tool_name"] == "search"
    assert d["model"] == "gpt-4o"
    assert d["ok"] is True
    assert "duration_ms" in d


def test_client_track():
    am = AgentMeter(api_key="test", endpoint="http://localhost:3000", ide="python-sdk", flush_interval=999)
    span = am.track("my_tool", model="gpt-4o")
    assert span.tool_name == "my_tool"
    assert span.ide == "python-sdk"
    span.finish()
    am._closed = True  # prevent flush timer
    if am._timer:
        am._timer.cancel()


def test_client_build_payload():
    am = AgentMeter(api_key="test", endpoint="http://localhost:3000", flush_interval=999)
    span = ToolCall(tool_name="code_gen", model="claude-sonnet-4-20250514")
    span.finish()
    payload = am._build_otlp_payload([span])
    assert "resourceSpans" in payload
    rs = payload["resourceSpans"][0]
    assert rs["scopeSpans"][0]["spans"][0]["name"] == "code_gen"
    am._closed = True
    if am._timer:
        am._timer.cancel()


@patch("agent_meter.client.httpx.post")
def test_flush_sends_to_server(mock_post):
    mock_resp = MagicMock()
    mock_resp.raise_for_status = MagicMock()
    mock_post.return_value = mock_resp

    am = AgentMeter(api_key="am_live_test", endpoint="http://localhost:3000", flush_interval=999)
    span = am.track("tool_a")
    span.finish()
    sent = am.flush()
    assert sent == 1
    mock_post.assert_called_once()
    call_kwargs = mock_post.call_args
    assert "/v1/traces" in call_kwargs.args[0] or "/v1/traces" in str(call_kwargs)
    am._closed = True
    if am._timer:
        am._timer.cancel()
