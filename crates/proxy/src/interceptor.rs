use std::collections::HashMap;
use std::sync::Mutex;

use http::{header, Request, Response};
use http_body_util::BodyExt;
use hudsucker::{Body, RequestOrResponse};
use serde_json::{json, Value};
use tracing::{debug, info, warn};

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
    // Google / Antigravity / Gemini
    "generativelanguage.googleapis.com",
    "aiplatform.googleapis.com",
    // OpenAI-compatible gateways used by OpenCode / agents
    "openrouter.ai",
    "api.deepseek.com",
    "api.groq.com",
    "api.mistral.ai",
    "api.fireworks.ai",
    "api.x.ai",
    "api.together.xyz",
    "api.perplexity.ai",
];

/// Paths that indicate an LLM call.
const LLM_PATHS: &[&str] = &[
    "/v1/messages",
    "/v1/chat/completions",
    "/chat/completions",
    "/v1/engines/",
    "/completions",
    "/responses",
    "/v1beta/models",
    "/openai/deployments",
    "/v1/responses",
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
    /// Original client User-Agent — forwarded on OTLP so collector can infer_ide.
    user_agent: String,
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

        let session_id = extract_session_id(&req);
        let user_agent = extract_user_agent(&req);

        let (parts, body) = req.into_parts();
        let collected = match body.collect().await {
            Ok(c) => c,
            Err(_) => {
                return RequestOrResponse::Request(Request::from_parts(parts, Body::empty()));
            }
        };
        let body_bytes = collected.to_bytes();
        let request_bytes = body_bytes.len();
        let (model, user_prompt) = extract_request_meta(&body_bytes, &path);

        let req_id = format!("{}:{}", parts.method, parts.uri);
        debug!(
            "[proxy] → {} {} model={:?} prompt={:?}",
            parts.method,
            parts.uri,
            model,
            user_prompt
                .as_deref()
                .map(|s| s.chars().take(80).collect::<String>())
        );

        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(
                req_id,
                PendingRequest {
                    started_ns: otlp::now_ns(),
                    model,
                    user_prompt,
                    session_id,
                    host,
                    path,
                    request_bytes,
                    user_agent,
                },
            );
        }

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
        let service_name = detect_service_name(&pending.host, &pending.user_agent);
        let trace_id = self.sessions.trace_id_for(&pending.session_id);
        let system = detect_system(&pending.host);
        let client_ua = pending.user_agent.clone();

        info!(
            "[proxy] ← {} {}ms model={} in={} out={} cached={} tools={}",
            status_code,
            duration_ms,
            model,
            input_tokens,
            output_tokens,
            cached_tokens,
            tool_calls.len()
        );

        spawn_otlp_export(
            self.http_client.clone(),
            &self.collector_url,
            CaptureExport {
                service_name: &service_name,
                model: &model,
                system: &system,
                trace_id: &trace_id,
                session_id: &pending.session_id,
                started_ns: pending.started_ns,
                ended_ns,
                status_code,
                request_bytes: pending.request_bytes,
                response_bytes,
                input_tokens,
                output_tokens,
                cached_tokens,
                user_prompt: pending.user_prompt.as_deref(),
                finish_reason: &finish_reason,
                tool_calls: &tool_calls,
                client_ua: &client_ua,
            },
        );

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
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
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

