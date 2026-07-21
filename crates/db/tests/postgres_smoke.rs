//! Postgres integration smoke test — runs when `DATABASE_URL` points at PostgreSQL.

use agent_meter_db::models::InsertToolCall;
use agent_meter_db::{Database, PostgresDb};
use chrono::Utc;
use uuid::Uuid;

fn sample_event() -> InsertToolCall {
    InsertToolCall {
        event_id: Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: Some("ci-test".into()),
        agent: Some("postgres-smoke".into()),
        skill: None,
        mcp_server: None,
        tool_name: "postgres_smoke".into(),
        started_at: Utc::now(),
        ended_at: Utc::now(),
        duration_ms: 1,
        ok: true,
        error: None,
        request_bytes: None,
        response_bytes: None,
        estimated_input_tokens: Some(10),
        estimated_output_tokens: Some(5),
        estimated_total_tokens: Some(15),
        request_sha256: None,
        response_sha256: None,
        metadata: serde_json::json!({}),
        model: Some("gpt-4o-mini".into()),
        cached_tokens: None,
        conversation_id: Some(format!("pg-smoke-{}", Uuid::new_v4())),
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

#[tokio::test]
async fn postgres_migrate_insert_and_cost_summary() {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) if u.starts_with("postgres") => u,
        _ => {
            eprintln!("skip postgres_smoke: DATABASE_URL not set to postgres");
            return;
        }
    };

    let db = PostgresDb::connect(&url).await.expect("postgres connect");
    db.migrate().await.expect("postgres migrate");
    db.health_check().await.expect("postgres health");

    let row = db
        .insert_tool_call(&sample_event())
        .await
        .expect("insert tool call");
    assert!(!row.event_id.is_nil());

    let summary = db
        .cost_summary(&agent_meter_db::params::CostQuery {
            from: Utc::now() - chrono::Duration::days(1),
            to: Utc::now() + chrono::Duration::seconds(1),
            model: None,
        })
        .await
        .expect("cost summary");
    assert!(summary.kpis.total_events >= 1);
}
