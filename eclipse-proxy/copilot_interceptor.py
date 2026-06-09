"""
mitmproxy addon: Intercepts GitHub Copilot API calls from Eclipse's
copilot-language-server.exe and forwards telemetry to agent-meter.

Usage:
    mitmdump -p 8899 -s copilot_interceptor.py --set confdir=~/.mitmproxy

The addon captures:
    - POST /chat/completions (chat messages)
    - POST /v1/engines/*/completions (inline completions)
    - Any request to *githubcopilot* or *api.github.com* with copilot paths
"""

import json
import time
import threading
import httpx
from mitmproxy import http, ctx

AGENT_METER_URL = "http://localhost:8081/v1/traces"
# Also send as structured event for direct ingestion
AGENT_METER_EVENT_URL = "http://localhost:8081/api/events"

# Hosts that carry Copilot traffic
COPILOT_HOSTS = [
    "api.githubcopilot.com",
    "api.business.githubcopilot.com",
    "api.individual.githubcopilot.com",
    "copilot-proxy.githubusercontent.com",
    "api.github.com",
]

# Paths that indicate LLM/chat activity
COPILOT_PATHS = [
    "/chat/completions",
    "/v1/engines/",
    "/completions",
    "/models",
    "/responses",
]


def is_copilot_request(flow: http.HTTPFlow) -> bool:
    """Check if this request is a Copilot API call."""
    host = flow.request.pretty_host

    # Check host
    host_match = any(h in host for h in COPILOT_HOSTS)
    if not host_match:
        return False

    # Any request to copilot hosts is interesting, but prioritize known paths
    return True


def is_llm_call(flow: http.HTTPFlow) -> bool:
    """Check if this is specifically an LLM completion/chat call."""
    path = flow.request.path
    return any(p in path for p in COPILOT_PATHS)


import re

_ATTACHMENTS_RE = re.compile(r'<attachments>.*?</attachments>\s*', re.DOTALL)
_ATTACHMENT_TAG_RE = re.compile(r'<attachment\b[^>]*>.*?(?:</attachment>|$)', re.DOTALL)
_USER_REQUEST_RE = re.compile(r'<userRequest>\s*(.*?)\s*</userRequest>', re.DOTALL)

def _clean_prompt(text: str) -> str:
    """Strip Copilot XML attachment wrappers from prompt text, returning the user's actual message."""
    if not text:
        return ""
    # First try: extract <userRequest> tag (Eclipse Copilot agent format)
    match = _USER_REQUEST_RE.search(text)
    if match:
        return match.group(1).strip()
    # Remove <attachments>...</attachments> blocks
    cleaned = _ATTACHMENTS_RE.sub('', text)
    # Remove standalone <attachment .../> tags too
    cleaned = _ATTACHMENT_TAG_RE.sub('', cleaned)
    # Remove full context blocks (opening tag + content + closing tag)
    cleaned = re.sub(r'<(?:skill-context|current_datetime|environment_info|workspace_info|userMemory|sessionMemory|repoMemory|conversation-summary|securityRequirements|operationalSafety|implementationDiscipline|toolUseInstructions|communicationStyle|outputFormatting|memoryInstructions|notebookInstructions)\b[^>]*>[\s\S]*?</[^>]+>', '', cleaned)
    # Remove any remaining XML-like tags that Copilot injects
    cleaned = re.sub(r'</?(?:context|references|instructions|response|environment_info|editorContext|reminderInstructions|workspace_info|skill-context|current_datetime)\b[^>]*>', '', cleaned)
    # Strip "The current date is..." context prefix lines
    cleaned = re.sub(r'^The current date is \d{4}-\d{2}-\d{2}[^\n]*\n?', '', cleaned, flags=re.MULTILINE)
    cleaned = re.sub(r'^Terminals?:\s*Terminal:[^\n]*\n?', '', cleaned, flags=re.MULTILINE)
    # Collapse whitespace
    cleaned = re.sub(r'\s+', ' ', cleaned).strip()
    return cleaned if cleaned else ""


