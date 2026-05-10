//! Axum handlers for the bridge-based proxy.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use crate::client::HttpClient;
use crate::config::ProxyConfig;
use crate::logging::{header_names, request_body_summary, RequestLogContext};
use crate::model::ModelPolicy;
use crate::pipeline::{error_response, execute_bridge, PipelineError};
use crate::routing::{find_bridge_for_models_path, find_bridge_for_request_path};

pub struct AppState {
    pub config: ProxyConfig,
    pub client: HttpClient,
    pub started_at: Instant,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub active_connections: AtomicI64,
    pub latency_total_ns: AtomicU64,
    pub request_times: Mutex<VecDeque<Instant>>,
    pub provider_stats: Arc<RwLock<HashMap<String, ProviderStats>>>,
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
    pub providers: HashMap<String, ProviderStats>,
}

pub async fn health() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn admin_status(State(state): State<Arc<AppState>>) -> Json<AdminStatus> {
    let providers = state.provider_stats.read().unwrap_or_else(|e| e.into_inner()).clone();
    let now = Instant::now();
    let rps = {
        let mut times = state.request_times.lock().unwrap_or_else(|e| e.into_inner());
        while times.front().is_some_and(|t| now.duration_since(*t).as_millis() > 1000) {
            times.pop_front();
        }
        times.len() as f64
    };
    let total_ns = state.latency_total_ns.load(Ordering::Relaxed);
    let total_reqs = state.request_count.load(Ordering::Relaxed);
    let avg_latency_ms =
        if total_reqs > 0 { total_ns as f64 / total_reqs as f64 / 1_000_000.0 } else { 0.0 };

    Json(AdminStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        listen_addr: state.config.server.listen_addr.clone(),
        request_count: total_reqs,
        error_count: state.error_count.load(Ordering::Relaxed),
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

pub async fn global_models_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut model_set = HashSet::new();
    for bridge in state.config.bridges.values() {
        for model in bridge.models.keys() {
            model_set.insert(model.clone());
        }
    }
    models_response(model_set)
}

pub async fn bridge_models_handler(
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    State(state): State<Arc<AppState>>,
) -> Response {
    let Some((bridge_name, bridge)) = find_bridge_for_models_path(&state.config, uri.path()) else {
        return (StatusCode::NOT_FOUND, "no matching bridge").into_response();
    };
    tracing::info!("[bridge:{}] GET {}", bridge_name, uri.path());
    let models = ModelPolicy::new(bridge_name, &bridge.models).agent_models().into_iter().collect();
    models_response(models)
}

fn models_response(model_set: HashSet<String>) -> Response {
    let mut names: Vec<_> = model_set.into_iter().collect();
    names.sort();
    let models: Vec<_> = names
        .into_iter()
        .map(|name| {
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
        })
        .collect();
    let body = serde_json::json!({ "models": models });
    (
        StatusCode::OK,
        serde_json::to_string(&body)
            .unwrap_or_else(|_| r#"{"error":"json serialize"}"#.to_string()),
    )
        .into_response()
}

pub async fn bridge_handler(
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let Some((bridge_name, bridge, provider)) =
        find_bridge_for_request_path(&state.config, uri.path())
    else {
        return (StatusCode::NOT_FOUND, "no matching bridge").into_response();
    };

    let request_start = Instant::now();
    let request_id = state.request_count.fetch_add(1, Ordering::Relaxed) + 1;
    state.active_connections.fetch_add(1, Ordering::Relaxed);
    let log_ctx = RequestLogContext::new(
        request_id,
        bridge_name,
        &bridge.agent.base_url,
        &bridge.provider.name,
    );
    {
        let mut times = state.request_times.lock().unwrap_or_else(|e| e.into_inner());
        times.push_back(request_start);
        while times.len() > 200 {
            times.pop_front();
        }
    }
    {
        let mut stats = state.provider_stats.write().unwrap_or_else(|e| e.into_inner());
        stats
            .entry(bridge_name.to_string())
            .or_insert(ProviderStats { requests: 0, errors: 0, last_error: None })
            .requests += 1;
    }

    tracing::info!(
        request_id,
        bridge = %bridge_name,
        agent_base_url = %bridge.agent.base_url,
        provider = %bridge.provider.name,
        request_path = %uri.path(),
        agent_format = ?bridge.agent.api_format,
        provider_format = ?provider.api_format,
        body_bytes = body.len(),
        "request start"
    );
    tracing::debug!(
        request_id,
        bridge = %bridge_name,
        agent_base_url = %bridge.agent.base_url,
        provider = %bridge.provider.name,
        header_names = %header_names(&headers),
        body_summary = %request_body_summary(&body),
        "inbound request summary"
    );

    let result =
        execute_bridge(&state.client, &log_ctx, bridge_name, bridge, provider, &body).await;
    let elapsed_ns = request_start.elapsed().as_nanos() as u64;
    let elapsed_ms = request_start.elapsed().as_millis();
    state.latency_total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
    state.active_connections.fetch_sub(1, Ordering::Relaxed);

    match result {
        Ok(response) => {
            tracing::info!(
                request_id,
                bridge = %bridge_name,
                agent_base_url = %bridge.agent.base_url,
                provider = %bridge.provider.name,
                elapsed_ms,
                "request response ready"
            );
            response
        }
        Err(error) => {
            tracing::error!(
                request_id,
                bridge = %bridge_name,
                agent_base_url = %bridge.agent.base_url,
                provider = %bridge.provider.name,
                status = error.status().as_u16(),
                error_type = %error.error_type(),
                elapsed_ms,
                error_message = %error,
                "request failed"
            );
            record_error(&state, bridge_name, &error);
            error_response(error)
        }
    }
}

fn record_error(state: &AppState, bridge_name: &str, error: &PipelineError) {
    state.error_count.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut stats) = state.provider_stats.write() {
        if let Some(entry) = stats.get_mut(bridge_name) {
            entry.errors += 1;
            entry.last_error = Some(error.to_string());
        }
    }

    let mut events = state.error_events.lock().unwrap_or_else(|e| e.into_inner());
    events.push_front(ErrorEvent {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        provider: bridge_name.to_string(),
        model: "unknown".to_string(),
        status: error.status().as_u16(),
        message: error.to_string(),
    });
    while events.len() > 100 {
        events.pop_back();
    }
}
