//! Synthetic data generator for showcases (`agent-meter demo`).
//!
//! Produces deterministic, realistic-looking tool-call activity across several
//! conversations, agents, models and tools so the dashboard, conversations and
//! reports have something to show without collecting any real telemetry.

use std::sync::Arc;

use agent_meter_db::models::InsertToolCall;
use agent_meter_db::params::ConversationQuery;
use agent_meter_db::Database;
use chrono::{Duration, Utc};
use uuid::Uuid;

/// (agent, ide) pairs cycled across the synthetic conversations.
const AGENTS: &[(&str, &str)] = &[
    ("cursor", "cursor"),
    ("copilot", "vscode"),
    ("claude-code", "cli"),
    ("codex-cli", "cli"),
    ("antigravity", "antigravity"),
    ("aider", "cli"),
];
const MODELS: &[&str] = &[
    "gpt-4o",
    "claude-sonnet-4",
    "gemini-2.5-pro",
    "o3-mini",
    "gpt-4o-mini",
    "claude-opus-4",
];
const SERVERS: &[&str] = &[
    "filesystem",
    "git",
    "chromeDevtools",
    "playwright",
    "fetch",
    "vscode-builtin",
];
const TOOLS: &[&str] = &[
    "read_file",
    "grep_search",
    "run_in_terminal",
    "edit_file",
    "list_dir",
    "semantic_search",
    "fetch_webpage",
    "create_file",
];
const PROMPTS: &[&str] = &[
    "Refactor the billing module and remove tight coupling",
    "Investigate a pod stuck in CrashLoopBackOff",
    "Add tests for the OTLP trace parser",
    "Migrate the dashboard to the new design system",
    "Profile the slow report endpoint and optimize it",
    "Wire up the SQLite backend behind the Database trait",
];

/// Minimal deterministic PRNG (LCG) — keeps demo output reproducible without
/// pulling in the `rand` crate.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 16
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo).max(1) as u64) as i64
    }
}

fn blank(tool_name: String) -> InsertToolCall {
    let now = Utc::now();
    InsertToolCall {
        event_id: Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: None,
        agent: None,
        skill: None,
        mcp_server: None,
        tool_name,
        started_at: now,
        ended_at: now,
        duration_ms: 0,
        ok: true,
        error: None,
        request_bytes: None,
        response_bytes: None,
        estimated_input_tokens: None,
        estimated_output_tokens: None,
        estimated_total_tokens: None,
        request_sha256: None,
        response_sha256: None,
        metadata: serde_json::Value::Null,
        model: None,
        cached_tokens: None,
        conversation_id: None,
        client_ip: None,
        user_agent: None,
        user_prompt: None,
        tool_arguments: None,
        tool_result: None,
        reasoning_tokens: None,
        finish_reason: None,
        request_max_tokens: None,
        request_temperature: None,
        llm_system: None,
        trace_id: None,
        span_id: None,
        parent_span_id: None,
        tool_call_id: None,
    }
}

/// Returns true if the database already has any conversations (so `demo`
/// doesn't pile up duplicate synthetic data on every restart).
pub async fn has_data(db: &Arc<dyn Database>) -> bool {
    let q = ConversationQuery {
        limit: 1,
        offset: 0,
        ide: None,
    };
    db.list_conversations(&q)
        .await
        .map(|rows| !rows.is_empty())
        .unwrap_or(false)
}

/// Seed `conversations` synthetic conversations, each with `events_per`
/// tool-call events. Returns the number of events inserted.
pub async fn seed(
    db: &Arc<dyn Database>,
    conversations: usize,
    events_per: usize,
) -> anyhow::Result<usize> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let now = Utc::now();
    let mut total = 0usize;

    for c in 0..conversations {
        let (agent, ide) = AGENTS[c % AGENTS.len()];
        let model = MODELS[c % MODELS.len()];
        let conv_id = format!("demo-conv-{:02}", c + 1);
        let prompt = PROMPTS[c % PROMPTS.len()];

        // start each conversation somewhere in the last ~4 days
        let mut cursor = now - Duration::minutes(rng.range(30, 5760));

        for _ in 0..events_per {
            let tool = TOOLS[rng.below(TOOLS.len())];
            let server = SERVERS[rng.below(SERVERS.len())];
            let dur = rng.range(120, 5200);
            let started = cursor + Duration::seconds(rng.range(2, 90));
            let ended = started + Duration::milliseconds(dur);
            cursor = ended;

            let ok = rng.below(15) != 0;
            let in_tok = rng.range(200, 4000) as i32;
            let out_tok = rng.range(50, 1500) as i32;

            let mut ev = blank(tool.to_string());
            ev.agent = Some(agent.to_string());
            ev.ide = Some(ide.to_string());
            ev.model = Some(model.to_string());
            ev.mcp_server = Some(server.to_string());
            ev.conversation_id = Some(conv_id.clone());
            ev.user_prompt = Some(prompt.to_string());
            ev.started_at = started;
            ev.ended_at = ended;
            ev.duration_ms = dur as i32;
            ev.ok = ok;
            ev.error = if ok {
                None
            } else {
                Some("tool call timed out".to_string())
            };
            ev.request_bytes = Some(rng.range(300, 4000) as i32);
            ev.response_bytes = Some(rng.range(800, 60000) as i32);
            ev.estimated_input_tokens = Some(in_tok);
            ev.estimated_output_tokens = Some(out_tok);
            ev.estimated_total_tokens = Some(in_tok + out_tok);
            ev.finish_reason = Some(if ok { "stop" } else { "error" }.to_string());

            db.insert_tool_call(&ev)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            total += 1;
        }
    }

    Ok(total)
}
