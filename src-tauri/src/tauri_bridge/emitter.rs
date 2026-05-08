use tauri::Emitter;

use crate::app::events::*;

pub struct TauriEventEmitter {
    pub handle: tauri::AppHandle,
}

impl EventEmitter for TauriEventEmitter {
    fn emit_sync_started(&self, payload: SyncStartedPayload) {
        let _ = self.handle.emit("sync-started", payload);
    }

    fn emit_sync_progress(&self, payload: SyncProgressPayload) {
        let _ = self.handle.emit("sync-progress", payload);
    }

    fn emit_sync_completed(&self, payload: SyncCompletedPayload) {
        let _ = self.handle.emit("sync-completed", payload);
    }

    fn emit_plugin_config_changed(&self) {
        let _ = self.handle.emit("plugin-config-changed", ());
    }

    fn emit_delete_progress(&self, payload: DeleteProgressPayload) {
        if let Err(e) = self.handle.emit("delete-progress", &payload) {
            tracing::warn!("emit_delete_progress failed: {}", e);
        }
    }
}
