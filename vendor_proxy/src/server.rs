//! Axum HTTP server: /health, /v1/models, /v1/responses proxy endpoint.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::{ApiFormat, ProxyPair};
use crate::adapters::anthropic::AnthropicAdapter;
use crate::adapters::chat_completions::ChatCompletionsAdapter;
use crate::adapters::{FormatAdapter, ProviderResponse};
use crate::bridge::anthropic_to_chat::anthropic_to_chat;
use crate::bridge::chat_to_anthropic::chat_to_anthropic_response;
use crate::client::{AuthHeaders, HttpClient};
use crate::config::{ProviderConfig, ProviderFormat, ProxyConfig};
use crate::model_mapper::map_model;
use crate::stream::anthropic_sse::AnthropicSseTranslator;
use crate::stream::chat_sse::ChatSseToResponsesTranslator;
use crate::stream::chat_to_anthropic_sse::ChatToAnthropicSseTranslator;
use crate::stream::parser::{SseLine, SseParser};
use crate::types::responses::ResponsesRequest;

pub struct AppState {
    pub config: ProxyConfig,
    pub client: HttpClient,
    pub started_at: Instant,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub active_connections: AtomicI64,
    pub latency_total_ns: AtomicU64,
    pub request_times: Mutex<VecDeque<Instant>>,
    pub provider_stats: Arc<std::sync::RwLock<std::collections::HashMap<String, ProviderStats>>>,
    pub error_events: Mutex<VecDeque<ErrorEvent>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStats {
    pub requests: u64,
    pub errors: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorEvent {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub status: u16,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AdminStatus {
    pub version: String,
    pub uptime_secs: u64,
    pub listen_addr: String,
    pub request_count: u64,
    pub error_count: u64,
    pub active_connections: i64,
    pub rps: f64,
    pub avg_latency_ms: f64,
    pub providers: std::collections::HashMap<String, ProviderStats>,
}

/// Determine which adapter to use based on (in order):
/// 1. x-codex-provider header
/// 2. Model name matching a provider's models list
/// 3. Default provider
fn select_adapter<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
    model: &str,
) -> (
    Arc<dyn FormatAdapter>,
    &'a crate::config::ProviderConfig,
    Option<String>,
    String,
) {
    // 1. Explicit provider header
    let header_provider =
        headers.get("x-codex-provider").and_then(|v| v.to_str().ok()).map(|s| s.to_string());

    let provider = header_provider
        .as_deref()
        .and_then(|name| state.config.provider_by_name(name))
        .or_else(|| state.config.providers.values().find(|p| p.models.iter().any(|m| m == model)))
        .unwrap_or_else(|| state.config.default_provider());

    // Resolve provider name for metrics
    let provider_name = state
        .config
        .providers
        .iter()
        .find(|(_, v)| std::ptr::eq(*v, provider))
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Prefer API key from incoming Authorization: Bearer header, fall back to env var
    let api_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| provider.api_key_env.as_ref().and_then(|env_key| std::env::var(env_key).ok()));

    let adapter: Arc<dyn FormatAdapter> = match provider.format {
        ProviderFormat::ChatCompletions => Arc::new(ChatCompletionsAdapter),
        ProviderFormat::AnthropicMessages => Arc::new(AnthropicAdapter),
    };

    (adapter, provider, api_key, provider_name)
}

/// GET /admin/status — runtime metrics for monitoring.
pub async fn admin_status(State(state): State<Arc<AppState>>) -> Json<AdminStatus> {
    let providers = state
        .provider_stats
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    // RPS: count requests in the last second
    let now = Instant::now();
    let rps = {
        let mut times = state.request_times.lock().unwrap_or_else(|e| e.into_inner());
        // Prune old entries (> 1s)
        while times.front().map_or(false, |t| now.duration_since(*t).as_millis() > 1000) {
            times.pop_front();
        }
        times.len() as f64
    };

    // Avg latency
    let total_ns = state.latency_total_ns.load(Ordering::Relaxed);
    let total_reqs = state.request_count.load(Ordering::Relaxed);
    let avg_latency_ms = if total_reqs > 0 {
        (total_ns as f64) / (total_reqs as f64) / 1_000_000.0
    } else {
        0.0
    };

    Json(AdminStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        listen_addr: state.config.server.listen_addr.clone(),
        request_count: total_reqs,
        error_count: state.error_events.lock().unwrap_or_else(|e| e.into_inner()).len() as u64,
        active_connections: state.active_connections.load(Ordering::Relaxed),
        rps,
        avg_latency_ms,
        providers,
    })
}

