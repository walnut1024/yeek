mod adapter;
mod app;
mod domain;
mod service;
mod store;
mod sync;

use std::sync::Arc;
use tauri::{Emitter, Manager};

use app::commands::{
    add_marketplace, browse_sessions, clean_plugin, destructive_delete_session, get_action_log, get_delete_plan, get_session_detail,
    get_session_preview, get_session_transcript, get_subagent_messages, get_system_status, install_marketplace_plugin, list_marketplaces, list_marketplace_plugins, list_plugins,
    reinstall_plugin, release_and_resync, remove_marketplace, rescan_sources, resume_session, search_sessions, soft_delete_project, soft_delete_sessions, toggle_plugin, uninstall_plugin, update_marketplace,
};
use app::state::AppState;
use store::schema;

// ---------------------------------------------------------------------------
// TauriEventEmitter — bridges EventEmitter trait to Tauri's AppHandle
// ---------------------------------------------------------------------------

struct TauriEventEmitter {
    handle: tauri::AppHandle,
}

impl app::events::EventEmitter for TauriEventEmitter {
    fn emit_sync_started(&self, payload: app::events::SyncStartedPayload) {
        let _ = self.handle.emit("sync-started", payload);
    }
    fn emit_sync_progress(&self, payload: app::events::SyncProgressPayload) {
        let _ = self.handle.emit("sync-progress", payload);
    }
    fn emit_sync_completed(&self, payload: app::events::SyncCompletedPayload) {
        let _ = self.handle.emit("sync-completed", payload);
    }
    fn emit_plugin_config_changed(&self) {
        let _ = self.handle.emit("plugin-config-changed", ());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize logging
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            } else {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Warn)
                        .build(),
                )?;
            }

            // Create event emitter
            let emitter: Arc<dyn app::events::EventEmitter> =
                Arc::new(TauriEventEmitter { handle: app.handle().clone() });

            // Initialize database
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();

            let db_path = app_dir.join("yeek.db");
            let conn =
                rusqlite::Connection::open(&db_path).expect("failed to open database");

            // Lightweight schema + migrations (main thread, fast)
            let pending_heavy = schema::init_schema(&conn)
                .expect("failed to initialize database schema");

            // Resolve Claude projects dir for file watcher
            let claude_projects_dir = dirs::home_dir()
                .expect("Cannot find home directory")
                .join(".claude")
                .join("projects");

            let scan_guard = Arc::new(sync::background::ScanGuard::new());

            // Start file watcher for auto incremental updates
            let watcher = sync::watcher::FileWatcher::start(
                claude_projects_dir,
                db_path.clone(),
                emitter.clone(),
                scan_guard.clone(),
            )
            .expect("Failed to start file watcher");

            // Start plugin config watcher for install/uninstall status updates
            let config_watcher = sync::watcher::FileWatcher::start_plugin_config_watcher(
                emitter.clone(),
            )
            .expect("Failed to start plugin config watcher");

            app.manage(AppState::new(conn, db_path.clone(), emitter.clone())
                .with_watcher(watcher)
                .with_config_watcher(config_watcher));

            // Heavy data migrations on background thread (non-blocking)
            if let Some(_target) = pending_heavy {
                let hm_db_path = db_path.clone();
                std::thread::Builder::new()
                    .name("yeek-schema-migrate".into())
                    .spawn(move || {
                        if let Err(e) = schema::run_heavy_migrations(&hm_db_path) {
                            log::error!("Heavy migration failed: {}", e);
                        }
                    })
                    .expect("Failed to spawn migration thread");
            }

            // Startup sync: background thread — window appears immediately
            sync::background::spawn_background_scan(
                db_path,
                emitter,
                scan_guard,
            );

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            browse_sessions,
            search_sessions,
            get_session_preview,
            get_session_detail,
            get_session_transcript,
            get_subagent_messages,
            soft_delete_sessions,
            soft_delete_project,
            get_action_log,
            rescan_sources,
            release_and_resync,
            resume_session,
            get_delete_plan,
            destructive_delete_session,
            list_plugins,
            toggle_plugin,
            uninstall_plugin,
            clean_plugin,
            reinstall_plugin,
            list_marketplaces,
            add_marketplace,
            update_marketplace,
            remove_marketplace,
            list_marketplace_plugins,
            install_marketplace_plugin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
