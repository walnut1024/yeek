use std::sync::Arc;
use yeek_lib::app::proxy::ProxyManager;
use yeek_lib::app::state::AppState;
use yeek_lib::http::{build_router, HttpRuntimeState, SseEventEmitter};
use yeek_lib::store::schema;
use yeek_lib::sync::background::ScanGuard;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Diagnostic mode: run scan diagnostics and exit
    if args.len() > 1 && args[1] == "--diagnose-scan" {
        run_diagnostics(&args);
        return;
    }

    // Normal server mode
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    tracing::info!("yeek-server starting...");

    // DB init
    let db_dir =
        dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("yeek");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("yeek.db");
    let conn = rusqlite::Connection::open(&db_path).expect("failed to open database");
    schema::init_schema(&conn).expect("failed to initialize schema");

    let sse = Arc::new(SseEventEmitter::new());
    let emitter: Arc<dyn yeek_lib::app::events::EventEmitter> = sse.clone();
    let scan_guard = Arc::new(ScanGuard::new());

    // File watchers
    let claude_projects_dir =
        dirs::home_dir().expect("Cannot find home directory").join(".claude").join("projects");
    let codex_sessions_dir =
        dirs::home_dir().expect("Cannot find home directory").join(".codex").join("sessions");

    let mut watchers = Vec::new();
    if claude_projects_dir.exists() {
        watchers.push(
            yeek_lib::sync::watcher::FileWatcher::start(
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
            yeek_lib::sync::watcher::FileWatcher::start(
                codex_sessions_dir,
                db_path.clone(),
                emitter.clone(),
                scan_guard.clone(),
            )
            .expect("Failed to start Codex file watcher"),
        );
    }

    let config_watcher =
        yeek_lib::sync::watcher::FileWatcher::start_plugin_config_watcher(emitter.clone())
            .expect("Failed to start plugin config watcher");

    // Open a second connection for proxy config (DB-backed mode)
    let proxy_db = std::sync::Arc::new(std::sync::Mutex::new(
        rusqlite::Connection::open(&db_path).expect("failed to open proxy db"),
    ));
    let proxy_manager = Arc::new(ProxyManager::with_db(proxy_db));
    ProxyManager::initialize(&proxy_manager);

    let app_state = Arc::new(
        AppState::new(conn, db_path.clone(), emitter, proxy_manager)
            .with_watchers(watchers)
            .with_config_watcher(config_watcher),
    );

    // Startup sync
    yeek_lib::sync::background::spawn_background_scan(
        db_path,
        app_state.event_emitter.clone(),
        scan_guard,
    );

    // Router
    let runtime_state = HttpRuntimeState { app_state, sse };
    let app = build_router(runtime_state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 17321));
    tracing::info!("yeek-server listening on http://{}", addr);
    let listener =
        tokio::net::TcpListener::bind(addr).await.expect("failed to bind diagnostic server");
    axum::serve(listener, app).await.expect("diagnostic server crashed");
}

fn run_diagnostics(args: &[String]) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let db_dir =
        dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("yeek");
    let db_path = db_dir.join("yeek.db");

    if !db_path.exists() {
        eprintln!("Database not found at {}. Run yeek-server normally first.", db_path.display());
        std::process::exit(1);
    }

    eprintln!("Running diagnostic scan on {}...", db_path.display());

    let use_json = args.iter().any(|a| a == "--json");

    match yeek_lib::adapter::claudecode::diagnostic::run_diagnostic_scan(&db_path) {
        Ok(result) => {
            if use_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|e| format!("JSON error: {}", e))
                );
            } else {
                println!("=== Scan Diagnostic Report ===");
                println!("Total discovered: {}", result.total_discovered);
                println!("Total attempted:  {}", result.total_attempted);
                println!("Succeeded:        {}", result.total_succeeded);
                println!("Skipped (cached): {}", result.total_skipped);
                println!("Failed:           {}", result.total_failed);
                println!();
                if result.total_failed > 0 {
                    println!("--- Failure Summary ---");
                    for (key, count) in &result.failure_summary {
                        println!("  {}: {}", key, count);
                    }
                    println!();
                    println!("--- Sample Failures (first 10) ---");
                    for err in result.failures.iter().take(10) {
                        println!("  [{:?}] {}", err.stage, err.source_path);
                        println!("    kind={}, msg={}", err.error_kind, err.message);
                    }
                }
            }
        },
        Err(e) => {
            eprintln!("Diagnostic scan failed: {}", e);
            std::process::exit(1);
        },
    }
}
