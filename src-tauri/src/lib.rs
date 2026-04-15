mod adapter;
mod app;
mod domain;
mod service;
mod store;
mod sync;

use tauri::Manager;

use app::commands::{
    browse_sessions, destructive_delete_session, get_action_log, get_delete_plan, get_session_detail,
    get_session_preview, get_subagent_messages, get_system_status, rescan_sources, search_sessions, set_archived,
    set_hidden, set_pinned, soft_delete_sessions,
};
use app::state::AppState;
use store::schema;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize logging — debug gets Info level to console, release gets Warn to log file
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

            // Run startup sync
            match sync::run_startup_sync(&conn) {
                Ok(summary) => {
                    log::info!(
                        "Startup sync: {} sessions indexed",
                        summary.sessions_indexed
                    );
                }
                Err(e) => {
                    log::error!("Startup sync failed: {}", e);
                }
            }

            app.manage(AppState::new(conn).with_handle(app.handle().clone()));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            browse_sessions,
            search_sessions,
            get_session_preview,
            get_session_detail,
            get_subagent_messages,
            set_pinned,
            set_archived,
            set_hidden,
            soft_delete_sessions,
            get_action_log,
            rescan_sources,
            get_delete_plan,
            destructive_delete_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