pub async fn admin_errors(State(state): State<Arc<AppState>>) -> Json<Vec<ErrorEvent>> {
    let events = state.error_events.lock().unwrap_or_else(|e| e.into_inner());
    Json(events.clone().into_iter().collect())
}

pub async fn health() -> impl IntoResponse {
    StatusCode::OK
}

/// GET /v1/models — Codex-compatible model list.
/// Returns ModelsResponse format: { "models": [ModelInfo, ...] }
pub async fn models_handler(State(state): State<Arc<AppState>>) -> Response {
    tracing::info!("GET /v1/models — returning model list");
    let mut model_set = std::collections::HashSet::new();

    // New format: collect unique values from all pair model_maps
    if state.config.uses_pairs() {
        for pair in state.config.proxy_pairs.values() {
            for model in pair.model_map.values() {
                model_set.insert(model.clone());
            }
        }
    }

    // Legacy format: collect from provider models list
    for p in state.config.providers.values() {
        for name in &p.models {
            model_set.insert(name.clone());
        }
    }

    let models: Vec<_> = model_set.into_iter().map(|name| {
        serde_json::json!({
            "slug": name,
            "display_name": name,
            "shell_type": "default",
            "visibility": "list",
            "supported_in_api": true,
            "priority": 100,
            "context_window": 128000,
            "max_context_window": 128000,
            "supports_parallel_tool_calls": true,
            "supports_reasoning_summaries": false,
            "truncation_policy": { "type": "tokens", "limit": 10000 },
            "effective_context_window_percent": 95,
            "web_search_tool_type": "text",
        })
    }).collect();

    let body = serde_json::json!({ "models": models });
    (
        StatusCode::OK,
        serde_json::to_string(&body)
            .unwrap_or_else(|_| r#"{"error":"json serialize"}"#.to_string()),
    )
        .into_response()
}

pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    body: String,
) -> Response {
    if path != "responses" {
        tracing::info!("Unknown path: {}", path);
        return (StatusCode::NOT_FOUND, "endpoint not found").into_response();
    }

    let responses_req: ResponsesRequest = match serde_json::from_str(&body) {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("Failed to parse request: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Invalid request: {}", e)).into_response();
        }
    };

    tracing::info!("Request: stream={}, model={}", responses_req.stream, responses_req.model);
    // Dump input items to diagnose reasoning_content issue
    if let serde_json::Value::Array(ref items) = responses_req.input {
        let types: Vec<String> = items
            .iter()
            .filter_map(|it| {
                let t = it.get("type").and_then(|v| v.as_str())?;
                if t == "message" {
                    let role = it.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                    Some(format!("message[{}]", role))
                } else {
                    Some(t.to_string())
                }
            })
            .collect();
        tracing::info!("Input item types: {:?}", types);

        // Sniff reasoning content in message items with role "assistant"
        for it in items {
            if it.get("type").and_then(|v| v.as_str()) == Some("message")
                && it.get("role").and_then(|v| v.as_str()) == Some("assistant")
            {
                if let Some(content) = it.get("content").and_then(|v| v.as_array()) {
                    let content_types: Vec<&str> = content
                        .iter()
                        .filter_map(|b| b.get("type").and_then(|v| v.as_str()))
                        .collect();
                    if !content_types.is_empty() {
                        tracing::info!("  assistant content types: {:?}", content_types);
                    }
                }
            }
        }
    }

    let original_model = responses_req.model.clone();
    let (adapter, provider, api_key, provider_name) =
        select_adapter(&state, &headers, &original_model);

    // Map model name if provider has a model_map entry
    let resolved_model = provider
        .model_map
        .get(&original_model)
        .cloned()
        .unwrap_or(original_model);
    let mut req = responses_req;
    req.model = resolved_model;

    tracing::info!(
        "Routed to provider: name={}, base_url={}, format={:?}, has_api_key={}",
        provider_name,
        provider.base_url,
        provider.format,
        api_key.is_some()
    );

    state.request_count.fetch_add(1, Ordering::Relaxed);
    state.active_connections.fetch_add(1, Ordering::Relaxed);
    let request_start = Instant::now();

    // Record request timestamp for RPS calculation
    {
        let mut times = state.request_times.lock().unwrap_or_else(|e| e.into_inner());
        times.push_back(request_start);
        // Keep max 200 entries to bound memory
        while times.len() > 200 {
            times.pop_front();
        }
    }

    // Track per-provider stats
    {
        let mut stats = state
            .provider_stats
            .write()
            .unwrap_or_else(|e| e.into_inner());
        let entry = stats.entry(provider_name.clone()).or_insert(ProviderStats {
            requests: 0,
            errors: 0,
            last_error: None,
        });
        entry.requests += 1;
    }

    let result = adapter
        .send(&state.client, &provider.base_url, api_key.as_deref(), &req)
        .await;

    let elapsed_ns = request_start.elapsed().as_nanos() as u64;
    state.latency_total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
    state.active_connections.fetch_sub(1, Ordering::Relaxed);

    match result
    {
        Ok(ProviderResponse::Complete(resp)) => {
            let json = serde_json::to_string(&*resp).expect("json serialize");
            tracing::info!("Response (complete): {} bytes, {}ms", json.len(), elapsed_ns / 1_000_000);
            (StatusCode::OK, json).into_response()
        }
        Ok(ProviderResponse::Stream(rx)) => {
            tracing::info!("Response: streaming");
            let provider_format = provider.format.clone();
            let stream = async_stream::stream! {
                let mut raw_parser = SseParser::new();
                let mut anthropic_translator = if provider_format == ProviderFormat::AnthropicMessages {
                    Some(AnthropicSseTranslator::new())
                } else {
                    None
                };
                let mut responses_translator = ChatSseToResponsesTranslator::new(Some(&req));
                let mut rx = rx;

                while let Some(raw_line) = rx.recv().await {
                    match raw_parser.feed(&raw_line) {
                        Some(SseLine::Data(data)) => {
                            if let Some(ref mut at) = anthropic_translator {
                                match serde_json::from_str::<crate::types::anthropic::AnthropicSseEvent>(data) {
                                    Ok(event) => {
                                        let chat_lines = at.feed(event);
                                        for chat_line in chat_lines {
                                            let resp_events = responses_translator.feed(&chat_line);
                                            for event in resp_events {
                                                tracing::trace!("SSE out: {}", event);
                                                if let Some(data_part) = event.strip_prefix("data: ") {
                                                    yield Ok::<_, std::convert::Infallible>(Event::default().data(data_part));
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        let resp_events = responses_translator.feed(data);
                                        for event in resp_events {
                                            tracing::trace!("SSE out: {}", event);
                                            if let Some(data_part) = event.strip_prefix("data: ") {
                                                yield Ok::<_, std::convert::Infallible>(Event::default().data(data_part));
                                            }
                                        }
                                    }
                                }
                            } else {
                                let resp_events = responses_translator.feed(data);
                                for event in resp_events {
                                    tracing::trace!("SSE out: {}", event);
                                    if let Some(data_part) = event.strip_prefix("data: ") {
                                        yield Ok::<_, std::convert::Infallible>(Event::default().data(data_part));
                                    }
                                }
                            }
                        }
                        Some(SseLine::Done) => {
                            tracing::info!("SSE stream: [DONE] received");
                            break;
                        }
                        _ => {}
                    }
                }
                tracing::info!("SSE stream: emitting [DONE] sentinel");
                yield Ok(Event::default().data("[DONE]"));
            };

            Sse::new(stream).into_response()
        }
        Err(e) => {
            state.error_count.fetch_add(1, Ordering::Relaxed);
            {
                let mut events = state.error_events.lock().unwrap_or_else(|e| e.into_inner());
                events.push_front(ErrorEvent {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    provider: provider_name.clone(),
                    model: req.model.clone(),
                    status: match &e {
                        crate::client::ProxyError::ProviderError { status, .. } => *status,
                        _ => 500,
                    },
                    message: e.to_string(),
                });
                while events.len() > 100 {
                    events.pop_back();
                }
            }
            // Track per-provider error
            if let Ok(mut stats) = state.provider_stats.write() {
                if let Some(entry) = stats.get_mut(&provider_name) {
                    entry.errors += 1;
                    entry.last_error = Some(e.to_string());
                }
            }
            tracing::error!("Provider error: {}", e);
            let status = match &e {
                crate::client::ProxyError::ProviderError { status, .. } => {
                    StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            let body = serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "proxy_error"
                }
            });
            (
                status,
                serde_json::to_string(&body)
                    .unwrap_or_else(|_| r#"{"error":"json serialize"}"#.to_string()),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Anthropic Messages handler (/v1/messages)
// ---------------------------------------------------------------------------

/// Select a provider by model name. Returns (provider_config, api_key, provider_name).
fn select_provider<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
    model: &str,
) -> (&'a ProviderConfig, Option<String>, String) {
    let header_provider =
        headers.get("x-codex-provider").and_then(|v| v.to_str().ok()).map(|s| s.to_string());

    let provider = header_provider
        .as_deref()
        .and_then(|name| state.config.provider_by_name(name))
        .or_else(|| state.config.providers.values().find(|p| p.models.iter().any(|m| m == model)))
        .unwrap_or_else(|| state.config.default_provider());

    let provider_name = state
        .config
        .providers
        .iter()
        .find(|(_, v)| std::ptr::eq(*v, provider))
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Auth: prefer x-api-key, then Authorization: Bearer, then env var
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        })
        .or_else(|| provider.api_key_env.as_ref().and_then(|env_key| std::env::var(env_key).ok()));

    (provider, api_key, provider_name)
}

/// POST /v1/messages — Anthropic Messages API handler.
///
/// Supports two paths:
/// - Anthropic backend: passthrough with model/auth replacement
/// - Chat backend: translate Anthropic → Chat → send → Chat → Anthropic
pub async fn anthropic_messages_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let req_body: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)).into_response();
        }
    };

    let original_model = req_body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (provider, api_key, provider_name) = select_provider(&state, &headers, &original_model);

    // Map model name
    let mapped_model = map_model(&original_model, &provider.model_map);

    tracing::info!(
        "[/v1/messages] model: {} → {}, provider: {}, format: {:?}",
        original_model, mapped_model, provider_name, provider.format
    );

    let is_stream = req_body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    state.request_count.fetch_add(1, Ordering::Relaxed);
    state.active_connections.fetch_add(1, Ordering::Relaxed);
    let request_start = Instant::now();

    // Track per-provider stats
    {
        let mut stats = state.provider_stats.write().unwrap_or_else(|e| e.into_inner());
        let entry = stats.entry(provider_name.clone()).or_insert(ProviderStats {
            requests: 0,
            errors: 0,
            last_error: None,
        });
        entry.requests += 1;
    }

    if provider.format == ProviderFormat::ChatCompletions {
        // Chat backend: translate Anthropic → Chat → send → Chat → Anthropic
        handle_anthropic_to_chat(&state, &provider, api_key.as_deref(), &req_body, &mapped_model, is_stream, request_start).await
    } else {
        // Anthropic backend: passthrough
        handle_anthropic_passthrough(&state, &provider, api_key.as_deref(), &req_body, &mapped_model, is_stream, request_start).await
    }
}

