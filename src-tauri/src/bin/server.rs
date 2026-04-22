use std::sync::Arc;
use yeek_lib::app::state::AppState;
use yeek_lib::http::{HttpRuntimeState, SseEventEmitter, build_router};
use yeek_lib::store::schema;
use yeek_lib::sync::background::ScanGuard;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("yeek-server starting...");

    // DB init
    let db_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yeek");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("yeek.db");
    let conn = rusqlite::Connection::open(&db_path).expect("failed to open database");
    schema::init_schema(&conn).expect("failed to initialize schema");

    let sse = Arc::new(SseEventEmitter::new());
    let emitter: Arc<dyn yeek_lib::app::events::EventEmitter> = sse.clone();
    let scan_guard = Arc::new(ScanGuard::new());

    // File watcher
    let claude_projects_dir = dirs::home_dir()
        .expect("Cannot find home directory")
        .join(".claude")
        .join("projects");
    let watcher = yeek_lib::sync::watcher::FileWatcher::start(
        claude_projects_dir, db_path.clone(), emitter.clone(), scan_guard.clone(),
    ).expect("Failed to start file watcher");

    let config_watcher = yeek_lib::sync::watcher::FileWatcher::start_plugin_config_watcher(
        emitter.clone(),
    ).expect("Failed to start plugin config watcher");

    let app_state = Arc::new(
        AppState::new(conn, db_path.clone(), emitter)
            .with_watcher(watcher)
            .with_config_watcher(config_watcher),
    );

    // Startup sync
    yeek_lib::sync::background::spawn_background_scan(
        db_path, app_state.event_emitter.clone(), scan_guard,
    );

    // Router
    let runtime_state = HttpRuntimeState { app_state, sse };
    let app = build_router(runtime_state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 17321));
    log::info!("yeek-server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
