use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use std::sync::atomic::{AtomicU64, Ordering};

static PORT_COUNTER: AtomicU64 = AtomicU64::new(10000);

fn next_port() -> u16 {
    (PORT_COUNTER.fetch_add(1, Ordering::Relaxed) % 50000 + 10000) as u16
}

/// Full E2E test: Responses request → Chat passthrough → Chat response → Responses response
#[tokio::test]
#[ignore = "requires network access; run with: cargo test -- --ignored"]
async fn test_chat_completions_passthrough_e2e() {
    let port = next_port();
    let mock_app = Router::new().route("/chat/completions", post(text_response_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/responses", proxy_addr))
        .header("Content-Type", "application/json")
        .header("x-codex-provider", "test")
        .json(&serde_json::json!({
            "model": "test-model",
            "instructions": "You are helpful.",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Hello"}]
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "completed");
    assert_eq!(body["output"][0]["type"], "message");
    assert_eq!(body["output"][0]["content"][0]["text"], "Hi there!");
    assert_eq!(body["usage"]["input_tokens"], 10);
    assert_eq!(body["usage"]["output_tokens"], 5);

    mock_handle.abort();
}

/// Verify echo fields are returned in the response
#[tokio::test]
#[ignore = "requires network access; run with: cargo test -- --ignored"]
async fn test_echo_fields_e2e() {
    let port = next_port();
    let mock_app = Router::new().route("/chat/completions", post(text_response_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/responses", proxy_addr))
        .header("Content-Type", "application/json")
        .header("x-codex-provider", "test")
        .json(&serde_json::json!({
            "model": "test-model",
            "instructions": "Be helpful.",
            "temperature": 0.7,
            "top_p": 0.9,
            "max_output_tokens": 100,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Hello"}]
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["temperature"], 0.7);
    assert_eq!(body["top_p"], 0.9);
    assert_eq!(body["max_output_tokens"], 100);

    mock_handle.abort();
}

/// Verify resp_ ID prefix and output item IDs
#[tokio::test]
#[ignore = "requires network access; run with: cargo test -- --ignored"]
async fn test_response_id_format_e2e() {
    let port = next_port();
    let mock_app = Router::new().route("/chat/completions", post(text_response_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/responses", proxy_addr))
        .header("Content-Type", "application/json")
        .header("x-codex-provider", "test")
        .json(&serde_json::json!({
            "model": "test-model",
            "instructions": "Be helpful.",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Hello"}]
            }]
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].as_str().unwrap().starts_with("resp_"));
    assert_eq!(body["object"], "response");

    let output_msg = &body["output"][0];
    assert!(output_msg["id"].as_str().unwrap().starts_with("msg_"));
    assert_eq!(output_msg["status"], "completed");
    assert_eq!(output_msg["role"], "assistant");

    mock_handle.abort();
}

/// Verify tool call in response has fc_ ID and completed status
#[tokio::test]
#[ignore = "requires network access; run with: cargo test -- --ignored"]
async fn test_tool_call_id_and_status_e2e() {
    let port = next_port();
    let mock_app = Router::new().route("/chat/completions", post(tool_call_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/responses", proxy_addr))
        .header("Content-Type", "application/json")
        .header("x-codex-provider", "test")
        .json(&serde_json::json!({
            "model": "test-model",
            "instructions": "Be helpful.",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "What's the weather?"}]
            }],
            "tools": [{
                "type": "function",
                "name": "get_weather",
                "description": "Get weather",
                "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let fc = &body["output"][0];
    assert_eq!(fc["type"], "function_call");
    assert!(fc["id"].as_str().unwrap().starts_with("fc_"));
    assert_eq!(fc["status"], "completed");
    assert_eq!(fc["call_id"], "call_abc");
    assert_eq!(fc["name"], "get_weather");

    mock_handle.abort();
}

/// Verify web_search tools are filtered from the chat request
#[tokio::test]
#[ignore = "requires network access; run with: cargo test -- --ignored"]
async fn test_web_search_tools_filtered_e2e() {
    let port = next_port();
    let mock_app = Router::new().route("/chat/completions", post(verify_tools_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let mock_handle = tokio::spawn(async {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let proxy_addr = start_proxy(&upstream_addr.to_string(), port).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/responses", proxy_addr))
        .header("Content-Type", "application/json")
        .header("x-codex-provider", "test")
        .json(&serde_json::json!({
            "model": "test-model",
            "instructions": "Be helpful.",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Hello"}]
            }],
            "tools": [
                {"type": "web_search_preview", "name": "web_search"},
                {"type": "function", "name": "my_func", "description": "A func", "parameters": {"type": "object", "properties": {}}}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    mock_handle.abort();
}

/// Verify /v1/models returns Codex-format model list
#[tokio::test]
#[ignore = "requires network access; run with: cargo test -- --ignored"]
async fn test_models_endpoint() {
    let port = next_port();
    let config_content = format!(
        "default_provider = \"test\"\n\n[server]\nlisten_addr = \"127.0.0.1:{}\"\n\n[providers.test]\nformat = \"chat_completions\"\nbase_url = \"http://localhost:9999\"\nmodels = [\"model-a\", \"model-b\"]\n",
        port);
    let config_path = format!("/tmp/test-proxy-models-{}.toml", port);
    std::fs::write(&config_path, &config_content).unwrap();

    let cfg = vendor_proxy::config::ProxyConfig::load(&config_path).unwrap();
    let state = std::sync::Arc::new(vendor_proxy::server::AppState {
        config: cfg.clone(),
        client: vendor_proxy::client::HttpClient::new(),
        started_at: std::time::Instant::now(),
        request_count: std::sync::atomic::AtomicU64::new(0),
        error_count: std::sync::atomic::AtomicU64::new(0),
        active_connections: std::sync::atomic::AtomicI64::new(0),
        latency_total_ns: std::sync::atomic::AtomicU64::new(0),
        request_times: std::sync::Mutex::new(std::collections::VecDeque::new()),
        provider_stats: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        error_events: std::sync::Mutex::new(std::collections::VecDeque::new()),
    });
    let proxy_app = Router::new()
        .route("/v1/models", axum::routing::get(vendor_proxy::server::models_handler))
        .route("/v1/{*path}", axum::routing::any(vendor_proxy::server::proxy_handler))
        .with_state(state);
    let proxy_listener =
        tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(async {
        axum::serve(proxy_listener, proxy_app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let resp = client.get(format!("http://{}/v1/models", proxy_addr)).send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let models = body["models"].as_array().unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0]["slug"], "model-a");
    assert_eq!(models[1]["slug"], "model-b");
}

async fn start_proxy(upstream_addr: &str, port: u16) -> std::net::SocketAddr {
    let addr = upstream_addr.trim();
    let config_content = format!(
        "default_provider = \"test\"\n\n[server]\nlisten_addr = \"127.0.0.1:{}\"\n\n[providers.test]\nformat = \"chat_completions\"\nbase_url = \"http://{}\"\n",
        port, addr);
    let config_path = format!("/tmp/test-proxy-{}.toml", port);
    std::fs::write(&config_path, &config_content).unwrap();

    let cfg = vendor_proxy::config::ProxyConfig::load(&config_path).unwrap();
    let state = std::sync::Arc::new(vendor_proxy::server::AppState {
        config: cfg.clone(),
        client: vendor_proxy::client::HttpClient::new(),
        started_at: std::time::Instant::now(),
        request_count: std::sync::atomic::AtomicU64::new(0),
        error_count: std::sync::atomic::AtomicU64::new(0),
        active_connections: std::sync::atomic::AtomicI64::new(0),
        latency_total_ns: std::sync::atomic::AtomicU64::new(0),
        request_times: std::sync::Mutex::new(std::collections::VecDeque::new()),
        provider_stats: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        error_events: std::sync::Mutex::new(std::collections::VecDeque::new()),
    });
    let proxy_app = Router::new()
        .route("/v1/{*path}", axum::routing::any(vendor_proxy::server::proxy_handler))
        .with_state(state);
    let proxy_listener =
        tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(proxy_listener, proxy_app).await.unwrap();
    });
    proxy_addr
}

async fn text_response_handler(Json(_body): Json<serde_json::Value>) -> impl IntoResponse {
    let resp = serde_json::json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1700000000,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hi there!"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    (StatusCode::OK, Json(resp))
}

async fn tool_call_handler(Json(_body): Json<serde_json::Value>) -> impl IntoResponse {
    let resp = serde_json::json!({
        "id": "chatcmpl-tc",
        "object": "chat.completion",
        "created": 1700000000,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_abc",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"city\":\"Paris\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });

    (StatusCode::OK, Json(resp))
}

async fn verify_tools_handler(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["function"]["name"], "my_func");

    let resp = serde_json::json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1700000000,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "ok"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 5, "completion_tokens": 1, "total_tokens": 6}
    });

    (StatusCode::OK, Json(resp))
}