/// Anthropic passthrough: forward to Anthropic backend with model/auth replacement.
async fn handle_anthropic_passthrough(
    state: &AppState,
    provider: &ProviderConfig,
    api_key: Option<&str>,
    req_body: &serde_json::Value,
    mapped_model: &str,
    is_stream: bool,
    request_start: Instant,
) -> Response {
    let mut body = req_body.clone();
    body["model"] = serde_json::json!(mapped_model);

    let upstream_url = format!("{}/v1/messages", provider.base_url.trim_end_matches('/'));
    let auth = AuthHeaders::anthropic(api_key);

    if is_stream {
        match state.client.post_streaming_with_headers(&upstream_url, &auth, &body).await {
            Ok(rx) => {
                let stream = async_stream::stream! {
                    let mut rx = rx;
                    while let Some(line) = rx.recv().await {
                        tracing::trace!("[SSE passthrough] {}", line);
                        yield Ok::<_, std::convert::Infallible>(Event::default().data(line));
                    }
                };
                Sse::new(stream).into_response()
            }
            Err(e) => {
                state.error_count.fetch_add(1, Ordering::Relaxed);
                state.active_connections.fetch_sub(1, Ordering::Relaxed);
                tracing::error!("[/v1/messages] passthrough stream error: {}", e);
                error_response(e)
            }
        }
    } else {
        let result: Result<serde_json::Value, crate::client::ProxyError> = state
            .client
            .post_json_with_headers(&upstream_url, &auth, &body)
            .await;

        state.latency_total_ns.fetch_add(request_start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        state.active_connections.fetch_sub(1, Ordering::Relaxed);

        match result {
            Ok(resp) => {
                let json = serde_json::to_string(&resp).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string());
                (StatusCode::OK, json).into_response()
            }
            Err(e) => {
                state.error_count.fetch_add(1, Ordering::Relaxed);
                tracing::error!("[/v1/messages] passthrough error: {}", e);
                error_response(e)
            }
        }
    }
}

