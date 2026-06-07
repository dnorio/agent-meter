use std::collections::HashMap;
use std::sync::Mutex;

use http::{header, Request, Response};
use http_body_util::BodyExt;
use hudsucker::{Body, RequestOrResponse};
use serde_json::{json, Value};
use tracing::{info, warn, debug};

use crate::otlp;
use crate::session::SessionManager;

/// Hosts whose traffic we intercept and capture telemetry from.
const AI_HOSTS: &[&str] = &[
    "api.anthropic.com",
    "api.openai.com",
    "api.githubcopilot.com",
    "api.business.githubcopilot.com",
    "api.individual.githubcopilot.com",
    "copilot-proxy.githubusercontent.com",
    "cursor.sh",
    "api2.cursor.sh",
    "proxy.cursor.sh",
];

/// Paths that indicate an LLM call.
const LLM_PATHS: &[&str] = &[
    "/v1/messages",
    "/v1/chat/completions",
    "/chat/completions",
    "/v1/engines/",
    "/completions",
    "/responses",
];

pub struct InterceptorState {
    collector_url: String,
    http_client: reqwest::Client,
    sessions: SessionManager,
    /// Pending requests: map from (method, uri) to request metadata
    pending: Mutex<HashMap<String, PendingRequest>>,
}

struct PendingRequest {
    started_ns: i64,
    model: Option<String>,
    user_prompt: Option<String>,
    session_id: String,
    host: String,
    #[allow(dead_code)]
    path: String,
    request_bytes: usize,
}

impl InterceptorState {
    pub fn new(collector_url: String) -> Self {
        Self {
            collector_url,
            http_client: reqwest::Client::new(),
            sessions: SessionManager::new(),
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn collector_url(&self) -> &str {
        &self.collector_url
    }

    /// Process an outgoing request. We read the body for metadata but pass it through.
    pub async fn on_request(&self, req: Request<Body>) -> RequestOrResponse {
        let host = req.uri().host().unwrap_or("").to_string();

        if !is_ai_host(&host) {
            return RequestOrResponse::Request(req);
        }

        let path = req.uri().path().to_string();
        if !is_llm_path(&path) {
            return RequestOrResponse::Request(req);
        }

        // Extract session ID from headers
        let session_id = extract_session_id(&req);

        // We need to read the body to extract model/prompt, then reconstruct the request
        let (parts, body) = req.into_parts();
        let collected = match body.collect().await {
            Ok(c) => c,
            Err(_) => {
                return RequestOrResponse::Request(Request::from_parts(parts, Body::empty()));
            }
        };
        let body_bytes = collected.to_bytes();

        let request_bytes = body_bytes.len();
        let mut model = None;
        let mut user_prompt = None;

        if let Ok(body_json) = serde_json::from_slice::<Value>(&body_bytes) {
            model = body_json.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());

            // Extract user prompt — try messages (Chat/Messages API), then input (Responses API)
            if let Some(messages) = body_json.get("messages").and_then(|v| v.as_array()) {
                user_prompt = extract_user_prompt_from_messages(messages);
            }

            // Responses API: "input" can be a string or array of messages
            if user_prompt.is_none() {
                if let Some(input_str) = body_json.get("input").and_then(|v| v.as_str()) {
                    let cleaned = clean_prompt(input_str);
                    if !cleaned.is_empty() && !is_noise_content(&cleaned) {
                        user_prompt = Some(cleaned);
                    }
                } else if let Some(input_arr) = body_json.get("input").and_then(|v| v.as_array()) {
                    user_prompt = extract_user_prompt_from_messages(input_arr);
                }
            }
        }

        let req_id = format!("{}:{}", parts.method, parts.uri);
        debug!("[proxy] → {} {} model={:?} prompt={:?}", parts.method, parts.uri, model,
               user_prompt.as_deref().map(|s| s.chars().take(80).collect::<String>()));

        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(req_id, PendingRequest {
                started_ns: otlp::now_ns(),
                model,
                user_prompt,
                session_id,
                host,
                path,
                request_bytes,
            });
        }

        // Reconstruct request with original body
        let rebuilt = Request::from_parts(parts, Body::from(http_body_util::Full::new(body_bytes)));
        RequestOrResponse::Request(rebuilt)
    }

