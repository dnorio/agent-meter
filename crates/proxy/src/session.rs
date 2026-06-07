use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use uuid::Uuid;

/// Session grouping: maps a session key to a (trace_id, last_seen) pair.
/// Calls within 30 minutes of idle share the same trace_id.
pub struct SessionManager {
    sessions: Mutex<HashMap<String, SessionEntry>>,
}

struct SessionEntry {
    trace_id: String,
    last_seen: Instant,
}

const IDLE_WINDOW_SECS: u64 = 30 * 60; // 30 minutes

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a trace_id for the given session key.
    pub fn trace_id_for(&self, session_key: &str) -> String {
        let mut sessions = self.sessions.lock().unwrap();
        let now = Instant::now();

        if let Some(entry) = sessions.get_mut(session_key) {
            if now.duration_since(entry.last_seen).as_secs() < IDLE_WINDOW_SECS {
                entry.last_seen = now;
                return entry.trace_id.clone();
            }
        }

        // New session or expired
        let trace_id = Uuid::new_v4().to_string().replace('-', "");
        sessions.insert(session_key.to_string(), SessionEntry {
            trace_id: trace_id.clone(),
            last_seen: now,
        });

        // Prune old sessions (> 2h)
        sessions.retain(|_, v| now.duration_since(v.last_seen).as_secs() < 7200);

        trace_id
    }
}