/// Chat backend: translate Anthropic Messages → Chat Completions → send → translate back.
async fn handle_anthropic_to_chat(
    state: &AppState,
    provider: &ProviderConfig,
    api_key: Option<&str>,
    req_body: &serde_json::Value,
    mapped_model: &str,
    is_stream: bool,
    request_start: Instant,
) -> Response {
    // Parse Anthropic request
    let mut anthro_req: crate::types::anthropic::AnthropicRequest = match serde_json::from_value(req_body.clone()) {
        Ok(r) => r,
        Err(e) => {
            state.active_connections.fetch_sub(1, Ordering::Relaxed);
            return (StatusCode::BAD_REQUEST, format!("Invalid Anthropic request: {}", e)).into_response();
        }
    };
    anthro_req.model = mapped_model.to_string();

    // Convert to Chat Completions request
    let chat_req = anthropic_to_chat(&anthro_req);

    let upstream_url = format!("{}/v1/chat/completions", provider.base_url.trim_end_matches('/'));
    let auth = AuthHeaders::bearer(api_key);

    if is_stream {
        match state.client.post_streaming_with_headers(&upstream_url, &auth, &chat_req).await {
            Ok(rx) => {
                let stream = async_stream::stream! {
                    let mut translator = ChatToAnthropicSseTranslator::new();
                    let mut rx = rx;
                    while let Some(line) = rx.recv().await {
                        let events = translator.feed(&line);
                        for event in events {
                            if let Some(data_part) = event.strip_prefix("data: ") {
                                yield Ok::<_, std::convert::Infallible>(Event::default().data(data_part));
                            }
                        }
                    }
                };
                Sse::new(stream).into_response()
            }
            Err(e) => {
                state.error_count.fetch_add(1, Ordering::Relaxed);
                state.active_connections.fetch_sub(1, Ordering::Relaxed);
                tracing::error!("[/v1/messages] chat stream error: {}", e);
                error_response(e)
            }
        }
    } else {
        let result: Result<crate::types::chat::ChatCompletionResponse, crate::client::ProxyError> =
            state.client.post_json_with_headers(&upstream_url, &auth, &chat_req).await;

        state.latency_total_ns.fetch_add(request_start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        state.active_connections.fetch_sub(1, Ordering::Relaxed);

        match result {
            Ok(chat_resp) => {
                let anthro_resp = chat_to_anthropic_response(&chat_resp);
                let json = serde_json::to_string(&anthro_resp).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string());
                (StatusCode::OK, json).into_response()
            }
            Err(e) => {
                state.error_count.fetch_add(1, Ordering::Relaxed);
                tracing::error!("[/v1/messages] chat error: {}", e);
                error_response(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pair-based handler (proxy_pairs format)
// ---------------------------------------------------------------------------

/// Generic handler for proxy_pairs routes.
/// Uses OriginalUri to determine which pair matched, then dispatches by format.
pub async fn pair_handler(
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let path = uri.path();

    // Find matching pair by route_path prefix
    let (pair_name, pair) = match state.config.proxy_pairs.iter().find(|(_, p)| {
        let prefix = p.route_path.trim_end_matches('/');
        path.starts_with(&format!("{}/", prefix)) || path == prefix
    }) {
        Some((n, p)) => (n.clone(), p.clone()),
        None => return (StatusCode::NOT_FOUND, "no matching proxy pair").into_response(),
    };

    // Resolve API key: prefer incoming headers, fall back to env var
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        })
        .or_else(|| pair.api_key_env.as_ref().and_then(|k| std::env::var(k).ok()));

    state.request_count.fetch_add(1, Ordering::Relaxed);
    state.active_connections.fetch_add(1, Ordering::Relaxed);
    let request_start = Instant::now();

    // Record for RPS
    {
        let mut times = state.request_times.lock().unwrap_or_else(|e| e.into_inner());
        times.push_back(request_start);
        while times.len() > 200 {
            times.pop_front();
        }
    }

    // Per-pair stats
    {
        let mut stats = state.provider_stats.write().unwrap_or_else(|e| e.into_inner());
        stats.entry(pair_name.clone()).or_insert(ProviderStats {
            requests: 0, errors: 0, last_error: None,
        }).requests += 1;
    }

    tracing::info!(
        "[pair:{}] {} → {}, route={:?} provider={:?}",
        pair_name, path, pair.provider_base_url, pair.route_format, pair.provider_format
    );

    let result = dispatch_pair(&state, &pair, api_key.as_deref(), &body).await;

    let elapsed_ns = request_start.elapsed().as_nanos() as u64;
    state.latency_total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
    state.active_connections.fetch_sub(1, Ordering::Relaxed);

    match result {
        Ok(resp) => resp,
        Err(e) => {
            state.error_count.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut stats) = state.provider_stats.write() {
                if let Some(entry) = stats.get_mut(&pair_name) {
                    entry.errors += 1;
                    entry.last_error = Some(e.to_string());
                }
            }
            {
                let mut events = state.error_events.lock().unwrap_or_else(|e| e.into_inner());
                events.push_front(ErrorEvent {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    provider: pair_name,
                    model: "unknown".to_string(),
                    status: match &e {
                        crate::client::ProxyError::ProviderError { status, .. } => *status,
                        _ => 500,
                    },
                    message: e.to_string(),
                });
                while events.len() > 100 { events.pop_back(); }
            }
            error_response(e)
        }
    }
}

/// Dispatch based on (route_format, provider_format) combination.
async fn dispatch_pair(
    state: &AppState,
    pair: &ProxyPair,
    api_key: Option<&str>,
    body: &str,
) -> Result<Response, crate::client::ProxyError> {
    match (&pair.route_format, &pair.provider_format) {
        // Anthropic → Anthropic: passthrough
        (ApiFormat::AnthropicMessages, ApiFormat::AnthropicMessages) => {
            pair_anthropic_passthrough(state, pair, api_key, body).await
        }
        // Anthropic → Chat: translate
        (ApiFormat::AnthropicMessages, ApiFormat::ChatCompletions) => {
            pair_anthropic_to_chat(state, pair, api_key, body).await
        }
        // Responses → Chat: use existing adapter pipeline
        (ApiFormat::Responses, ApiFormat::ChatCompletions) => {
            pair_responses_to_chat(state, pair, api_key, body).await
        }
        // Chat → Chat: passthrough
        (ApiFormat::ChatCompletions, ApiFormat::ChatCompletions) => {
            pair_chat_passthrough(state, pair, api_key, body).await
        }
        // Unsupported combinations are rejected at config validation
        _ => unreachable!(),
    }
}

/// Anthropic → Anthropic passthrough: forward with model/auth replacement.
async fn pair_anthropic_passthrough(
    state: &AppState,
    pair: &ProxyPair,
    api_key: Option<&str>,
    body: &str,
) -> Result<Response, crate::client::ProxyError> {
    let mut req: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| crate::client::ProxyError::UpstreamError(format!("Invalid JSON: {}", e)))?;

    let original_model = req.get("model").and_then(|v: &serde_json::Value| v.as_str()).unwrap_or("").to_string();
    let mapped = map_model(&original_model, &pair.model_map);
    req["model"] = serde_json::json!(mapped);

    let is_stream = req.get("stream").and_then(|v: &serde_json::Value| v.as_bool()).unwrap_or(false);
    let upstream_url = format!("{}/v1/messages", pair.provider_base_url.trim_end_matches('/'));
    let auth = AuthHeaders::anthropic(api_key);

    if is_stream {
        let rx = state.client.post_streaming_with_headers(&upstream_url, &auth, &req).await?;
        let model_to_restore = original_model.clone();
        let mapped_model = mapped.clone();
        let stream = async_stream::stream! {
            let mut rx = rx;
            while let Some(line) = rx.recv().await {
                // Replace mapped model back to original in SSE data lines
                let patched = if line.contains(&mapped_model) {
                    line.replace(&mapped_model, &model_to_restore)
                } else {
                    line
                };
                yield Ok::<_, std::convert::Infallible>(Event::default().data(patched));
            }
        };
        Ok(Sse::new(stream).into_response())
    } else {
        let mut resp: serde_json::Value = state.client.post_json_with_headers(&upstream_url, &auth, &req).await?;
        // Replace model in response back to original
        if let Some(m) = resp.get_mut("model").and_then(|v| v.as_str()) {
            if m == mapped {
                resp["model"] = serde_json::json!(original_model);
            }
        }
        let json = serde_json::to_string(&resp).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string());
        Ok((StatusCode::OK, json).into_response())
    }
}

