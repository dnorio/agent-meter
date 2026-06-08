//! Query parameter structs for the Database trait methods.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Parameters for report queries (top tools, agents, etc.)
#[derive(Debug, Clone, Default)]
pub struct ReportQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub repo: Option<String>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub skill: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for event feed queries.
#[derive(Debug, Clone)]
pub struct EventQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub conversation_id: Option<String>,
    pub before_started_at: Option<DateTime<Utc>>,
    pub before_event_id: Option<Uuid>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            ide: None,
            agent: None,
            model: None,
            conversation_id: None,
            before_started_at: None,
            before_event_id: None,
            limit: 50,
            offset: 0,
        }
    }
}

/// Parameters for conversation list.
#[derive(Debug, Clone)]
pub struct ConversationQuery {
    pub limit: i64,
    pub offset: i64,
    pub ide: Option<String>,
}

impl Default for ConversationQuery {
    fn default() -> Self {
        Self {
            limit: 20,
            offset: 0,
            ide: None,
        }
    }
}

/// Parameters for cost summary.
#[derive(Debug, Clone)]
pub struct CostQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub model: Option<String>,
}
