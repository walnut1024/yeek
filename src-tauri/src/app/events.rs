// Event types emitted from Rust to frontend
// Events carry small payloads - frontend should treat them as invalidation signals

use serde::Serialize;

// ---------------------------------------------------------------------------
// EventEmitter trait — abstracts event emission away from Tauri::AppHandle
// ---------------------------------------------------------------------------

pub trait EventEmitter: Send + Sync {
    fn emit_sync_started(&self, payload: SyncStartedPayload);
    fn emit_sync_progress(&self, payload: SyncProgressPayload);
    fn emit_sync_completed(&self, payload: SyncCompletedPayload);
    fn emit_plugin_config_changed(&self);
}

// ---------------------------------------------------------------------------
// Payload types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct SyncUpdatedPayload {
    pub changed_session_ids: Vec<String>,
    pub last_refresh_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncStartedPayload {
    pub source_count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncProgressPayload {
    pub processed: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncCompletedPayload {
    pub sessions_indexed: i64,
    pub sessions_updated: i64,
    pub errors: i64,
}

#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct ActionCompletedPayload {
    pub action: String,
    pub session_ids: Vec<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct SystemErrorPayload {
    pub kind: String,
    pub message: String,
    pub source: Option<String>,
}
