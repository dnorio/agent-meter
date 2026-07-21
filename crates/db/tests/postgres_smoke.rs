//! Postgres integration smoke tests — runs when `DATABASE_URL` points at PostgreSQL.

use agent_meter_db::models::InsertToolCall;
use agent_meter_db::params::{ConversationQuery, EventQuery, ReportQuery};
use agent_meter_db::{Database, PostgresDb};
use chrono::Utc;
use uuid::Uuid;

fn sample_event(tool: &str, conversation_id: &str) -> InsertToolCall {
    InsertToolCall {
        event_id: Uuid::new_v4(),
        task_id: None,
        repo: Some("agent-meter".into()),
        branch: None,
        ide: Some("ci-test".into()),
        agent: Some("postgres-smoke".into()),
        skill: None,
        mcp_server: None,
        tool_name: tool.into(),
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
        conversation_id: Some(conversation_id.into()),
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

async fn postgres_db() -> Option<PostgresDb> {
    let url = std::env::var("DATABASE_URL").ok()?;
    if !url.starts_with("postgres") {
        eprintln!("skip postgres_smoke: DATABASE_URL not set to postgres");
        return None;
    }
    let db = PostgresDb::connect(&url).await.expect("postgres connect");
    db.migrate().await.expect("postgres migrate");
    Some(db)
}

#[tokio::test]
async fn postgres_migrate_insert_and_cost_summary() {
    let Some(db) = postgres_db().await else {
        return;
    };
    db.health_check().await.expect("postgres health");

    let row = db
        .insert_tool_call(&sample_event(
            "postgres_smoke",
            &format!("pg-smoke-{}", Uuid::new_v4()),
        ))
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

#[tokio::test]
async fn postgres_query_events_and_top_tools() {
    let Some(db) = postgres_db().await else {
        return;
    };

    let conv = format!("pg-query-{}", Uuid::new_v4());
    db.insert_tool_call(&sample_event("read_file", &conv))
        .await
        .expect("insert first");
    db.insert_tool_call(&sample_event("read_file", &conv))
        .await
        .expect("insert second");

    let rows = db
        .query_events(&EventQuery {
            conversation_id: Some(conv.clone()),
            limit: 10,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("query events");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].tool_name, "read_file");

    let tools = db
        .top_tools(&ReportQuery {
            limit: Some(10),
            ..Default::default()
        })
        .await
        .expect("top tools");
    assert!(tools
        .iter()
        .any(|t| t.tool_name == "read_file" && t.calls >= 2));
}

#[tokio::test]
async fn postgres_conversation_list_and_delete() {
    let Some(db) = postgres_db().await else {
        return;
    };

    let conv = format!("pg-delete-{}", Uuid::new_v4());
    db.insert_tool_call(&sample_event("grep", &conv))
        .await
        .expect("insert");

    let conversations = db
        .list_conversations(&ConversationQuery {
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("list conversations");
    assert!(conversations.iter().any(|c| c.conversation_id == conv));

    let detail = db
        .conversation_detail(&conv)
        .await
        .expect("conversation detail");
    assert_eq!(detail.len(), 1);

    let removed = db
        .delete_conversation(&conv)
        .await
        .expect("delete conversation");
    assert_eq!(removed, 1);

    let after = db
        .list_conversations(&ConversationQuery {
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("list after delete");
    assert!(!after.iter().any(|c| c.conversation_id == conv));
}