    /// Process the response. Extract tokens, model, build OTLP span, send to collector.
    pub async fn on_response(&self, res: Response<Body>) -> Response<Body> {
        // Try to find the pending request for this response
        // hudsucker doesn't give us the original URI in the response context,
        // so we use a simple approach: pop the most recent pending request for same status
        let pending_req = {
            let mut pending = self.pending.lock().unwrap();
            // Pop the oldest pending entry (FIFO)
            if pending.is_empty() {
                None
            } else {
                let key = pending.keys().next().unwrap().clone();
                pending.remove(&key)
            }
        };

        let pending = match pending_req {
            Some(p) => p,
            None => return res,
        };

        let status_code = res.status().as_u16();
        let ended_ns = otlp::now_ns();

        // Read the response body
        let (parts, body) = res.into_parts();
        let collected = match body.collect().await {
            Ok(c) => c,
            Err(_) => {
                return Response::from_parts(parts, Body::empty());
            }
        };
        let body_bytes = collected.to_bytes();

        let response_bytes = body_bytes.len();

        // Try to extract usage from response body
        let mut input_tokens: i64 = 0;
        let mut output_tokens: i64 = 0;
        let mut cached_tokens: i64 = 0;
        let mut response_model = pending.model.clone();
        let mut tool_calls: Vec<String> = vec![];
        let mut finish_reason = String::new();

        // Handle SSE streaming responses
        let body_str = String::from_utf8_lossy(&body_bytes);

        if body_str.starts_with("data: ") || body_str.contains("\ndata: ") {
            // SSE stream — find the last data chunk with usage
            parse_sse_usage(
                &body_str,
                &mut input_tokens,
                &mut output_tokens,
                &mut cached_tokens,
                &mut response_model,
                &mut tool_calls,
                &mut finish_reason,
            );
        } else if let Ok(body_json) = serde_json::from_slice::<Value>(&body_bytes) {
            // Regular JSON response
            extract_json_usage(
                &body_json,
                &mut input_tokens,
                &mut output_tokens,
                &mut cached_tokens,
                &mut response_model,
                &mut tool_calls,
                &mut finish_reason,
            );
        }

        let model = response_model.unwrap_or_else(|| "unknown".to_string());
        let duration_ms = (ended_ns - pending.started_ns) / 1_000_000;
        let service_name = detect_service_name(&pending.host);
        let trace_id = self.sessions.trace_id_for(&pending.session_id);
        let system = detect_system(&pending.host);

        info!(
            "[proxy] ← {} {}ms model={} in={} out={} cached={} tools={}",
            status_code, duration_ms, model, input_tokens, output_tokens, cached_tokens, tool_calls.len()
        );

        // Build OTLP span
        let span_name = format!("chat {model}");
        let mut attrs: Vec<(&str, Value)> = vec![
            ("gen_ai.request.model", json!(model)),
            ("gen_ai.response.model", json!(model)),
            ("gen_ai.system", json!(system)),
            ("gen_ai.usage.input_tokens", json!(input_tokens)),
            ("gen_ai.usage.output_tokens", json!(output_tokens)),
            ("gen_ai.conversation.id", json!(pending.session_id)),
            ("http.status_code", json!(status_code)),
            ("gen_ai.request.bytes", json!(pending.request_bytes)),
            ("gen_ai.response.bytes", json!(response_bytes)),
        ];

        if cached_tokens > 0 {
            attrs.push(("gen_ai.usage.cached_tokens", json!(cached_tokens)));
        }
        if let Some(ref prompt) = pending.user_prompt {
            attrs.push(("gen_ai.prompt", json!(prompt)));
        }
        if !finish_reason.is_empty() {
            attrs.push(("gen_ai.finish_reason", json!(finish_reason)));
        }

        let payload = otlp::build_otlp_payload(
            &service_name,
            &span_name,
            &trace_id,
            pending.started_ns,
            ended_ns,
            attrs,
        );

        // Build tool call child spans
        let mut tool_payloads = vec![];
        for tc in &tool_calls {
            let tool_span = otlp::build_otlp_payload(
                &service_name,
                &format!("execute_tool {tc}"),
                &trace_id,
                pending.started_ns,
                ended_ns,
                vec![
                    ("gen_ai.tool.name", json!(tc)),
                    ("gen_ai.conversation.id", json!(pending.session_id)),
                ],
            );
            tool_payloads.push(tool_span);
        }

        // Send to collector asynchronously
        let client = self.http_client.clone();
        let url = format!("{}/v1/traces", self.collector_url);
        tokio::spawn(async move {
            if let Err(e) = client.post(&url).json(&payload).send().await {
                warn!("[proxy] Failed to send OTLP span: {e}");
            }
            for tp in tool_payloads {
                if let Err(e) = client.post(&url).json(&tp).send().await {
                    warn!("[proxy] Failed to send tool span: {e}");
                }
            }
        });

        // Reconstruct response with original body
        Response::from_parts(parts, Body::from(http_body_util::Full::new(body_bytes)))
    }
}