/// Anthropic → Chat Completions: translate both directions.
async fn pair_anthropic_to_chat(
    state: &AppState,
    pair: &ProxyPair,
    api_key: Option<&str>,
    body: &str,
) -> Result<Response, crate::client::ProxyError> {
    let mut anthro_req: crate::types::anthropic::AnthropicRequest = serde_json::from_str(body)
        .map_err(|e| crate::client::ProxyError::UpstreamError(format!("Invalid Anthropic request: {}", e)))?;

    let original_model = anthro_req.model.clone();
    anthro_req.model = map_model(&original_model, &pair.model_map);

    let is_stream = anthro_req.stream;
    let chat_req = anthropic_to_chat(&anthro_req);
    let upstream_url = format!("{}/v1/chat/completions", pair.provider_base_url.trim_end_matches('/'));
    let auth = AuthHeaders::bearer(api_key);

    if is_stream {
        let rx = state.client.post_streaming_with_headers(&upstream_url, &auth, &chat_req).await?;
        let stream = async_stream::stream! {
            let mut translator = ChatToAnthropicSseTranslator::new();
            let mut rx = rx;
            while let Some(line) = rx.recv().await {
                let events = translator.feed(&line);
                for event in events {
                    if let Some(data_part) = event.strip_prefix("data: ") {
                        yield Ok::<_, std::convert::Infallible>(Event::default().data(data_part));
                    }
                }
            }
        };
        Ok(Sse::new(stream).into_response())
    } else {
        let chat_resp: crate::types::chat::ChatCompletionResponse =
            state.client.post_json_with_headers(&upstream_url, &auth, &chat_req).await?;
        let anthro_resp = chat_to_anthropic_response(&chat_resp);
        let json = serde_json::to_string(&anthro_resp).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string());
        Ok((StatusCode::OK, json).into_response())
    }
}

