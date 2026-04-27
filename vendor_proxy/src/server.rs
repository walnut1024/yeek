use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
};
use std::sync::Arc;

use crate::adapters::anthropic::AnthropicAdapter;
use crate::adapters::chat_completions::ChatCompletionsAdapter;
use crate::adapters::{FormatAdapter, ProviderResponse};
use crate::client::HttpClient;
use crate::config::{ProviderFormat, ProxyConfig};
use crate::stream::anthropic_sse::AnthropicSseTranslator;
use crate::stream::chat_sse::ChatSseToResponsesTranslator;
use crate::stream::parser::{SseLine, SseParser};
use crate::types::responses::ResponsesRequest;

pub struct AppState {
    pub config: ProxyConfig,
    pub client: HttpClient,
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
) {
    // 1. Explicit provider header
    let provider_name = headers
        .get("x-codex-provider")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let provider = provider_name
        .as_deref()
        .and_then(|name| state.config.provider_by_name(name))
        .or_else(|| {
            state.config.providers.values().find(|p| {
                p.models.iter().any(|m| m == model)
            })
        })
        .unwrap_or_else(|| state.config.default_provider());

    // Prefer API key from incoming Authorization: Bearer header, fall back to env var
    let api_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            provider
                .api_key_env
                .as_ref()
                .and_then(|env_key| std::env::var(env_key).ok())
        });

    let adapter: Arc<dyn FormatAdapter> = match provider.format {
        ProviderFormat::ChatCompletions => Arc::new(ChatCompletionsAdapter),
        ProviderFormat::AnthropicMessages => Arc::new(AnthropicAdapter),
    };

    (adapter, provider, api_key)
}

pub async fn health() -> impl IntoResponse {
    StatusCode::OK
}

/// GET /v1/models — Codex-compatible model list.
/// Returns ModelsResponse format: { "models": [ModelInfo, ...] }
pub async fn models_handler(State(state): State<Arc<AppState>>) -> Response {
    tracing::info!("GET /v1/models — returning model list");
    let mut models = Vec::new();
    for (_name, p) in &state.config.providers {
        let model_names: Vec<&str> = if p.models.is_empty() {
            vec![]
        } else {
            p.models.iter().map(|s| s.as_str()).collect()
        };
        for name in model_names {
            models.push(serde_json::json!({
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
            }));
        }
    }

    let body = serde_json::json!({ "models": models });
    (StatusCode::OK, serde_json::to_string(&body).unwrap()).into_response()
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
        let types: Vec<String> = items.iter()
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
            if it.get("type").and_then(|v| v.as_str()) == Some("message") {
                if it.get("role").and_then(|v| v.as_str()) == Some("assistant") {
                    if let Some(content) = it.get("content").and_then(|v| v.as_array()) {
                        let content_types: Vec<&str> = content.iter()
                            .filter_map(|b| b.get("type").and_then(|v| v.as_str()))
                            .collect();
                        if !content_types.is_empty() {
                            tracing::info!("  assistant content types: {:?}", content_types);
                        }
                    }
                }
            }
        }
    }

    let (adapter, provider, api_key) = select_adapter(&state, &headers, &responses_req.model);
    tracing::info!(
        "Routed to provider: base_url={}, format={:?}, has_api_key={}",
        provider.base_url,
        provider.format,
        api_key.is_some()
    );

    match adapter
        .send(
            &state.client,
            &provider.base_url,
            api_key.as_deref(),
            &responses_req,
        )
        .await
    {
        Ok(ProviderResponse::Complete(resp)) => {
            let json = serde_json::to_string(&*resp).unwrap();
            tracing::info!("Response (complete): {} bytes", json.len());
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
                let mut responses_translator = ChatSseToResponsesTranslator::new(Some(&responses_req));
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
            (status, serde_json::to_string(&body).unwrap()).into_response()
        }
    }
}