fn is_ai_host(host: &str) -> bool {
    AI_HOSTS.iter().any(|h| host.contains(h))
}

fn is_llm_path(path: &str) -> bool {
    LLM_PATHS.iter().any(|p| path.contains(p))
}

fn extract_session_id<T>(req: &Request<T>) -> String {
    let headers = req.headers();

    // Try explicit session headers
    for header_name in &["x-session-id", "x-cursor-session", "vscode-sessionid"] {
        if let Some(val) = headers.get(*header_name).and_then(|v| v.to_str().ok()) {
            if !val.is_empty() {
                return val.to_string();
            }
        }
    }

    // Fallback: Bearer token prefix (first 16 chars — stable per login)
    if let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if auth.len() > 23 {
            let prefix = &auth[7..23]; // skip "Bearer "
            return format!("token-{prefix}");
        }
    }

    // Fallback: x-request-id or generate
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

fn detect_service_name(host: &str) -> String {
    if host.contains("cursor") {
        "cursor".to_string()
    } else if host.contains("anthropic") {
        // Could be claude-code or cursor; we detect from user-agent downstream
        "claude-code".to_string()
    } else {
        "copilot".to_string()
    }
}

fn detect_system(host: &str) -> String {
    if host.contains("anthropic") {
        "anthropic".to_string()
    } else {
        "openai".to_string()
    }
}

fn clean_prompt(content: &str) -> String {
    let mut s = content.to_string();

    // Strip common XML wrappers
    for tag in &["attachments", "workspace_info", "environment_info", "skill-context", "context",
                 "repoMemory", "sessionMemory", "userMemory", "securityRequirements",
                 "operationalSafety", "implementationDiscipline", "communicationStyle",
                 "toolUseInstructions", "outputFormatting", "memoryInstructions",
                 "reminderInstructions", "editorContext", "notebookInstructions",
                 "instructions", "conversation-summary", "workspace_info",
                 "availableDeferredTools", "parallelizationStrategy", "taskTracking",
                 "current_datetime", "copilot_instructions", "copilotInstructions",
                 "fileLinkification", "communicationExamples", "toolSearchInstructions",
                 "memoryScopes", "memoryGuidelines", "system_reminder", "sql_tables",
                 "active_selection", "file_context", "reference_data"] {
        let open = format!("<{tag}");
        // Match both <tag> and <tag ...attrs>
        if let Some(start) = s.find(&open) {
            let close = format!("</{tag}>");
            if let Some(end) = s.find(&close) {
                s = format!("{}{}", &s[..start], &s[end + close.len()..]);
            }
        }
    }

    // Extract <userRequest> if present (but not if mentioned in docs)
    if let Some(start) = s.find("<userRequest>") {
        if let Some(end) = s.find("</userRequest>") {
            let extracted = &s[start + 13..end];
            if !extracted.trim().is_empty() {
                return extracted.trim().chars().take(500).collect();
            }
        }
    }

    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.len() > 2000 {
        return String::new();
    }
    trimmed.chars().take(500).collect()
}

/// Extracts the actual user-typed prompt from a messages array.
/// Handles OpenAI Chat, Anthropic Messages, and Responses API formats.
/// Searches forward for the first user message with real content (not tool_result, not context-only).
fn extract_user_prompt_from_messages(messages: &[Value]) -> Option<String> {
    for msg in messages.iter() {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        // Responses API: input items may have type="message" wrapping role+content
        let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if msg_type == "message" && role != "user" { continue; }
        if msg_type != "message" && role != "user" { continue; }

        // Format 1: content is a plain string (OpenAI style)
        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
            let cleaned = clean_prompt(text);
            if !cleaned.is_empty() && !is_noise_content(&cleaned) {
                return Some(cleaned);
            }
            continue;
        }

        // Format 2: content is an array of blocks (Anthropic style / Responses API)
        if let Some(blocks) = msg.get("content").and_then(|c| c.as_array()) {
            // Skip if first block is tool_result (agentic loop turn)
            let first_type = blocks.first()
                .and_then(|b| b.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if first_type == "tool_result" || first_type == "function_call_output" {
                continue;
            }

            for block in blocks {
                let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if btype == "text" || btype == "input_text" {
                    let text_field = block.get("text")
                        .or_else(|| block.get("content"))
                        .and_then(|t| t.as_str());
                    if let Some(text) = text_field {
                        let cleaned = clean_prompt(text);
                        if !cleaned.is_empty() && !is_noise_content(&cleaned) {
                            return Some(cleaned);
                        }
                    }
                }
            }
        }

        // Format 3: parts array (Copilot/Gemini format)
        if let Some(parts) = msg.get("parts").and_then(|p| p.as_array()) {
            // Iterate in reverse — last non-XML part is typically the user prompt
            for part in parts.iter().rev() {
                if part.get("type").and_then(|t| t.as_str()) != Some("text") { continue; }
                if let Some(content) = part.get("content").and_then(|c| c.as_str()) {
                    let cleaned = clean_prompt(content);
                    if !cleaned.is_empty() && !is_noise_content(&cleaned) {
                        return Some(cleaned);
                    }
                }
            }
        }
    }
    None
}