def extract_request_meta(flow: http.HTTPFlow) -> dict:
    """Extract metadata from the request."""
    meta = {
        "method": flow.request.method,
        "url": flow.request.url,
        "host": flow.request.pretty_host,
        "path": flow.request.path,
        "started_at": flow.request.timestamp_start,
    }

    # Request size
    if flow.request.content:
        meta["request_bytes"] = len(flow.request.content)

    # Try to extract model and messages from request body
    if flow.request.content and flow.request.method == "POST":
        try:
            body = json.loads(flow.request.content)
            if "model" in body:
                meta["model"] = body["model"]
            # Request parameters
            if "max_output_tokens" in body:
                meta["max_output_tokens"] = body["max_output_tokens"]
            elif "max_tokens" in body:
                meta["max_output_tokens"] = body["max_tokens"]
            if "temperature" in body:
                meta["temperature"] = body["temperature"]
            if "messages" in body:
                messages = body["messages"]
                meta["message_count"] = len(messages)
                # Get the last user message as prompt summary
                user_msgs = [m for m in messages if m.get("role") == "user"]
                if user_msgs:
                    last_msg = user_msgs[-1].get("content", "")
                    if isinstance(last_msg, str):
                        meta["user_prompt"] = _clean_prompt(last_msg)
                    elif isinstance(last_msg, list):
                        # Multi-part content
                        text_parts = [p.get("text", "") for p in last_msg if p.get("type") == "text"]
                        meta["user_prompt"] = _clean_prompt(" ".join(text_parts))
            # New /responses API format (input as array of items)
            if "input" in body and isinstance(body["input"], list):
                input_items = body["input"]
                meta["input_items"] = input_items  # Keep full input for tool call extraction
                # Find the actual user prompt — search all user messages for <userRequest>
                # or use the last user message with _clean_prompt
                best_prompt = ""
                for item in input_items:
                    if item.get("role") == "user":
                        content = item.get("content", "")
                        raw_text = ""
                        if isinstance(content, str):
                            raw_text = content
                        elif isinstance(content, list):
                            text_parts = [p.get("text", "") for p in content if p.get("type") in ("input_text", "text")]
                            raw_text = " ".join(text_parts)
                        # Check if this message contains <userRequest>
                        if "<userRequest>" in raw_text:
                            cleaned = _clean_prompt(raw_text)
                            if cleaned:
                                best_prompt = cleaned
                                break
                if not best_prompt:
                    # Fallback: use last user message cleaned
                    for item in reversed(input_items):
                        if item.get("role") == "user":
                            content = item.get("content", "")
                            if isinstance(content, str):
                                best_prompt = _clean_prompt(content)
                            elif isinstance(content, list):
                                text_parts = [p.get("text", "") for p in content if p.get("type") in ("input_text", "text")]
                                best_prompt = _clean_prompt(" ".join(text_parts))
                            break
                if best_prompt:
                    meta["user_prompt"] = best_prompt
                # Also extract tool call results from input (these fill the gaps)
                tool_results = []
                for item in input_items:
                    if item.get("type") == "function_call_output":
                        tool_results.append({
                            "call_id": item.get("call_id", ""),
                            "output": item.get("output", ""),
                        })
                if tool_results:
                    meta["tool_results_in_input"] = tool_results
            if "stream" in body:
                meta["stream"] = body["stream"]
        except (json.JSONDecodeError, KeyError, TypeError):
            pass  # Best-effort metadata extraction; malformed body is non-fatal

    # Extract useful headers
    headers = flow.request.headers
    if "x-request-id" in headers:
        meta["request_id"] = headers["x-request-id"]
    if "vscode-sessionid" in headers:
        meta["session_id"] = headers["vscode-sessionid"]
    if "editor-version" in headers:
        meta["editor_version"] = headers["editor-version"]
    if "copilot-integration-id" in headers:
        meta["integration_id"] = headers["copilot-integration-id"]
    if "openai-intent" in headers:
        meta["intent"] = headers["openai-intent"]

    return meta


