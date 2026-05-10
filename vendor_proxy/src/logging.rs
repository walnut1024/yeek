use axum::http::HeaderMap;
use serde_json::Value;

const PROVIDER_ERROR_PREVIEW_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct RequestLogContext {
    pub request_id: u64,
    pub bridge: String,
    pub agent_base_url: String,
    pub provider: String,
}

impl RequestLogContext {
    pub fn new(request_id: u64, bridge: &str, agent_base_url: &str, provider: &str) -> Self {
        Self {
            request_id,
            bridge: bridge.to_string(),
            agent_base_url: agent_base_url.to_string(),
            provider: provider.to_string(),
        }
    }
}

pub fn header_names(headers: &HeaderMap) -> String {
    let mut names: Vec<_> = headers.keys().map(|name| name.as_str().to_string()).collect();
    names.sort();
    names.dedup();
    names.join(",")
}

pub fn request_body_summary(body: &str) -> String {
    let mut parts = vec![format!("bytes={}", body.len())];
    match serde_json::from_str::<Value>(body) {
        Ok(value) => {
            if let Some(model) = value.get("model").and_then(Value::as_str) {
                parts.push(format!("model={}", model));
            }
            if let Some(stream) = value.get("stream").and_then(Value::as_bool) {
                parts.push(format!("stream={}", stream));
            }
            if let Some(max_tokens) = value.get("max_tokens").and_then(Value::as_u64) {
                parts.push(format!("max_tokens={}", max_tokens));
            }
            if let Some(messages) = value.get("messages").and_then(Value::as_array) {
                parts.push(format!("messages={}", messages.len()));
            }
            if let Some(tools) = value.get("tools").and_then(Value::as_array) {
                parts.push(format!("tools={}", tools.len()));
            }
        }
        Err(error) => {
            parts.push(format!("json_error={}", error));
        }
    }
    parts.join(" ")
}

pub fn provider_error_preview(body: &str) -> String {
    if body.len() <= PROVIDER_ERROR_PREVIEW_LIMIT {
        return body.to_string();
    }

    let mut end = PROVIDER_ERROR_PREVIEW_LIMIT;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...<truncated {} bytes>", &body[..end], body.len() - end)
}
