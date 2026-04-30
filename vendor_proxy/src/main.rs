use std::collections::{HashMap, VecDeque};
use std::sync::{atomic::AtomicI64, Arc, Mutex};
use std::time::Instant;
use vendor_proxy::client;
use vendor_proxy::config;
use vendor_proxy::server;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "proxy.toml".to_string());

    let cfg = config::ProxyConfig::load(&config_path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {}", config_path, e));

    tracing::info!("Proxy config loaded: {} providers", cfg.providers.len());
    tracing::info!("Default provider: {}", cfg.default_provider);

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
    });

    let app = axum::Router::new()
        .route("/health", axum::routing::any(server::health))
        .route("/admin/status", axum::routing::get(server::admin_status))
        .route("/v1/models", axum::routing::get(server::models_handler))
        .route("/v1/{*path}", axum::routing::any(server::proxy_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.server.listen_addr)
        .await
        .expect("Failed to bind");
    tracing::info!("Proxy listening on {}", cfg.server.listen_addr);
    axum::serve(listener, app).await.expect("axum serve");
}
