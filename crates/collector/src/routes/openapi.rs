use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

fn build_spec() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "agent-meter",
            "description": "AI agent observability and cost tracking collector API.",
            "version": env!("CARGO_PKG_VERSION"),
            "license": { "name": "MIT", "url": "https://opensource.org/licenses/MIT" }
        },
        "servers": [
            { "url": "http://127.0.0.1:8081", "description": "Default local UI + REST" },
            { "url": "http://127.0.0.1:4318", "description": "Dedicated OTLP receiver (optional)" }
        ],
        "tags": [
            { "name": "ingest", "description": "Event ingestion" },
            { "name": "query", "description": "Read APIs and reports" },
            { "name": "admin", "description": "Localhost-only maintenance" },
            { "name": "meta", "description": "Health and discovery" }
        ],
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Required when AGENT_METER_REQUIRE_API_KEY=1"
                }
            },
            "schemas": {
                "ToolCallEvent": {
                    "type": "object",
                    "required": ["tool_name", "started_at", "ended_at", "ok"],
                    "properties": {
                        "event_id": { "type": "string", "format": "uuid" },
                        "tool_name": { "type": "string" },
                        "conversation_id": { "type": "string" },
                        "model": { "type": "string" },
                        "agent": { "type": "string" },
                        "ide": { "type": "string" },
                        "started_at": { "type": "string", "format": "date-time" },
                        "ended_at": { "type": "string", "format": "date-time" },
                        "ok": { "type": "boolean" },
                        "estimated_input_tokens": { "type": "integer" },
                        "estimated_output_tokens": { "type": "integer" }
                    }
                }
            }
        },
        "paths": {
            "/health": {
                "get": {
                    "tags": ["meta"],
                    "summary": "Liveness probe",
                    "responses": { "200": { "description": "Service is up" } }
                }
            },
            "/health/ready": {
                "get": {
                    "tags": ["meta"],
                    "summary": "Readiness probe (DB connectivity)",
                    "responses": { "200": { "description": "Ready" }, "503": { "description": "Not ready" } }
                }
            },
            "/api/version": {
                "get": {
                    "tags": ["meta"],
                    "summary": "Collector version and feature flags",
                    "responses": { "200": { "description": "Version JSON" } }
                }
            },
            "/openapi.json": {
                "get": {
                    "tags": ["meta"],
                    "summary": "This OpenAPI document",
                    "responses": { "200": { "description": "OpenAPI 3.1 JSON" } }
                }
            },
            "/events/tool-call": {
                "post": {
                    "tags": ["ingest"],
                    "summary": "Ingest a tool-call event (REST)",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ToolCallEvent" }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Accepted (buffered)" },
                        "401": { "description": "Missing or invalid API key" },
                        "429": { "description": "Rate limit exceeded" },
                        "503": { "description": "Ingest buffer full" }
                    }
                }
            },
            "/v1/traces": {
                "post": {
                    "tags": ["ingest"],
                    "summary": "OTLP HTTP trace ingest (JSON or protobuf)",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": { "description": "Spans buffered" },
                        "401": { "description": "Unauthorized" },
                        "429": { "description": "Rate limit exceeded" },
                        "503": { "description": "Ingest buffer full" }
                    }
                }
            },
            "/api/conversations": {
                "get": {
                    "tags": ["query"],
                    "summary": "List conversations (paginated)",
                    "parameters": [
                        { "name": "limit", "in": "query", "schema": { "type": "integer" } },
                        { "name": "offset", "in": "query", "schema": { "type": "integer" } },
                        { "name": "ide", "in": "query", "schema": { "type": "string" } }
                    ],
                    "responses": { "200": { "description": "Conversation array" } }
                }
            },
            "/api/conversations/{conversation_id}/timeline": {
                "get": {
                    "tags": ["query"],
                    "summary": "Conversation timeline with summary",
                    "parameters": [
                        { "name": "conversation_id", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": { "200": { "description": "Timeline JSON" } }
                }
            },
            "/api/cost/summary": {
                "get": {
                    "tags": ["query"],
                    "summary": "Token and USD cost summary",
                    "responses": { "200": { "description": "Cost KPIs and breakdowns" } }
                }
            },
            "/reports/top-tools": {
                "get": {
                    "tags": ["query"],
                    "summary": "Most-used tools",
                    "responses": { "200": { "description": "Report rows" } }
                }
            },
            "/reports/events": {
                "get": {
                    "tags": ["query"],
                    "summary": "Paginated event feed with filters",
                    "responses": { "200": { "description": "Event rows" } }
                }
            },
            "/api/conversations/{conversation_id}": {
                "delete": {
                    "tags": ["admin"],
                    "summary": "Delete conversation and events (localhost only)",
                    "parameters": [
                        { "name": "conversation_id", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "description": "Deleted" },
                        "403": { "description": "Non-loopback client" }
                    }
                }
            },
            "/api/admin/reset": {
                "post": {
                    "tags": ["admin"],
                    "summary": "Wipe all ingested events (localhost only)",
                    "responses": {
                        "200": { "description": "Reset complete" },
                        "403": { "description": "Non-loopback client" }
                    }
                }
            },
            "/badge/cost.svg": {
                "get": {
                    "tags": ["query"],
                    "summary": "Embeddable cost badge (SVG)",
                    "responses": { "200": { "description": "image/svg+xml" } }
                }
            },
            "/badge/events.svg": {
                "get": {
                    "tags": ["query"],
                    "summary": "Embeddable events badge (SVG)",
                    "responses": { "200": { "description": "image/svg+xml" } }
                }
            }
        }
    })
}

async fn spec_handler() -> Json<Value> {
    Json(build_spec())
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new().route("/openapi.json", get(spec_handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_has_core_paths() {
        let spec = build_spec();
        let paths = spec["paths"].as_object().expect("paths object");
        for path in [
            "/events/tool-call",
            "/v1/traces",
            "/api/conversations",
            "/health",
        ] {
            assert!(paths.contains_key(path), "missing path {path}");
        }
        assert_eq!(spec["openapi"], "3.1.0");
    }
}
