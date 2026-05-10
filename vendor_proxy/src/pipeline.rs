//! Compiled bridge request pipeline.

use axum::{
    http::{header, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
};
use serde_json::Value;
use std::time::Instant;

use crate::auth::{provider_api_key, provider_auth};
use crate::client::{HttpClient, ProxyError};
use crate::config::{ApiFormat, BridgeConfig, ProviderConfig};
use crate::logging::{request_body_summary, RequestLogContext};
use crate::model::{restore_model_fields, restore_model_in_sse_data, ModelPolicy, UnknownModel};
use crate::stream::parser::{SseLine, SseParser};

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    UnknownModel(#[from] UnknownModel),
    #[error("{0}")]
    Provider(#[from] ProxyError),
    #[error("{0}")]
    Internal(String),
}

impl PipelineError {
    pub fn status(&self) -> StatusCode {
        match self {
            PipelineError::BadRequest(_) | PipelineError::UnknownModel(_) => {
                StatusCode::BAD_REQUEST
            }
            PipelineError::Provider(ProxyError::ProviderError { status, .. }) => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            PipelineError::Provider(_) | PipelineError::Internal(_) => StatusCode::BAD_GATEWAY,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            PipelineError::BadRequest(_) => "invalid_request",
            PipelineError::UnknownModel(_) => "unknown_model",
            PipelineError::Provider(_) => "provider_error",
            PipelineError::Internal(_) => "proxy_error",
        }
    }
}

pub async fn execute_bridge(
    client: &HttpClient,
    log_ctx: &RequestLogContext,
    bridge_name: &str,
    bridge: &BridgeConfig,
    provider: &ProviderConfig,
    body: &str,
) -> Result<Response, PipelineError> {
    match (&bridge.agent.api_format, &provider.api_format) {
        (ApiFormat::AnthropicMessages, ApiFormat::AnthropicMessages) => {
            anthropic_passthrough(client, log_ctx, bridge_name, bridge, provider, body).await
        }
        _ => Err(PipelineError::Internal(format!(
            "unsupported format pair {:?} -> {:?}",
            bridge.agent.api_format, provider.api_format
        ))),
    }
}

pub fn error_response(error: PipelineError) -> Response {
    let body = serde_json::json!({
        "error": {
            "type": error.error_type(),
            "message": error.to_string(),
        }
    });
    (
        error.status(),
        serde_json::to_string(&body).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string()),
    )
        .into_response()
}

fn provider_key(provider: &ProviderConfig) -> Option<String> {
    provider_api_key(provider.api_key_env.as_deref())
}

fn json_model(value: &Value) -> Result<&str, PipelineError> {
    value.get("model").and_then(Value::as_str).ok_or_else(|| {
        PipelineError::BadRequest("request body must contain string field 'model'".to_string())
    })
}

fn map_json_model(
    log_ctx: &RequestLogContext,
    bridge_name: &str,
    bridge: &BridgeConfig,
    value: &mut Value,
) -> Result<(String, String), PipelineError> {
    let agent_model = json_model(value)?.to_string();
    let provider_model =
        ModelPolicy::new(bridge_name, &bridge.models).resolve_provider_model(&agent_model)?;
    value["model"] = Value::String(provider_model.to_string());
    tracing::info!(
        request_id = log_ctx.request_id,
        bridge = %log_ctx.bridge,
        agent_base_url = %log_ctx.agent_base_url,
        provider = %log_ctx.provider,
        agent_model = %agent_model,
        provider_model = %provider_model,
        "model map"
    );
    Ok((agent_model, provider_model.to_string()))
}

fn response_json(value: Value) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&value).unwrap_or_else(|_| r#"{"error":"serialize"}"#.to_string()),
    )
        .into_response()
}

