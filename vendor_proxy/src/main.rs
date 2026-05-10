use std::collections::{HashMap, VecDeque};
use std::os::unix::io::AsRawFd;
use std::sync::{atomic::AtomicI64, Arc, Mutex};
use std::time::Instant;
use vendor_proxy::client;
use vendor_proxy::config;
use vendor_proxy::server;

const PID_FILE: &str = "proxy.pid";

fn current_timezone_offset() -> String {
    chrono::Local::now().format("%:z").to_string()
}

fn acquire_pid_lock() -> std::fs::File {
    let dir = std::env::temp_dir().join("yeek");
    std::fs::create_dir_all(&dir).expect("create pid dir");
    let path = dir.join(PID_FILE);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .unwrap_or_else(|e| panic!("open pid file {}: {}", path.display(), e));
    let fd = file.as_raw_fd();
    let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        let existing_pid = std::fs::read_to_string(&path).unwrap_or_default();
        panic!(
            "Another proxy instance is running (pid: {}) — lock {}: {}",
            existing_pid.trim(),
            path.display(),
            std::io::Error::last_os_error()
        );
    }
    file.set_len(0).ok();
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0)).ok();
    write!(file, "{}", std::process::id()).ok();
    tracing::info!("Acquired PID lock: {} (pid {})", path.display(), std::process::id());
    file
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!("Log timezone selected: local offset={}", current_timezone_offset());

    let _pid_lock = acquire_pid_lock();

    let config_path = std::env::args().nth(1).unwrap_or_else(|| "proxy.toml".to_string());

    let cfg = config::ProxyConfig::load(&config_path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {}", config_path, e));

    tracing::info!(
        "Proxy config loaded: {} providers, {} bridges",
        cfg.providers.len(),
        cfg.bridges.len()
    );

    let state = Arc::new(server::AppState {
        config: cfg.clone(),
        client: client::HttpClient::new(),
        started_at: Instant::now(),
        request_count: std::sync::atomic::AtomicU64::new(0),
        error_count: std::sync::atomic::AtomicU64::new(0),
        active_connections: AtomicI64::new(0),
        latency_total_ns: std::sync::atomic::AtomicU64::new(0),
        request_times: Mutex::new(VecDeque::new()),
        provider_stats: Arc::new(std::sync::RwLock::new(HashMap::new())),
        error_events: Mutex::new(VecDeque::new()),
    });

    let mut app = axum::Router::new()
        .route("/health", axum::routing::any(server::health))
        .route("/admin/status", axum::routing::get(server::admin_status))
        .route("/admin/errors", axum::routing::get(server::admin_errors))
        .route("/v1/models", axum::routing::get(server::global_models_handler));

    for (name, bridge) in &cfg.bridges {
        let prefix = bridge.agent.base_url.trim_end_matches('/');
        let request_path = format!("{}{}", prefix, bridge.agent.api_format.endpoint_path());
        let models_path = format!("{}/v1/models", prefix);
        app = app
            .route(&request_path, axum::routing::post(server::bridge_handler))
            .route(&models_path, axum::routing::get(server::bridge_models_handler));
        tracing::info!(
            "Bridge '{}': agent_base_url={}, agent_api_format={:?}, provider={}",
            name,
            bridge.agent.base_url,
            bridge.agent.api_format,
            bridge.provider.name
        );
    }

    let app = app.with_state(state);

    let listener =
        tokio::net::TcpListener::bind(&cfg.server.listen_addr).await.expect("Failed to bind");
    tracing::info!("Proxy listening on {}", cfg.server.listen_addr);
    axum::serve(listener, app).await.expect("axum serve");
}