def extract_response_meta(flow: http.HTTPFlow) -> dict:
    """Extract metadata from the response."""
    meta = {
        "status_code": flow.response.status_code,
        "ended_at": flow.response.timestamp_end,
        "duration_ms": int((flow.response.timestamp_end - flow.request.timestamp_start) * 1000),
    }

    # Response size
    if flow.response.content:
        meta["response_bytes"] = len(flow.response.content)

    if flow.response.content and not flow.response.headers.get("content-type", "").startswith("text/event-stream"):
        try:
            body = json.loads(flow.response.content)
            if "usage" in body:
                usage = body["usage"]
                meta["input_tokens"] = usage.get("prompt_tokens") or usage.get("input_tokens", 0)
                meta["output_tokens"] = usage.get("completion_tokens") or usage.get("output_tokens", 0)
                meta["total_tokens"] = usage.get("total_tokens", 0)
            if "model" in body:
                meta["model"] = body["model"]
            if "choices" in body and body["choices"]:
                choice = body["choices"][0]
                if "message" in choice:
                    content = choice["message"].get("content", "")
                    meta["response_text"] = content if content else ""
                    meta["response_preview"] = content[:200] if content else ""
                    # Extract tool_calls from chat completions response
                    tool_calls = choice["message"].get("tool_calls", [])
                    if tool_calls:
                        meta["tool_calls"] = tool_calls
                elif "text" in choice:
                    meta["response_text"] = choice["text"]
                    meta["response_preview"] = choice["text"][:200]
            # New /responses API format — extract output items
            if "output" in body and isinstance(body["output"], list):
                tool_calls = []
                reasoning_text = ""
                response_text = ""
                for item in body["output"]:
                    item_type = item.get("type", "")
                    if item_type == "function_call":
                        tool_calls.append({
                            "call_id": item.get("call_id", ""),
                            "name": item.get("name", ""),
                            "arguments": item.get("arguments", ""),
                        })
                    elif item_type == "reasoning":
                        # Extract reasoning/thinking content
                        summary = item.get("summary", [])
                        if summary:
                            reasoning_text = " ".join(
                                s.get("text", "") for s in summary if s.get("type") == "summary_text"
                            )
                    elif item_type == "message" and "content" in item:
                        for part in item["content"]:
                            if part.get("type") == "output_text":
                                response_text += part.get("text", "")
                if tool_calls:
                    meta["tool_calls"] = tool_calls
                if reasoning_text:
                    meta["reasoning"] = reasoning_text
                if response_text:
                    meta["response_text"] = response_text
                    meta["response_preview"] = response_text[:200]
        except (json.JSONDecodeError, KeyError, TypeError):
            pass  # Best-effort response parsing; malformed JSON is non-fatal
    elif flow.response.content and flow.response.headers.get("content-type", "").startswith("text/event-stream"):
        # Parse SSE stream — look for response.completed (Responses API) or last data chunk (chat completions)
        try:
            raw = flow.response.content.decode("utf-8", errors="replace")
            lines = raw.split("\n")

            # First try: find "response.completed" event (Responses API SSE)
            found_completed = False
            current_event_type = None
            for line in lines:
                if line.startswith("event: "):
                    current_event_type = line[7:].strip()
                elif line.startswith("data: ") and current_event_type == "response.completed":
                    data_str = line[6:]
                    try:
                        envelope = json.loads(data_str)
                        resp_body = envelope.get("response", envelope)
                        # Extract usage
                        if "usage" in resp_body:
                            usage = resp_body["usage"]
                            meta["input_tokens"] = usage.get("prompt_tokens") or usage.get("input_tokens", 0)
                            meta["output_tokens"] = usage.get("completion_tokens") or usage.get("output_tokens", 0)
                            meta["total_tokens"] = usage.get("total_tokens", 0)
                            # Cached tokens (prompt cache hits)
                            input_details = usage.get("input_tokens_details", {})
                            cached = input_details.get("cached_tokens", 0)
                            if cached:
                                meta["cached_tokens"] = cached
                            # Reasoning tokens (o1/o3/thinking)
                            output_details = usage.get("output_tokens_details", {})
                            reasoning_tok = output_details.get("reasoning_tokens", 0)
                            if reasoning_tok:
                                meta["reasoning_tokens"] = reasoning_tok
                        if "model" in resp_body:
                            meta["model"] = resp_body["model"]
                        # Extract output items (tool calls, reasoning, messages)
                        output = resp_body.get("output", [])
                        tool_calls = []
                        reasoning_text = ""
                        response_text = ""
                        for item in output:
                            item_type = item.get("type", "")
                            if item_type == "function_call":
                                tool_calls.append({
                                    "call_id": item.get("call_id", ""),
                                    "name": item.get("name", ""),
                                    "arguments": item.get("arguments", ""),
                                })
                            elif item_type == "reasoning":
                                summary = item.get("summary", [])
                                if summary:
                                    reasoning_text = " ".join(
                                        s.get("text", "") for s in summary if s.get("type") == "summary_text"
                                    )
                            elif item_type == "message" and "content" in item:
                                for part in item["content"]:
                                    if part.get("type") == "output_text":
                                        response_text += part.get("text", "")
                        if tool_calls:
                            meta["tool_calls"] = tool_calls
                        if reasoning_text:
                            meta["reasoning"] = reasoning_text
                        if response_text:
                            meta["response_text"] = response_text
                            meta["response_preview"] = response_text[:200]
                        # Finish reason: Responses API uses "status" field
                        # "completed" = normal stop, "incomplete" = length/etc
                        resp_status = resp_body.get("status", "")
                        if resp_status == "completed":
                            meta["finish_reason"] = "stop" if not tool_calls else "tool_calls"
                        elif resp_status == "incomplete":
                            incomplete_details = resp_body.get("incomplete_details", {})
                            meta["finish_reason"] = incomplete_details.get("reason", "length")
                        elif resp_status:
                            meta["finish_reason"] = resp_status
                        found_completed = True
                    except (json.JSONDecodeError, KeyError, TypeError):
                        pass  # Skip malformed SSE chunk; continue to fallback
                    break

            # Fallback: chat completions SSE — find last data chunk with usage
            if not found_completed:
                for line in reversed(lines):
                    if line.startswith("data: ") and line != "data: [DONE]":
                        try:
                            chunk = json.loads(line[6:])
                            if "usage" in chunk:
                                usage = chunk["usage"]
                                meta["input_tokens"] = usage.get("prompt_tokens") or usage.get("input_tokens", 0)
                                meta["output_tokens"] = usage.get("completion_tokens") or usage.get("output_tokens", 0)
                                meta["total_tokens"] = usage.get("total_tokens", 0)
                            if "model" in chunk:
                                meta["model"] = chunk["model"]
                            break
                        except (json.JSONDecodeError, KeyError, TypeError):
                            continue
        except (UnicodeDecodeError, Exception):
            pass  # SSE stream decode failure; metadata remains partial

    return meta