fn extract_user_agent<T>(req: &Request<T>) -> String {
    req.headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn extract_request_meta(body_bytes: &[u8], path: &str) -> (Option<String>, Option<String>) {
    let mut model = None;
    let mut user_prompt = None;
    if let Ok(body_json) = serde_json::from_slice::<Value>(body_bytes) {
        model = body_json
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        user_prompt = extract_user_prompt_from_body(&body_json);
    }
    if model.is_none() {
        model = extract_model_from_path(path);
    }
    (model, user_prompt)
}

fn extract_user_prompt_from_body(body_json: &Value) -> Option<String> {
    if let Some(messages) = body_json.get("messages").and_then(|v| v.as_array()) {
        if let Some(p) = extract_user_prompt_from_messages(messages) {
            return Some(p);
        }
    }
    if let Some(input_str) = body_json.get("input").and_then(|v| v.as_str()) {
        let cleaned = clean_prompt(input_str);
        if !cleaned.is_empty() && !is_noise_content(&cleaned) {
            return Some(cleaned);
        }
    } else if let Some(input_arr) = body_json.get("input").and_then(|v| v.as_array()) {
        if let Some(p) = extract_user_prompt_from_messages(input_arr) {
            return Some(p);
        }
    }
    extract_user_prompt_from_gemini_contents(body_json)
}

/// Prefer client User-Agent, then host heuristics. Table-driven to keep cognitive complexity low.
fn detect_service_name(host: &str, user_agent: &str) -> String {
    if let Some(name) = service_from_user_agent(user_agent) {
        return name.to_string();
    }
    service_from_host(host).to_string()
}

fn service_from_user_agent(user_agent: &str) -> Option<&'static str> {
    let ua = user_agent.to_lowercase();
    const RULES: &[(&[&str], &str)] = &[
        (
            &["copilot-jetbrains", "github-copilot-jetbrains"],
            "copilot-jetbrains",
        ),
        (
            &["copilot-cli", "copilot_cli", "github-copilot-cli"],
            "copilot-cli",
        ),
        (&["opencode"], "opencode"),
        (&["codex"], "codex"),
        (&["antigravity"], "antigravity"),
        (&["claude-code", "claude_code"], "claude-code"),
        (&["rust-rover", "rustrover"], "rust-rover"),
        (&["windsurf", "codeium"], "windsurf"),
        (&["gemini-cli", "gemini_cli"], "gemini-cli"),
        (&["cursor"], "cursor"),
        (&["vscode"], "copilot"),
        (&["eclipse", "jdt"], "copilot-eclipse"),
    ];
    for (needles, name) in RULES {
        if needles.iter().any(|n| ua.contains(n)) {
            return Some(name);
        }
    }
    // IntelliJ + GitHub Copilot plugin UA → copilot-jetbrains (before generic jetbrains).
    if is_jetbrains_ua(&ua)
        && (ua.contains("githubcopilot") || ua.contains("github-copilot") || ua.contains("copilot"))
    {
        return Some("copilot-jetbrains");
    }
    if is_jetbrains_ua(&ua) {
        return Some("jetbrains");
    }
    None
}

fn is_jetbrains_ua(ua: &str) -> bool {
    const IDES: &[&str] = &["intellij", "pycharm", "webstorm", "goland", "phpstorm"];
    if IDES.iter().any(|n| ua.contains(n)) {
        return true;
    }
    ua.contains("jetbrains") && !ua.contains("rust-rover") && !ua.contains("rustrover")
}

fn service_from_host(host: &str) -> &'static str {
    let host = host.to_lowercase();
    if host.contains("cursor") {
        "cursor"
    } else if host.contains("anthropic") {
        "claude-code"
    } else {
        "copilot"
    }
}

fn detect_system(host: &str) -> String {
    let host = host.to_lowercase();
    const RULES: &[(&[&str], &str)] = &[
        (&["anthropic"], "anthropic"),
        (
            &["generativelanguage.googleapis", "aiplatform.googleapis"],
            "google",
        ),
        (&["openrouter"], "openrouter"),
        (&["deepseek"], "deepseek"),
        (&["groq"], "groq"),
        (&["mistral"], "mistral"),
        (&["fireworks"], "fireworks"),
        (&["x.ai", "xai"], "xai"),
        (&["together"], "together"),
        (&["perplexity"], "perplexity"),
        (
            &["githubcopilot", "githubusercontent.com"],
            "github-copilot",
        ),
    ];
    for (needles, name) in RULES {
        if needles.iter().any(|n| host.contains(n)) {
            return name.to_string();
        }
    }
    "openai".to_string()
}

fn extract_model_from_path(path: &str) -> Option<String> {
    if let Some(rest) = path.strip_prefix("/v1beta/models/") {
        let model = rest.split(':').next().unwrap_or("").trim();
        if !model.is_empty() {
            return Some(model.to_string());
        }
    }
    const DEPLOY: &str = "/openai/deployments/";
    if let Some(idx) = path.find(DEPLOY) {
        let rest = &path[idx + DEPLOY.len()..];
        let model = rest.split('/').next().unwrap_or("").trim();
        if !model.is_empty() {
            return Some(model.to_string());
        }
    }
    None
}

