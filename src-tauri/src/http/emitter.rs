use std::sync::Arc;
use tokio::sync::broadcast;
use crate::app::events::*;

#[derive(Clone)]
pub struct SseEventEmitter {
    tx: broadcast::Sender<String>,
}

impl SseEventEmitter {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(32);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn into_arc(self) -> Arc<dyn EventEmitter> {
        Arc::new(self)
    }
}

impl EventEmitter for SseEventEmitter {
    fn emit_sync_started(&self, payload: SyncStartedPayload) {
        self.broadcast("sync-started", &payload);
    }
    fn emit_sync_progress(&self, payload: SyncProgressPayload) {
        self.broadcast("sync-progress", &payload);
    }
    fn emit_sync_completed(&self, payload: SyncCompletedPayload) {
        self.broadcast("sync-completed", &payload);
    }
    fn emit_plugin_config_changed(&self) {
        let json = serde_json::json!({
            "event": "plugin-config-changed",
            "payload": null,
            "ts": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        });
        let _ = self.tx.send(json.to_string());
    }
}

impl SseEventEmitter {
    fn broadcast(&self, event: &str, payload: &impl serde::Serialize) {
        let json = serde_json::json!({
            "event": event,
            "payload": payload,
            "ts": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        });
        let _ = self.tx.send(json.to_string());
    }
}