def send_to_agent_meter(event: dict):
    """Send captured event to agent-meter in background."""
    def _send():
        try:
            # Send as a direct event payload
            resp = httpx.post(
                AGENT_METER_EVENT_URL,
                json=event,
                timeout=5.0,
                verify=False,  # Our custom CA might not be in system store
            )
            if resp.status_code < 300:
                ctx.log.info(f"[copilot-interceptor] Event sent: {event.get('model', 'unknown')} → {resp.status_code}")
            else:
                ctx.log.warn(f"[copilot-interceptor] Event send failed: {resp.status_code} {resp.text[:100]}")
        except Exception as e:
            ctx.log.warn(f"[copilot-interceptor] Failed to send event: {e}")

    threading.Thread(target=_send, daemon=True).start()


def send_otlp_span(req_meta: dict, resp_meta: dict, tool_call: dict = None, parent_span_id: str = None, shared_trace_id: str = None):
    """Send an OTLP-compatible span to agent-meter.

    If tool_call is provided, emits a tool span instead of an LLM span.
    Returns the generated span_id (useful for parent linking).
    """
    import struct
    import os

    span_id = os.urandom(8).hex()
    trace_id = shared_trace_id or os.urandom(16).hex()

    # Determine span name based on whether this is a tool call or LLM call
    if tool_call:
        # Use "execute_tool <name>" format recognized by agent-meter backend
        tool_name = tool_call.get("name", "tool_call")
        span_name = f"execute_tool {tool_name}"
    else:
        span_name = req_meta.get("method", "POST")

    span = {
        "traceId": trace_id,
        "spanId": span_id,
        "name": span_name,
        "kind": 3,  # CLIENT
        "startTimeUnixNano": str(int(req_meta.get("started_at", time.time()) * 1_000_000_000)),
        "endTimeUnixNano": str(int(resp_meta.get("ended_at", time.time()) * 1_000_000_000)),
        "attributes": [],
        "status": {"code": 1 if resp_meta.get("status_code", 200) < 400 else 2},
    }
    if parent_span_id:
        span["parentSpanId"] = parent_span_id

    # Add attributes
    attrs = {}

    if tool_call:
        # Tool call span
        attrs["gen_ai.tool.name"] = tool_call.get("name", "")
        # Parse arguments JSON — use key the backend expects
        args_str = tool_call.get("arguments", "")
        if args_str:
            attrs["gen_ai.tool.call.arguments"] = args_str
        attrs["gen_ai.tool.call_id"] = tool_call.get("call_id", "")
        # Tool call result (output) — from correlated function_call_output
        result_str = tool_call.get("result", "")
        if result_str:
            attrs["gen_ai.tool.call.result"] = result_str
    else:
        # LLM span
        attrs["http.method"] = req_meta.get("method", "POST")
        attrs["http.url"] = req_meta.get("url", "")
        attrs["http.host"] = req_meta.get("host", "")
        attrs["http.status_code"] = resp_meta.get("status_code", 0)
        attrs["http.request.duration_ms"] = resp_meta.get("duration_ms", 0)
        # LLM response text (what the assistant said)
        response_text = resp_meta.get("response_text", "")
        if response_text:
            attrs["gen_ai.tool.call.result"] = response_text

    if "model" in req_meta:
        attrs["gen_ai.request.model"] = req_meta["model"]
    if "model" in resp_meta:
        attrs["gen_ai.response.model"] = resp_meta["model"]
    if "input_tokens" in resp_meta:
        attrs["gen_ai.usage.input_tokens"] = resp_meta["input_tokens"]
    if "output_tokens" in resp_meta:
        attrs["gen_ai.usage.output_tokens"] = resp_meta["output_tokens"]
    if "cached_tokens" in resp_meta:
        attrs["gen_ai.usage.cached_tokens"] = resp_meta["cached_tokens"]
    if "reasoning_tokens" in resp_meta:
        attrs["gen_ai.usage.reasoning_tokens"] = resp_meta["reasoning_tokens"]
    if not tool_call and "user_prompt" in req_meta:
        attrs["gen_ai.prompt"] = req_meta["user_prompt"]
    if "intent" in req_meta:
        attrs["copilot.intent"] = req_meta["intent"]
    if "session_id" in req_meta:
        attrs["copilot.session_id"] = req_meta["session_id"]
        attrs["copilot.conversation.id"] = req_meta["session_id"]
    # Add reasoning if present
    if not tool_call and "reasoning" in resp_meta:
        attrs["gen_ai.reasoning"] = resp_meta["reasoning"]
    # Finish reason
    if "finish_reason" in resp_meta:
        attrs["gen_ai.response.finish_reason"] = resp_meta["finish_reason"]
    # Request/response bytes
    if "request_bytes" in req_meta:
        attrs["gen_ai.request.bytes"] = req_meta["request_bytes"]
    if "response_bytes" in resp_meta:
        attrs["gen_ai.response.bytes"] = resp_meta["response_bytes"]
    # Request parameters
    if "max_output_tokens" in req_meta:
        attrs["gen_ai.request.max_tokens"] = req_meta["max_output_tokens"]
    if "temperature" in req_meta:
        attrs["gen_ai.request.temperature"] = req_meta["temperature"]
    # LLM system (all Copilot traffic goes through OpenAI)
    attrs["gen_ai.system"] = "openai"

    for k, v in attrs.items():
        if isinstance(v, int):
            span["attributes"].append({"key": k, "value": {"intValue": str(v)}})
        elif isinstance(v, float):
            span["attributes"].append({"key": k, "value": {"doubleValue": v}})
        elif isinstance(v, str):
            span["attributes"].append({"key": k, "value": {"stringValue": v}})

    payload = {
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": "eclipse-copilot"}},
                    {"key": "service.namespace", "value": {"stringValue": "ide"}},
                    {"key": "deployment.environment", "value": {"stringValue": "dev"}},
                ]
            },
            "scopeSpans": [{
                "scope": {"name": "copilot-interceptor", "version": "1.0.0"},
                "spans": [span],
            }]
        }]
    }

    def _send():
        try:
            resp = httpx.post(
                AGENT_METER_URL,
                json=payload,
                headers={"Content-Type": "application/json"},
                timeout=5.0,
                verify=False,
            )
            ctx.log.info(f"[copilot-interceptor] OTLP span sent: {resp.status_code}")
        except Exception as e:
            ctx.log.warn(f"[copilot-interceptor] OTLP send failed: {e}")

    threading.Thread(target=_send, daemon=True).start()
    return span_id, trace_id


