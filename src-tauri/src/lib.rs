mod adapter;
mod app;
mod domain;
mod service;
mod store;
mod sync;

use tauri::Manager;

use app::commands::{
    browse_sessions, destructive_delete_session, get_action_log, get_delete_plan, get_session_detail,
    get_session_preview, get_session_transcript, get_subagent_messages, get_system_status, list_plugins, release_and_resync, rescan_sources, resume_session, search_sessions, soft_delete_project, soft_delete_sessions,
};
use app::state::AppState;
use store::schema;

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

            // Initialize database
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();

            let db_path = app_dir.join("yeek.db");
            let conn =
                rusqlite::Connection::open(&db_path).expect("failed to open database");

            schema::init_schema(&conn).expect("failed to initialize database schema");

            // Resolve Claude projects dir for file watcher
            let claude_projects_dir = dirs::home_dir()
                .expect("Cannot find home directory")
                .join(".claude")
                .join("projects");

            let scan_guard = std::sync::Arc::new(sync::background::ScanGuard::new());

            // Start file watcher for auto incremental updates
            let watcher = sync::watcher::FileWatcher::start(
                claude_projects_dir,
                db_path.clone(),
                app.handle().clone(),
                scan_guard.clone(),
            )
            .expect("Failed to start file watcher");

            app.manage(AppState::new(conn, db_path.clone())
                .with_handle(app.handle().clone())
                .with_watcher(watcher));

            // Startup sync: background thread — window appears immediately
            sync::background::spawn_background_scan(
                db_path,
                app.handle().clone(),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
