mod adapter;
mod app;
mod domain;
mod service;
mod store;
mod sync;
mod tauri_bridge;

use std::sync::Arc;

use tauri::Manager;

use app::state::AppState;
use store::schema;
use tauri_bridge::TauriEventEmitter;

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
            tauri_bridge::commands::get_system_status,
            tauri_bridge::commands::browse_sessions,
            tauri_bridge::commands::search_sessions,
            tauri_bridge::commands::get_session_preview,
            tauri_bridge::commands::get_session_detail,
            tauri_bridge::commands::get_session_transcript,
            tauri_bridge::commands::get_subagent_messages,
            tauri_bridge::commands::soft_delete_sessions,
            tauri_bridge::commands::soft_delete_project,
            tauri_bridge::commands::get_action_log,
            tauri_bridge::commands::rescan_sources,
            tauri_bridge::commands::release_and_resync,
            tauri_bridge::commands::resume_session,
            tauri_bridge::commands::get_delete_plan,
            tauri_bridge::commands::destructive_delete_session,
            tauri_bridge::commands::list_plugins,
            tauri_bridge::commands::toggle_plugin,
            tauri_bridge::commands::uninstall_plugin,
            tauri_bridge::commands::clean_plugin,
            tauri_bridge::commands::reinstall_plugin,
            tauri_bridge::commands::list_marketplaces,
            tauri_bridge::commands::add_marketplace,
            tauri_bridge::commands::update_marketplace,
            tauri_bridge::commands::remove_marketplace,
            tauri_bridge::commands::list_marketplace_plugins,
            tauri_bridge::commands::install_marketplace_plugin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