async fn anthropic_passthrough(
    client: &HttpClient,
    log_ctx: &RequestLogContext,
    bridge_name: &str,
    bridge: &BridgeConfig,
    provider: &ProviderConfig,
    body: &str,
) -> Result<Response, PipelineError> {
    let started_at = Instant::now();
    tracing::debug!(
        request_id = log_ctx.request_id,
        bridge = %log_ctx.bridge,
        agent_base_url = %log_ctx.agent_base_url,
        provider = %log_ctx.provider,
        summary = %request_body_summary(body),
        inbound_body_bytes = body.len(),
        "inbound request body summary"
    );
    let mut request: Value = serde_json::from_str(body)
        .map_err(|e| PipelineError::BadRequest(format!("invalid JSON: {}", e)))?;
    let (agent_model, provider_model) = map_json_model(log_ctx, bridge_name, bridge, &mut request)?;
    let stream = request.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let url = format!("{}/v1/messages", provider.base_url.trim_end_matches('/'));
    let key = provider_key(provider);
    let auth = provider_auth(&provider.api_format, key.as_deref());
    tracing::info!(
        request_id = log_ctx.request_id,
        bridge = %log_ctx.bridge,
        agent_base_url = %log_ctx.agent_base_url,
        provider = %log_ctx.provider,
        agent_model = %agent_model,
        provider_model = %provider_model,
        stream,
        url = %url,
        has_api_key = key.is_some(),
        "upstream request"
    );
    tracing::debug!(
        request_id = log_ctx.request_id,
        bridge = %log_ctx.bridge,
        provider = %log_ctx.provider,
        outbound_body_bytes = serde_json::to_vec(&request).map(|body| body.len()).unwrap_or(0),
        auth_headers = %auth.header_names(),
        "upstream request body summary"
    );

    if stream {
        let rx =
            client.post_streaming_with_headers_logged(&url, &auth, &request, Some(log_ctx)).await?;
        let log_ctx = log_ctx.clone();
        let stream = async_stream::stream! {
            let mut parser = SseParser::new();
            let mut current_event: Option<String> = None;
            let mut data_count = 0usize;
            let mut event_count = 0usize;
            let mut done = false;
            let mut rx = rx;
            while let Some(line) = rx.recv().await {
                match parser.feed(&line) {
                    Some(SseLine::Event(event)) => {
                        event_count += 1;
                        current_event = Some(event.to_string());
                    }
                    Some(SseLine::Data(data)) => {
                        data_count += 1;
                        let (restored, restored_count) =
                            restore_model_in_sse_data(data, &provider_model, &agent_model);
                        if data_count == 1 && restored_count == 0 {
                            tracing::warn!(
                                request_id = log_ctx.request_id,
                                bridge = %log_ctx.bridge,
                                provider = %log_ctx.provider,
                                agent_model = %agent_model,
                                provider_model = %provider_model,
                                "first SSE data did not restore model"
                            );
                        } else if data_count == 1 {
                            tracing::debug!(
                                request_id = log_ctx.request_id,
                                bridge = %log_ctx.bridge,
                                provider = %log_ctx.provider,
                                restored_fields = restored_count,
                                "first SSE data restored model"
                            );
                        }
                        let mut event = Event::default().data(restored);
                        if let Some(name) = current_event.take() {
                            event = event.event(name);
                        } else {
                            tracing::debug!(
                                request_id = log_ctx.request_id,
                                bridge = %log_ctx.bridge,
                                provider = %log_ctx.provider,
                                data_count,
                                "upstream SSE data without preceding event line"
                            );
                        }
                        yield Ok::<_, std::convert::Infallible>(event);
                    }
                    Some(SseLine::Done) => {
                        done = true;
                        tracing::info!(
                            request_id = log_ctx.request_id,
                            bridge = %log_ctx.bridge,
                            provider = %log_ctx.provider,
                            agent_model = %agent_model,
                            provider_model = %provider_model,
                            stream,
                            events = event_count,
                            data = data_count,
                            elapsed_ms = started_at.elapsed().as_millis(),
                            "upstream SSE done"
                        );
                        break;
                    }
                    _ => {}
                }
            }
            tracing::debug!(
                request_id = log_ctx.request_id,
                bridge = %log_ctx.bridge,
                provider = %log_ctx.provider,
                agent_model = %agent_model,
                provider_model = %provider_model,
                stream,
                events = event_count,
                data = data_count,
                done,
                elapsed_ms = started_at.elapsed().as_millis(),
                "upstream SSE closed"
            );
        };
        Ok(Sse::new(stream).into_response())
    } else {
        let mut response: Value =
            client.post_json_with_headers_logged(&url, &auth, &request, Some(log_ctx)).await?;
        let restored = restore_model_fields(&mut response, &provider_model, &agent_model);
        if restored == 0 {
            tracing::warn!(
                request_id = log_ctx.request_id,
                bridge = %log_ctx.bridge,
                provider = %log_ctx.provider,
                agent_model = %agent_model,
                provider_model = %provider_model,
                "response model restore did not match"
            );
        }
        tracing::info!(
            request_id = log_ctx.request_id,
            bridge = %log_ctx.bridge,
            provider = %log_ctx.provider,
            agent_model = %agent_model,
            provider_model = %provider_model,
            stream,
            restored_fields = restored,
            elapsed_ms = started_at.elapsed().as_millis(),
            "non-stream response restored"
        );
        Ok(response_json(response))
    }
}