class CopilotInterceptor:
    """mitmproxy addon that captures Copilot API traffic."""

    def __init__(self):
        self.call_count = 0
        self.llm_count = 0
        # Track pending tool calls awaiting results (call_id → {name, arguments, req_meta, resp_meta})
        self.pending_tool_calls = {}
        ctx.log.info("[copilot-interceptor] Addon loaded. Watching for Copilot traffic...")

    def response(self, flow: http.HTTPFlow):
        """Called when a response is received."""
        if not is_copilot_request(flow):
            return

        self.call_count += 1
        req_meta = extract_request_meta(flow)
        resp_meta = extract_response_meta(flow)

        is_llm = is_llm_call(flow)
        if is_llm:
            self.llm_count += 1

        model = req_meta.get("model") or resp_meta.get("model") or "unknown"
        duration = resp_meta.get("duration_ms", 0)
        tokens_in = resp_meta.get("input_tokens", 0)
        tokens_out = resp_meta.get("output_tokens", 0)
        tool_calls = resp_meta.get("tool_calls", [])

        ctx.log.info(
            f"[copilot-interceptor] #{self.call_count} "
            f"{'LLM' if is_llm else 'API'} "
            f"{req_meta.get('method')} {req_meta.get('host')}{req_meta.get('path', '')[:60]} "
            f"→ {resp_meta.get('status_code')} "
            f"[{duration}ms, model={model}, in={tokens_in}, out={tokens_out}, tools={len(tool_calls)}]"
        )

        # --- Step 1: Emit tool-result spans for tool calls that now have results ---
        # Check if this request's input contains function_call_output items
        # These are results from tools called in a PREVIOUS response
        tool_results_in_input = req_meta.get("tool_results_in_input", [])
        input_items = req_meta.get("input_items", [])
        if tool_results_in_input:
            # Build lookup: call_id → function_call item (for name + arguments)
            fc_lookup = {}
            for item in input_items:
                if item.get("type") == "function_call":
                    fc_lookup[item.get("call_id", "")] = item

            for tr in tool_results_in_input:
                call_id = tr.get("call_id", "")
                result_output = tr.get("output", "")
                fc = fc_lookup.get(call_id) or self.pending_tool_calls.get(call_id, {})
                tool_name = fc.get("name", "unknown_tool")
                tool_args = fc.get("arguments", "")

                # Emit a complete tool span with arguments + result
                tc_with_result = {
                    "name": tool_name,
                    "arguments": tool_args,
                    "call_id": call_id,
                    "result": result_output,
                }
                ctx.log.info(f"[copilot-interceptor]   ✓ tool_result: {tool_name} ({len(result_output)} chars)")
                send_otlp_span(req_meta, resp_meta, tool_call=tc_with_result)

                # Remove from pending
                self.pending_tool_calls.pop(call_id, None)

        # --- Step 2: Send the main LLM span (with response text as output) ---
        # Generate a shared trace_id for this request+tools group
        import os
        shared_trace_id = os.urandom(16).hex()
        llm_span_id, _ = send_otlp_span(req_meta, resp_meta, shared_trace_id=shared_trace_id)

        # --- Step 3: Store new tool calls from this response for later correlation ---
        # Emit tool call spans as children of the LLM span
        for tc in tool_calls:
            tc_name = tc.get("name", "unknown_tool")
            call_id = tc.get("call_id", "")
            ctx.log.info(f"[copilot-interceptor]   → tool_call pending: {tc_name} (id={call_id[:12]})")
            # Store for correlation with next request's function_call_output
            if call_id:
                self.pending_tool_calls[call_id] = {
                    "name": tc_name,
                    "arguments": tc.get("arguments", ""),
                    "call_id": call_id,
                }
            # Emit tool call span as child of the LLM span
            send_otlp_span(req_meta, resp_meta, tool_call=tc,
                          parent_span_id=llm_span_id, shared_trace_id=shared_trace_id)

        # --- Cleanup: expire old pending calls (>5 min) to prevent memory leak ---
        now = time.time()
        expired = [cid for cid, v in self.pending_tool_calls.items()
                   if now - v.get("_ts", now) > 300]
        for cid in expired:
            self.pending_tool_calls.pop(cid, None)
        # Tag new entries with timestamp
        for tc in tool_calls:
            call_id = tc.get("call_id", "")
            if call_id in self.pending_tool_calls:
                self.pending_tool_calls[call_id]["_ts"] = now


addons = [CopilotInterceptor()]