fn extract_user_prompt_from_gemini_contents(body: &Value) -> Option<String> {
    let contents = body.get("contents")?.as_array()?;
    for content in contents.iter().rev() {
        let parts = content.get("parts")?.as_array()?;
        for part in parts.iter().rev() {
            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                let cleaned = clean_prompt(text);
                if !cleaned.is_empty() && !is_noise_content(&cleaned) {
                    return Some(cleaned);
                }
            }
        }
    }
    None
}

struct CaptureExport<'a> {
    service_name: &'a str,
    model: &'a str,
    system: &'a str,
    trace_id: &'a str,
    session_id: &'a str,
    started_ns: i64,
    ended_ns: i64,
    status_code: u16,
    request_bytes: usize,
    response_bytes: usize,
    input_tokens: i64,
    output_tokens: i64,
    cached_tokens: i64,
    user_prompt: Option<&'a str>,
    finish_reason: &'a str,
    tool_calls: &'a [String],
    client_ua: &'a str,
}

fn spawn_otlp_export(client: reqwest::Client, collector_url: &str, cap: CaptureExport<'_>) {
    let span_name = format!("chat {}", cap.model);
    let mut attrs: Vec<(&str, Value)> = vec![
        ("gen_ai.request.model", json!(cap.model)),
        ("gen_ai.response.model", json!(cap.model)),
        ("gen_ai.system", json!(cap.system)),
        ("gen_ai.usage.input_tokens", json!(cap.input_tokens)),
        ("gen_ai.usage.output_tokens", json!(cap.output_tokens)),
        ("gen_ai.conversation.id", json!(cap.session_id)),
        ("http.status_code", json!(cap.status_code)),
        ("gen_ai.request.bytes", json!(cap.request_bytes)),
        ("gen_ai.response.bytes", json!(cap.response_bytes)),
    ];
    if cap.cached_tokens > 0 {
        attrs.push(("gen_ai.usage.cached_tokens", json!(cap.cached_tokens)));
    }
    if let Some(prompt) = cap.user_prompt {
        attrs.push(("gen_ai.prompt", json!(prompt)));
    }
    if !cap.finish_reason.is_empty() {
        attrs.push(("gen_ai.finish_reason", json!(cap.finish_reason)));
    }

    let ua = Some(cap.client_ua).filter(|s| !s.is_empty());
    let payload = otlp::build_otlp_payload(
        cap.service_name,
        &span_name,
        cap.trace_id,
        cap.started_ns,
        cap.ended_ns,
        attrs,
        ua,
        Some(cap.status_code),
    );

    let mut bodies = vec![payload];
    for tc in cap.tool_calls {
        bodies.push(otlp::build_otlp_payload(
            cap.service_name,
            &format!("execute_tool {tc}"),
            cap.trace_id,
            cap.started_ns,
            cap.ended_ns,
            vec![
                ("gen_ai.tool.name", json!(tc)),
                ("gen_ai.conversation.id", json!(cap.session_id)),
            ],
            ua,
            Some(cap.status_code),
        ));
    }

    let url = format!("{}/v1/traces", collector_url);
    let otlp_ua = if cap.client_ua.is_empty() {
        format!("agent-meter-proxy/{}", env!("CARGO_PKG_VERSION"))
    } else {
        cap.client_ua.to_string()
    };
    tokio::spawn(async move {
        for body in bodies {
            match client
                .post(&url)
                .header(header::USER_AGENT, otlp_ua.as_str())
                .json(&body)
                .send()
                .await
            {
                Ok(resp) if !resp.status().is_success() => {
                    warn!("[proxy] OTLP export HTTP {}", resp.status());
                }
                Err(e) => warn!("[proxy] Failed to send OTLP span: {e}"),
                _ => {}
            }
        }
    });
}