/// Responses → Chat Completions: use existing adapter pipeline.
async fn pair_responses_to_chat(
    state: &AppState,
    pair: &ProxyPair,
    api_key: Option<&str>,
    body: &str,
) -> Result<Response, crate::client::ProxyError> {
    let mut req: ResponsesRequest = serde_json::from_str(body)
        .map_err(|e| crate::client::ProxyError::UpstreamError(format!("Invalid request: {}", e)))?;

    let original_model = req.model.clone();
    req.model = map_model(&original_model, &pair.model_map);

    let adapter = ChatCompletionsAdapter;
    let result = adapter.send(&state.client, &pair.provider_base_url, api_key, &req).await;

    match result {
        Ok(ProviderResponse::Complete(resp)) => {
            let json = serde_json::to_string(&*resp).expect("json serialize");
            Ok((StatusCode::OK, json).into_response())
        }
        Ok(ProviderResponse::Stream(rx)) => {
            let stream = async_stream::stream! {
                let mut raw_parser = SseParser::new();
                let mut responses_translator = ChatSseToResponsesTranslator::new(None);
                let mut rx = rx;
                while let Some(raw_line) = rx.recv().await {
                    match raw_parser.feed(&raw_line) {
                        Some(SseLine::Data(data)) => {
                            let resp_events = responses_translator.feed(data);
                            for event in resp_events {
                                if let Some(data_part) = event.strip_prefix("data: ") {
                                    yield Ok::<_, std::convert::Infallible>(Event::default().data(data_part));
                                }
                            }
                        }
                        Some(SseLine::Done) => break,
                        _ => {}
                    }
                }
                yield Ok(Event::default().data("[DONE]"));
            };
            Ok(Sse::new(stream).into_response())
        }
        Err(e) => Err(e),
    }
}