fn is_noise_content(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('[')
        || t.starts_with('{')
        || t.starts_with("The current date")
        || t.starts_with("Terminals:")
        || t.starts_with("[Terminal")
        || t.starts_with("You are ")
        || t.to_ascii_lowercase().starts_with("summarize the following")
        || t.to_ascii_lowercase().starts_with("please write a brief title")
}

fn extract_json_usage(
    body: &Value,
    input_tokens: &mut i64,
    output_tokens: &mut i64,
    cached_tokens: &mut i64,
    model: &mut Option<String>,
    tool_calls: &mut Vec<String>,
    finish_reason: &mut String,
) {
    // Model from response
    if let Some(m) = body.get("model").and_then(|v| v.as_str()) {
        *model = Some(m.to_string());
    }

    // Usage (OpenAI format)
    if let Some(usage) = body.get("usage") {
        *input_tokens = usage.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
        *output_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);

        // Anthropic format
        if *input_tokens == 0 {
            *input_tokens = usage.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
        }
        if *output_tokens == 0 {
            *output_tokens = usage.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
        }
        *cached_tokens = usage
            .get("cache_read_input_tokens")
            .or_else(|| usage.pointer("/prompt_tokens_details/cached_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
    }

    // Tool calls (OpenAI format)
    if let Some(choices) = body.get("choices").and_then(|v| v.as_array()) {
        if let Some(choice) = choices.first() {
            if let Some(tcs) = choice.pointer("/message/tool_calls").and_then(|v| v.as_array()) {
                for tc in tcs {
                    if let Some(name) = tc.pointer("/function/name").and_then(|v| v.as_str()) {
                        tool_calls.push(name.to_string());
                    }
                }
            }
            if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                *finish_reason = fr.to_string();
            }
        }
    }

    // Tool calls (Anthropic format)
    if let Some(content) = body.get("content").and_then(|v| v.as_array()) {
        for item in content {
            if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    tool_calls.push(name.to_string());
                }
            }
        }
    }
    if let Some(sr) = body.get("stop_reason").and_then(|v| v.as_str()) {
        *finish_reason = sr.to_string();
    }
}

fn parse_sse_usage(
    body_str: &str,
    input_tokens: &mut i64,
    output_tokens: &mut i64,
    cached_tokens: &mut i64,
    model: &mut Option<String>,
    tool_calls: &mut Vec<String>,
    finish_reason: &mut String,
) {
    // Walk lines in reverse to find the last chunk with usage data
    for line in body_str.lines().rev() {
        let line = line.trim();
        if !line.starts_with("data: ") {
            continue;
        }
        let data = &line[6..];
        if data == "[DONE]" {
            continue;
        }

        if let Ok(chunk) = serde_json::from_str::<Value>(data) {
            // Check for usage in this chunk
            if chunk.get("usage").is_some() {
                extract_json_usage(
                    &chunk,
                    input_tokens,
                    output_tokens,
                    cached_tokens,
                    model,
                    tool_calls,
                    finish_reason,
                );
                if *input_tokens > 0 || *output_tokens > 0 {
                    return;
                }
            }

            // Responses API: event: response.completed
            if let Some(resp) = chunk.get("response") {
                if resp.get("usage").is_some() {
                    extract_json_usage(
                        resp,
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        model,
                        tool_calls,
                        finish_reason,
                    );
                    if *input_tokens > 0 || *output_tokens > 0 {
                        return;
                    }
                }
            }
        }
    }
}