fn clean_prompt(content: &str) -> String {
    let mut s = content.to_string();

    // Strip common XML wrappers
    for tag in &[
        "attachments",
        "workspace_info",
        "environment_info",
        "skill-context",
        "context",
        "repoMemory",
        "sessionMemory",
        "userMemory",
        "securityRequirements",
        "operationalSafety",
        "implementationDiscipline",
        "communicationStyle",
        "toolUseInstructions",
        "outputFormatting",
        "memoryInstructions",
        "reminderInstructions",
        "editorContext",
        "notebookInstructions",
        "instructions",
        "conversation-summary",
        "workspace_info",
        "availableDeferredTools",
        "parallelizationStrategy",
        "taskTracking",
        "current_datetime",
        "copilot_instructions",
        "copilotInstructions",
        "fileLinkification",
        "communicationExamples",
        "toolSearchInstructions",
        "memoryScopes",
        "memoryGuidelines",
        "system_reminder",
        "sql_tables",
        "active_selection",
        "file_context",
        "reference_data",
    ] {
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
        if msg_type == "message" && role != "user" {
            continue;
        }
        if msg_type != "message" && role != "user" {
            continue;
        }

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
            let first_type = blocks
                .first()
                .and_then(|b| b.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if first_type == "tool_result" || first_type == "function_call_output" {
                continue;
            }

            for block in blocks {
                let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if btype == "text" || btype == "input_text" {
                    let text_field = block
                        .get("text")
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
                if part.get("type").and_then(|t| t.as_str()) != Some("text") {
                    continue;
                }
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
        || t.to_ascii_lowercase()
            .starts_with("summarize the following")
        || t.to_ascii_lowercase()
            .starts_with("please write a brief title")
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
        *input_tokens = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        *output_tokens = usage
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Anthropic format
        if *input_tokens == 0 {
            *input_tokens = usage
                .get("input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        }
        if *output_tokens == 0 {
            *output_tokens = usage
                .get("output_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
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
            if let Some(tcs) = choice
                .pointer("/message/tool_calls")
                .and_then(|v| v.as_array())
            {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_service_name_prefers_user_agent() {
        assert_eq!(
            detect_service_name("api.openai.com", "codex/0.1.0"),
            "codex"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "opencode/0.5.0"),
            "opencode"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "github-copilot-cli/1.0"),
            "copilot-cli"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "Mozilla/5.0 cursor/0.48"),
            "cursor"
        );
        assert_eq!(
            detect_service_name("api.anthropic.com", "claude-code/1.0"),
            "claude-code"
        );
        assert_eq!(detect_service_name("api2.cursor.sh", "something"), "cursor");
        assert_eq!(
            detect_service_name("api.openai.com", "vscode/1.100"),
            "copilot"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "rust-rover/2025.1"),
            "rust-rover"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "eclipse/2026-03 jdt"),
            "copilot-eclipse"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "Windsurf/1.2.0"),
            "windsurf"
        );
        assert_eq!(
            detect_service_name("api.openai.com", "IntelliJ IDEA/2025.1"),
            "jetbrains"
        );
        assert_eq!(
            detect_service_name(
                "api.githubcopilot.com",
                "IntelliJ IDEA/2025.1 GitHubCopilot/1.5.0"
            ),
            "copilot-jetbrains"
        );
        assert_eq!(
            detect_service_name("generativelanguage.googleapis.com", "gemini-cli/0.1.0"),
            "gemini-cli"
        );
    }

    #[test]
    fn extract_model_from_gemini_and_azure_paths() {
        assert_eq!(
            extract_model_from_path("/v1beta/models/gemini-2.0-flash:generateContent"),
            Some("gemini-2.0-flash".into())
        );
        assert_eq!(
            extract_model_from_path("/openai/deployments/gpt-4o/chat/completions"),
            Some("gpt-4o".into())
        );
    }

    #[test]
    fn detect_system_google_and_gateways() {
        assert_eq!(detect_system("generativelanguage.googleapis.com"), "google");
        assert_eq!(detect_system("openrouter.ai"), "openrouter");
        assert_eq!(detect_system("api.together.xyz"), "together");
        assert_eq!(detect_system("api.perplexity.ai"), "perplexity");
        assert_eq!(detect_system("api.openai.com"), "openai");
    }
}
