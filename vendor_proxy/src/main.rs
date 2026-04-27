use vendor_proxy::client;
use vendor_proxy::config;
use vendor_proxy::server;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = config::ProxyConfig::load("proxy.toml").expect("Failed to load proxy.toml");

    tracing::info!("Proxy config loaded: {} providers", cfg.providers.len());
    tracing::info!("Default provider: {}", cfg.default_provider);

    let state = Arc::new(server::AppState {
        config: cfg.clone(),
        client: client::HttpClient::new(),
    });

    let app = axum::Router::new()
        .route("/health", axum::routing::any(server::health))
        .route("/v1/models", axum::routing::get(server::models_handler))
        .route("/v1/{*path}", axum::routing::any(server::proxy_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.server.listen_addr)
        .await
        .expect("Failed to bind");
    tracing::info!("Proxy listening on {}", cfg.server.listen_addr);
    axum::serve(listener, app).await.unwrap();
}