/// Chat → Chat passthrough: forward as-is with model mapping.
async fn pair_chat_passthrough(
    state: &AppState,
    pair: &ProxyPair,
    api_key: Option<&str>,
    body: &str,
) -> Result<Response, crate::client::ProxyError> {
    let mut req: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| crate::client::ProxyError::UpstreamError(format!("Invalid JSON: {}", e)))?;

    let original_model = req.get("model").and_then(|v: &serde_json::Value| v.as_str()).unwrap_or("").to_string();
    let mapped = map_model(&original_model, &pair.model_map);
    req["model"] = serde_json::json!(mapped);

    let is_stream = req.get("stream").and_then(|v: &serde_json::Value| v.as_bool()).unwrap_or(false);
    let upstream_url = format!("{}/v1/chat/completions", pair.provider_base_url.trim_end_matches('/'));
    let auth = AuthHeaders::bearer(api_key);

    if is_stream {
        let rx = state.client.post_streaming_with_headers(&upstream_url, &auth, &req).await?;
        let stream = async_stream::stream! {
            let mut rx = rx;
            while let Some(line) = rx.recv().await {
                yield Ok::<_, std::convert::Infallible>(Event::default().data(line));
            }
        };
        Ok(Sse::new(stream).into_response())
    } else {
        let resp: serde_json::Value = state.client.post_json_with_headers(&upstream_url, &auth, &req).await?;
        let json = serde_json::to_string(&resp).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string());
        Ok((StatusCode::OK, json).into_response())
    }
}

fn error_response(e: crate::client::ProxyError) -> Response {
    let status = match &e {
        crate::client::ProxyError::ProviderError { status, .. } => {
            StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let body = serde_json::json!({
        "error": { "message": e.to_string(), "type": "proxy_error" }
    });
    (
        status,
        serde_json::to_string(&body).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string()),
    )
        .into_response()
}
