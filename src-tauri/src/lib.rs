pub mod adapter;
pub mod app;
pub mod domain;
pub mod service;
pub mod store;
pub mod sync;

#[cfg(feature = "tauri-shell")]
mod tauri_bridge;

#[cfg(feature = "http-server")]
pub mod http;

#[cfg(feature = "tauri-shell")]
pub fn run() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;
    use tauri::{Manager, WindowEvent};

    use app::state::AppState;
    use store::schema;
    use tauri_bridge::TauriEventEmitter;

    let quitting = Arc::new(AtomicBool::new(false));
    let quitting_window = quitting.clone();

    tauri::Builder::default()
        .on_window_event(move |window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !quitting_window.load(Ordering::Relaxed) {
                    api.prevent_close();
                    window.hide().ok();
                }
            }
        })
        .setup(move |app| {
            // Initialize logging
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build(),
                )?;
            } else {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default().level(log::LevelFilter::Warn).build(),
                )?;
            }

            // Updater plugin — automatic in-app updates from GitHub Releases
            app.handle().plugin(
                tauri_plugin_updater::Builder::new().build(),
            )?;
            // Process plugin — relaunch after update install
            app.handle().plugin(tauri_plugin_process::init())?;

            // Create event emitter
            let emitter: Arc<dyn app::events::EventEmitter> =
                Arc::new(TauriEventEmitter { handle: app.handle().clone() });

            // Initialize database
            let app_dir = app.path().app_data_dir().expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();

            let db_path = app_dir.join("yeek.db");
            let conn = rusqlite::Connection::open(&db_path).expect("failed to open database");

            // Lightweight schema + migrations (main thread, fast)
            let pending_heavy =
                schema::init_schema(&conn).expect("failed to initialize database schema");

            // Resolve Claude projects dir for file watcher
            let claude_projects_dir = dirs::home_dir()
                .expect("Cannot find home directory")
                .join(".claude")
                .join("projects");

            let codex_sessions_dir = dirs::home_dir()
                .expect("Cannot find home directory")
                .join(".codex")
                .join("sessions");

            let scan_guard = Arc::new(sync::background::ScanGuard::new());

            // Start file watchers for auto incremental updates
            let mut watchers = Vec::new();

            if claude_projects_dir.exists() {
                watchers.push(
                    sync::watcher::FileWatcher::start(
                        claude_projects_dir,
                        db_path.clone(),
                        emitter.clone(),
                        scan_guard.clone(),
                    )
                    .expect("Failed to start Claude file watcher"),
                );
            }

            if codex_sessions_dir.exists() {
                watchers.push(
                    sync::watcher::FileWatcher::start(
                        codex_sessions_dir,
                        db_path.clone(),
                        emitter.clone(),
                        scan_guard.clone(),
                    )
                    .expect("Failed to start Codex file watcher"),
                );
            }

            // Start plugin config watcher for install/uninstall status updates
            let config_watcher =
                sync::watcher::FileWatcher::start_plugin_config_watcher(emitter.clone())
                    .expect("Failed to start plugin config watcher");

            // Initialize proxy manager (config stored in database)
            let proxy_db = std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open(&db_path).expect("failed to open proxy db"),
            ));
            let proxy_manager = std::sync::Arc::new(app::proxy::ProxyManager::with_db(proxy_db));
            app::proxy::ProxyManager::initialize(&proxy_manager);

            app.manage(
                AppState::new(
                    conn,
                    db_path.clone(),
                    emitter.clone(),
                    proxy_manager,
                )
                .with_watchers(watchers)
                .with_config_watcher(config_watcher),
            );

            // Heavy data migrations on background thread (non-blocking)
            if let Some(_target) = pending_heavy {
                let hm_db_path = db_path.clone();
                std::thread::Builder::new()
                    .name("yeek-schema-migrate".into())
                    .spawn(move || {
                        if let Err(e) = schema::run_heavy_migrations(&hm_db_path) {
                            tracing::error!("Heavy migration failed: {}", e);
                        }
                    })
                    .expect("Failed to spawn migration thread");
            }

            // Resume any incomplete delete jobs from previous session
            app::commands::resume_pending_delete_jobs(&app.state::<AppState>());

            // Startup sync: background thread — window appears immediately
            sync::background::spawn_background_scan(db_path, emitter, scan_guard);

            // System tray — keep running in background when window is closed
            let handle = app.handle().clone();
            let quitting_tray = quitting.clone();
            let show = MenuItemBuilder::with_id("show", "Show Yeek").build(&handle)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit Yeek").build(&handle)?;
            let tray_menu = MenuBuilder::new(&handle)
                .item(&show)
                .separator()
                .item(&quit)
                .build()?;
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&tray_menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            w.show().ok();
                            w.set_focus().ok();
                        }
                    }
                    "quit" => {
                        quitting_tray.store(true, Ordering::SeqCst);
                        std::process::exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            w.show().ok();
                            w.set_focus().ok();
                        }
                    }
                })
                .build(app)?;

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
            tauri_bridge::commands::destructive_delete_sessions,
            tauri_bridge::commands::get_delete_job,
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
            tauri_bridge::commands::get_proxy_status,
            tauri_bridge::commands::start_proxy,
            tauri_bridge::commands::stop_proxy,
            tauri_bridge::commands::restart_proxy,
            tauri_bridge::commands::get_proxy_config,
            tauri_bridge::commands::update_proxy_config,
            tauri_bridge::commands::get_proxy_logs,
            tauri_bridge::commands::get_proxy_metrics,
            tauri_bridge::commands::get_proxy_error_events,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
