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
                },
                "Error": {
                    "type": "object",
                    "required": ["error", "code"],
                    "description": "Standard error body returned by every handler in `AppError::into_response`",
                    "properties": {
                        "error": {
                            "type": "string",
                            "description": "Human-readable error message"
                        },
                        "code": {
                            "type": "integer",
                            "description": "HTTP status code of the response"
                        }
                    }
                }
            },
            "parameters": {
                "From": {
                    "name": "from", "in": "query", "required": false,
                    "description": "Inclusive RFC 3339 lower bound on started_at",
                    "schema": { "type": "string", "format": "date-time" }
                },
                "To": {
                    "name": "to", "in": "query", "required": false,
                    "description": "Inclusive RFC 3339 upper bound on started_at",
                    "schema": { "type": "string", "format": "date-time" }
                },
                "Repo": {
                    "name": "repo", "in": "query", "required": false,
                    "description": "Only rows recorded for this repository",
                    "schema": { "type": "string" }
                },
                "Ide": {
                    "name": "ide", "in": "query", "required": false,
                    "description": "Only rows recorded by this IDE",
                    "schema": { "type": "string" }
                },
                "Agent": {
                    "name": "agent", "in": "query", "required": false,
                    "description": "Only rows recorded by this agent",
                    "schema": { "type": "string" }
                },
                "Model": {
                    "name": "model", "in": "query", "required": false,
                    "description": "Only rows recorded for this model",
                    "schema": { "type": "string" }
                },
                "Skill": {
                    "name": "skill", "in": "query", "required": false,
                    "description": "Only rows recorded for this skill",
                    "schema": { "type": "string" }
                },
                "Limit": {
                    "name": "limit", "in": "query", "required": false,
                    "description": "Maximum number of rows to return; each endpoint applies its own cap",
                    "schema": { "type": "integer", "minimum": 1 }
                },
                "Bucket": {
                    "name": "bucket", "in": "query", "required": false,
                    "description": "Time bucket width; unknown values fall back to hour",
                    "schema": { "type": "string", "enum": ["minute", "hour", "day"], "default": "hour" }
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
                    "parameters": [{
                        "name": "style",
                        "in": "query",
                        "required": false,
                        "schema": {
                            "type": "string",
                            "enum": ["flat", "flat-square", "for-the-badge"],
                            "default": "flat-square"
                        }
                    }],
                    "responses": { "200": { "description": "image/svg+xml" } }
                }
            },
            "/badge/events.svg": {
                "get": {
                    "tags": ["query"],
                    "summary": "Embeddable events badge (SVG)",
                    "parameters": [{
                        "name": "style",
                        "in": "query",
                        "required": false,
                        "schema": {
                            "type": "string",
                            "enum": ["flat", "flat-square", "for-the-badge"],
                            "default": "flat-square"
                        }
                    }],
                    "responses": { "200": { "description": "image/svg+xml" } }
                }
            },
            "/api/search": {
                "get": {
                    "tags": ["query"],
                    "summary": "Full-text search over prompts, tools, models and agents",
                    "parameters": [
                        {
                            "name": "q", "in": "query", "required": true,
                            "description": "Search expression; plain terms are matched literally",
                            "schema": { "type": "string" }
                        },
                        { "$ref": "#/components/parameters/Limit" }
                    ],
                    "responses": {
                        "200": { "description": "Matching rows, or an empty list when the query holds no searchable term" },
                        "400": {
                            "description": "The search expression is invalid FTS5 syntax and could not be parsed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Error" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/status/otlp": {
                "get": {
                    "tags": ["meta"],
                    "summary": "OTLP receiver status and ingest buffer pressure",
                    "responses": { "200": { "description": "Receiver status" } }
                }
            },
            "/reports/by-ide": {
                "get": {
                    "tags": ["query"],
                    "summary": "Calls and cost grouped by IDE",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" },
                        { "$ref": "#/components/parameters/Limit" }
                    ],
                    "responses": { "200": { "description": "Report rows" } }
                }
            },
            "/reports/top-agents": {
                "get": {
                    "tags": ["query"],
                    "summary": "Most active agents",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" },
                        { "$ref": "#/components/parameters/Limit" }
                    ],
                    "responses": { "200": { "description": "Report rows" } }
                }
            },
            "/reports/top-tasks": {
                "get": {
                    "tags": ["query"],
                    "summary": "Most active tasks",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" },
                        { "$ref": "#/components/parameters/Limit" }
                    ],
                    "responses": { "200": { "description": "Report rows" } }
                }
            },
            "/reports/top-mcp-servers": {
                "get": {
                    "tags": ["query"],
                    "summary": "Most used MCP servers",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" },
                        { "$ref": "#/components/parameters/Limit" }
                    ],
                    "responses": { "200": { "description": "Report rows" } }
                }
            },
            "/reports/error-patterns": {
                "get": {
                    "tags": ["query"],
                    "summary": "Most frequent error patterns",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" },
                        { "$ref": "#/components/parameters/Limit" }
                    ],
                    "responses": { "200": { "description": "Report rows" } }
                }
            },
            "/reports/models": {
                "get": {
                    "tags": ["query"],
                    "summary": "Distinct model names seen in ingested events",
                    "responses": { "200": { "description": "Model names" } }
                }
            },
            "/reports/calls-over-time": {
                "get": {
                    "tags": ["query"],
                    "summary": "Call volume over time",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" },
                        { "$ref": "#/components/parameters/Bucket" }
                    ],
                    "responses": { "200": { "description": "Bucketed call counts" } }
                }
            },
            "/reports/cost-over-time": {
                "get": {
                    "tags": ["query"],
                    "summary": "Cost over time, bucketed by hour",
                    "parameters": [
                        { "$ref": "#/components/parameters/From" },
                        { "$ref": "#/components/parameters/To" },
                        { "$ref": "#/components/parameters/Repo" },
                        { "$ref": "#/components/parameters/Ide" },
                        { "$ref": "#/components/parameters/Agent" },
                        { "$ref": "#/components/parameters/Model" },
                        { "$ref": "#/components/parameters/Skill" }
                    ],
                    "responses": { "200": { "description": "Bucketed cost" } }
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
        for path in ["/badge/cost.svg", "/badge/events.svg"] {
            let schema = &spec["paths"][path]["get"]["parameters"][0]["schema"];
            assert_eq!(schema["default"], "flat-square");
            assert_eq!(
                schema["enum"],
                serde_json::json!(["flat", "flat-square", "for-the-badge"])
            );
        }
        assert_eq!(spec["openapi"], "3.1.0");
    }

    #[test]
    fn spec_covers_every_rest_endpoint() {
        let spec = build_spec();
        let paths = spec["paths"].as_object().expect("paths object");

        // Every endpoint the collector mounts must be described here. The list duplicates the
        // router on purpose: scripts/ci/check-openapi-coverage.sh is what fails when a new route
        // is mounted without an entry, and this test keeps the two in step.
        for path in [
            "/events/tool-call",
            "/v1/traces",
            "/health",
            "/health/ready",
            "/api/version",
            "/api/status/otlp",
            "/api/admin/reset",
            "/api/search",
            "/api/conversations",
            "/api/conversations/{conversation_id}",
            "/api/conversations/{conversation_id}/timeline",
            "/api/cost/summary",
            "/badge/cost.svg",
            "/badge/events.svg",
            "/reports/top-tools",
            "/reports/top-tasks",
            "/reports/top-mcp-servers",
            "/reports/calls-over-time",
            "/reports/cost-over-time",
            "/reports/by-ide",
            "/reports/top-agents",
            "/reports/error-patterns",
            "/reports/models",
            "/reports/events",
        ] {
            assert!(paths.contains_key(path), "missing path {path}");
        }
    }

    #[test]
    fn spec_parameters_are_declared_once_and_resolve() {
        let spec = build_spec();
        let components = spec["components"]["parameters"]
            .as_object()
            .expect("parameter components");
        for name in [
            "From", "To", "Repo", "Ide", "Agent", "Model", "Skill", "Limit", "Bucket",
        ] {
            assert!(
                components.contains_key(name),
                "missing component parameter {name}"
            );
        }

        // every `$ref` used by an endpoint must point at a declared component
        for (path, entry) in spec["paths"].as_object().expect("paths object") {
            let Some(params) = entry["get"]["parameters"].as_array() else {
                continue;
            };
            for param in params {
                if let Some(reference) = param["$ref"].as_str() {
                    let name = reference.rsplit('/').next().unwrap_or_default();
                    assert!(
                        components.contains_key(name),
                        "{path} references missing parameter {reference}"
                    );
                }
            }
        }
    }
}
