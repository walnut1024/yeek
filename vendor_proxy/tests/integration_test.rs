use axum::{
    extract::State,
    http::HeaderMap,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::post,
    Json, Router,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicI64, AtomicU64, Ordering},
    Arc, Mutex, RwLock,
};

static PORT_COUNTER: AtomicU64 = AtomicU64::new(10000);

fn next_port() -> u16 {
    (PORT_COUNTER.fetch_add(1, Ordering::Relaxed) % 50000 + 10000) as u16
}

#[derive(Clone)]
struct MockState {
    expected_key: String,
}

#[tokio::test]
async fn anthropic_passthrough_maps_request_and_restores_response_model() {
    let port = next_port();
    std::env::set_var("VENDOR_PROXY_TEST_KEY", "provider-secret");

    let mock_app = Router::new()
        .route("/v1/messages", post(anthropic_response_handler))
        .with_state(MockState { expected_key: "provider-secret".to_string() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_anthropic_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/test/v1/messages", proxy_addr))
        .header("Content-Type", "application/json")
        .header("x-api-key", "agent-key-that-must-not-be-forwarded")
        .json(&serde_json::json!({
            "model": "claude-sonnet",
            "max_tokens": 64,
            "stream": false,
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["model"], "claude-sonnet");
    assert_eq!(body["content"][0]["text"], "Hi there!");

    std::env::remove_var("VENDOR_PROXY_TEST_KEY");
    mock_handle.abort();
}

#[tokio::test]
async fn anthropic_passthrough_rejects_unconfigured_model() {
    let port = next_port();
    std::env::set_var("VENDOR_PROXY_TEST_KEY", "provider-secret");

    let mock_app = Router::new()
        .route("/v1/messages", post(anthropic_response_handler))
        .with_state(MockState { expected_key: "provider-secret".to_string() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_anthropic_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/test/v1/messages", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 64,
            "stream": false,
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "unknown_model");

    std::env::remove_var("VENDOR_PROXY_TEST_KEY");
    mock_handle.abort();
}

#[tokio::test]
async fn anthropic_passthrough_preserves_sse_event_names() {
    let port = next_port();
    std::env::set_var("VENDOR_PROXY_TEST_KEY", "provider-secret");

    let mock_app = Router::new()
        .route("/v1/messages", post(anthropic_stream_handler))
        .with_state(MockState { expected_key: "provider-secret".to_string() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_anthropic_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let body = client
        .post(format!("http://{}/test/v1/messages", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet",
            "max_tokens": 64,
            "stream": true,
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(body.contains("event: message_start"), "{body}");
    assert!(body.contains(r#""model":"claude-sonnet""#), "{body}");

    std::env::remove_var("VENDOR_PROXY_TEST_KEY");
    mock_handle.abort();
}

#[tokio::test]
async fn models_endpoint_exposes_agent_models_only() {
    let port = next_port();
    let proxy_addr = start_anthropic_proxy("127.0.0.1:9", port).await;

    let client = reqwest::Client::new();
    let resp = client.get(format!("http://{}/test/v1/models", proxy_addr)).send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let models = body["models"].as_array().unwrap();
    let slugs: Vec<_> = models.iter().map(|m| m["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs, vec!["claude-haiku", "claude-opus", "claude-sonnet"]);
}

async fn start_anthropic_proxy(upstream_addr: &str, port: u16) -> std::net::SocketAddr {
    let config_content = format!(
        "[server]\nlisten_addr = \"127.0.0.1:{}\"\n\n[bridges.test.agent]\nbase_url = \"/test\"\napi_format = \"anthropic_messages\"\n\n[bridges.test.provider]\nname = \"test\"\n\n[bridges.test.models]\n\"claude-sonnet\" = \"provider-sonnet\"\n\"claude-haiku\" = \"provider-haiku\"\n\"claude-opus\" = \"provider-opus\"\n\n[providers.test]\nbase_url = \"http://{}\"\napi_format = \"anthropic_messages\"\napi_key_env = \"VENDOR_PROXY_TEST_KEY\"\n",
        port, upstream_addr
    );

    let cfg = vendor_proxy::config::ProxyConfig::from_toml_str(&config_content).unwrap();
    let state = Arc::new(vendor_proxy::server::AppState {
        config: cfg,
        client: vendor_proxy::client::HttpClient::new(),
        started_at: std::time::Instant::now(),
        request_count: AtomicU64::new(0),
        error_count: AtomicU64::new(0),
        active_connections: AtomicI64::new(0),
        latency_total_ns: AtomicU64::new(0),
        request_times: Mutex::new(VecDeque::new()),
        provider_stats: Arc::new(RwLock::new(HashMap::new())),
        error_events: Mutex::new(VecDeque::new()),
    });
    let proxy_app = Router::new()
        .route("/test/v1/messages", post(vendor_proxy::server::bridge_handler))
        .route("/test/v1/models", axum::routing::get(vendor_proxy::server::bridge_models_handler))
        .with_state(state);
    let proxy_listener =
        tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(proxy_listener, proxy_app).await.unwrap();
    });
    proxy_addr
}

async fn anthropic_response_handler(
    State(state): State<MockState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    assert_eq!(
        headers.get("x-api-key").and_then(|v| v.to_str().ok()),
        Some(state.expected_key.as_str())
    );
    assert_eq!(body["model"], "provider-sonnet");

    Json(serde_json::json!({
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "model": "provider-sonnet",
        "content": [{"type": "text", "text": "Hi there!"}],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 10, "output_tokens": 3}
    }))
}

async fn anthropic_stream_handler(
    State(state): State<MockState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    assert_eq!(
        headers.get("x-api-key").and_then(|v| v.to_str().ok()),
        Some(state.expected_key.as_str())
    );
    assert_eq!(body["model"], "provider-sonnet");

    let stream = futures_util::stream::iter(vec![
        Ok::<_, std::convert::Infallible>(
            Event::default().event("message_start").data(
                r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","model":"provider-sonnet","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":0}}}"#,
            ),
        ),
        Ok(Event::default().event("message_stop").data(r#"{"type":"message_stop"}"#)),
    ]);
    Sse::new(stream)
}
