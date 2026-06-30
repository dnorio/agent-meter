//! Async ingest buffer.
//!
//! Decouples ingest request handling from database writes using a bounded
//! tokio mpsc channel. Events are sent to the channel and the handler returns
//! immediately. A background task drains the channel in batches and writes
//! through the `Database` trait (backend-agnostic: Postgres or SQLite).

use std::sync::Arc;

use agent_meter_db::Database;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::models::event::ToolCallEvent;
use crate::services::event_service;

const BATCH_SIZE: usize = 64;
const FLUSH_INTERVAL_MS: u64 = 500;

/// Handle to the ingest buffer. Clone-safe (holds sender half).
#[derive(Clone)]
pub struct IngestBuffer {
    tx: mpsc::Sender<ToolCallEvent>,
    capacity: usize,
}

impl IngestBuffer {
    /// Spawn the buffer worker. Returns a handle for sending events.
    pub fn spawn(db: Arc<dyn Database>, capacity: usize, cancel: CancellationToken) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        tokio::spawn(buffer_worker(rx, db, cancel));
        Self { tx, capacity }
    }

    /// Send an event to the buffer. Returns Err if the channel is full or closed.
    pub async fn send(&self, event: ToolCallEvent) -> Result<(), mpsc::error::SendError<ToolCallEvent>> {
        self.tx.send(event).await
    }

    /// Try to send without waiting (for fire-and-forget paths).
    pub fn try_send(&self, event: ToolCallEvent) -> Result<(), mpsc::error::TrySendError<ToolCallEvent>> {
        self.tx.try_send(event)
    }

    /// Total channel capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of messages currently queued (approximate).
    pub fn queued(&self) -> usize {
        self.capacity - self.tx.capacity()
    }
}

async fn buffer_worker(
    mut rx: mpsc::Receiver<ToolCallEvent>,
    db: Arc<dyn Database>,
    cancel: CancellationToken,
) {
    info!("ingest_buffer: worker started (batch={}, flush={}ms)", BATCH_SIZE, FLUSH_INTERVAL_MS);
    let mut batch: Vec<ToolCallEvent> = Vec::with_capacity(BATCH_SIZE);

    loop {
        let deadline = tokio::time::sleep(tokio::time::Duration::from_millis(FLUSH_INTERVAL_MS));
        tokio::pin!(deadline);

        loop {
            tokio::select! {
                biased;

                _ = cancel.cancelled() => {
                    rx.close();
                    while let Some(ev) = rx.recv().await {
                        batch.push(ev);
                    }
                    if !batch.is_empty() {
                        flush_batch(&db, &mut batch).await;
                    }
                    info!("ingest_buffer: worker stopped (graceful)");
                    return;
                }

                Some(ev) = rx.recv() => {
                    batch.push(ev);
                    if batch.len() >= BATCH_SIZE {
                        break;
                    }
                }

                _ = &mut deadline => {
                    break;
                }
            }
        }

        if !batch.is_empty() {
            flush_batch(&db, &mut batch).await;
        }
    }
}

async fn flush_batch(db: &Arc<dyn Database>, batch: &mut Vec<ToolCallEvent>) {
    let count = batch.len();
    let mut success = 0;
    let mut failed = 0;

    for event in batch.drain(..) {
        let insert = event_service::to_insert(event);
        match db.insert_tool_call(&insert).await {
            Ok(_) => success += 1,
            Err(e) => {
                failed += 1;
                if failed <= 3 {
                    error!("ingest_buffer: insert failed: {e}");
                }
            }
        }
    }

    if failed > 0 {
        warn!("ingest_buffer: flushed {success}/{count} ({failed} errors)");
    }
}
