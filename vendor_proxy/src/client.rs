//! HTTP client with JSON and SSE streaming support.

use reqwest::Client;
use std::time::Duration;
use thiserror::Error;

use crate::logging::{provider_error_preview, RequestLogContext};

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Stream error: {0}")]
    Stream(String),
    #[error("Provider error: {status} — {message}")]
    ProviderError { status: u16, message: String },
    #[error("Upstream error: {0}")]
    UpstreamError(String),
}

pub struct HttpClient {
    inner: Client,
}

/// Header configuration for provider-specific auth
pub struct AuthHeaders {
    /// If set, sent as `Authorization: Bearer {key}`
    pub bearer_key: Option<String>,
    /// Additional headers (e.g., x-api-key, anthropic-version)
    pub extra: Vec<(String, String)>,
}

impl AuthHeaders {
    pub fn bearer(key: Option<&str>) -> Self {
        Self { bearer_key: key.map(|k| k.to_string()), extra: vec![] }
    }

    pub fn anthropic(api_key: Option<&str>) -> Self {
        let mut extra = vec![("anthropic-version".to_string(), "2023-06-01".to_string())];
        if let Some(key) = api_key {
            extra.push(("x-api-key".to_string(), key.to_string()));
        }
        Self { bearer_key: None, extra }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    pub fn new() -> Self {
        let inner = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!("Failed to create HTTP client: {}", e);
                reqwest::Client::new()
            });
        Self { inner }
    }

    /// POST JSON, get JSON back (non-streaming)
    pub async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &T,
    ) -> Result<R, ProxyError> {
        self.post_json_with_headers(url, &AuthHeaders::bearer(api_key), body).await
    }

    /// POST JSON with custom headers, get JSON back (non-streaming)
    pub async fn post_json_with_headers<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        auth: &AuthHeaders,
        body: &T,
    ) -> Result<R, ProxyError> {
        self.post_json_with_headers_logged(url, auth, body, None).await
    }

    pub async fn post_json_with_headers_logged<
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    >(
        &self,
        url: &str,
        auth: &AuthHeaders,
        body: &T,
        ctx: Option<&RequestLogContext>,
    ) -> Result<R, ProxyError> {
        let mut req = self.inner.post(url).timeout(Duration::from_secs(120)).json(body);

        if let Some(ref key) = auth.bearer_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        for (name, value) in &auth.extra {
            req = req.header(name.as_str(), value.as_str());
        }

        if let Some(ctx) = ctx {
            tracing::debug!(
                request_id = ctx.request_id,
                bridge = %ctx.bridge,
                agent_base_url = %ctx.agent_base_url,
                provider = %ctx.provider,
                url = %url,
                auth_headers = %auth.header_names(),
                "upstream JSON request"
            );
        } else {
            tracing::debug!(
                "upstream JSON request: url={} auth_headers={}",
                url,
                auth.header_names()
            );
        }
        let resp = req.send().await?;
        let status = resp.status();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<missing>")
            .to_string();
        if let Some(ctx) = ctx {
            tracing::info!(
                request_id = ctx.request_id,
                bridge = %ctx.bridge,
                provider = %ctx.provider,
                url = %url,
                status = %status,
                content_type = %content_type,
                "upstream JSON response"
            );
        } else {
            tracing::info!(
                "upstream JSON response: url={} status={} content_type={}",
                url,
                status,
                content_type
            );
        }

        if status.is_success() {
            match resp.json().await {
                Ok(value) => Ok(value),
                Err(error) => {
                    if let Some(ctx) = ctx {
                        tracing::error!(
                            request_id = ctx.request_id,
                            bridge = %ctx.bridge,
                            provider = %ctx.provider,
                            url = %url,
                            status = %status,
                            content_type = %content_type,
                            error = %error,
                            "upstream_decode_error"
                        );
                    } else {
                        tracing::error!(
                            "upstream JSON parse error: url={} status={} content_type={} error={}",
                            url,
                            status,
                            content_type,
                            error
                        );
                    }
                    Err(error.into())
                }
            }
        } else {
            let body = resp.text().await.unwrap_or_default();
            let preview = provider_error_preview(&body);
            if let Some(ctx) = ctx {
                tracing::warn!(
                    request_id = ctx.request_id,
                    bridge = %ctx.bridge,
                    provider = %ctx.provider,
                    url = %url,
                    status = %status,
                    body_bytes = body.len(),
                    body_preview = %preview,
                    "provider_error"
                );
            } else {
                tracing::warn!(
                    "upstream JSON provider error: url={} status={} body_bytes={} body_preview={}",
                    url,
                    status,
                    body.len(),
                    preview
                );
            }
            Err(ProxyError::ProviderError { status: status.as_u16(), message: body })
        }
    }

    /// POST JSON, get SSE stream back
    pub async fn post_streaming<T: serde::Serialize>(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &T,
    ) -> Result<tokio::sync::mpsc::Receiver<String>, ProxyError> {
        self.post_streaming_with_headers(url, &AuthHeaders::bearer(api_key), body).await
    }

    /// POST JSON with custom headers, get SSE stream back
    pub async fn post_streaming_with_headers<T: serde::Serialize>(
        &self,
        url: &str,
        auth: &AuthHeaders,
        body: &T,
    ) -> Result<tokio::sync::mpsc::Receiver<String>, ProxyError> {
        self.post_streaming_with_headers_logged(url, auth, body, None).await
    }

    pub async fn post_streaming_with_headers_logged<T: serde::Serialize>(
        &self,
        url: &str,
        auth: &AuthHeaders,
        body: &T,
        ctx: Option<&RequestLogContext>,
    ) -> Result<tokio::sync::mpsc::Receiver<String>, ProxyError> {
        use tokio::sync::mpsc;
        let started_at = std::time::Instant::now();

        let mut req = self.inner.post(url).json(body).header("Accept", "text/event-stream");

        if let Some(ref key) = auth.bearer_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        for (name, value) in &auth.extra {
            req = req.header(name.as_str(), value.as_str());
        }

        if let Some(ctx) = ctx {
            tracing::debug!(
                request_id = ctx.request_id,
                bridge = %ctx.bridge,
                agent_base_url = %ctx.agent_base_url,
                provider = %ctx.provider,
                url = %url,
                auth_headers = %auth.header_names(),
                "upstream SSE request"
            );
        } else {
            tracing::debug!(
                "upstream SSE request: url={} auth_headers={}",
                url,
                auth.header_names()
            );
        }
        let resp = req.send().await?;
        let status = resp.status();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<missing>")
            .to_string();
        if let Some(ctx) = ctx {
            tracing::info!(
                request_id = ctx.request_id,
                bridge = %ctx.bridge,
                provider = %ctx.provider,
                url = %url,
                status = %status,
                content_type = %content_type,
                "upstream SSE response"
            );
        } else {
            tracing::info!(
                "upstream SSE response: url={} status={} content_type={}",
                url,
                status,
                content_type
            );
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let preview = provider_error_preview(&body);
            if let Some(ctx) = ctx {
                tracing::warn!(
                    request_id = ctx.request_id,
                    bridge = %ctx.bridge,
                    provider = %ctx.provider,
                    url = %url,
                    status = %status,
                    body_bytes = body.len(),
                    body_preview = %preview,
                    "provider_error"
                );
            } else {
                tracing::warn!(
                    "upstream SSE provider error: url={} status={} body_bytes={} body_preview={}",
                    url,
                    status,
                    body.len(),
                    preview
                );
            }
            return Err(ProxyError::ProviderError { status: status.as_u16(), message: body });
        }

        let (tx, rx) = mpsc::channel(64);
        let mut stream = resp.bytes_stream();
        let ctx = ctx.cloned();
        let url = url.to_string();

        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut buffer = String::new();
            let mut chunk_count = 0usize;
            let mut line_count = 0usize;
            let mut byte_count = 0usize;
            let mut saw_done = false;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        chunk_count += 1;
                        byte_count += bytes.len();
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();
                            if line == "data: [DONE]" || line == "[DONE]" {
                                saw_done = true;
                            }
                            let has_line = !line.is_empty();
                            if has_line {
                                line_count += 1;
                            }
                            if has_line && tx.send(line).await.is_err() {
                                if let Some(ctx) = &ctx {
                                    tracing::warn!(
                                        request_id = ctx.request_id,
                                        bridge = %ctx.bridge,
                                        provider = %ctx.provider,
                                        url = %url,
                                        elapsed_ms = started_at.elapsed().as_millis(),
                                        chunks = chunk_count,
                                        lines = line_count,
                                        bytes = byte_count,
                                        saw_done,
                                        "client_disconnected"
                                    );
                                }
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(ctx) = &ctx {
                            tracing::error!(
                                request_id = ctx.request_id,
                                bridge = %ctx.bridge,
                                provider = %ctx.provider,
                                url = %url,
                                elapsed_ms = started_at.elapsed().as_millis(),
                                chunks = chunk_count,
                                lines = line_count,
                                bytes = byte_count,
                                saw_done,
                                error = %e,
                                "upstream_decode_error"
                            );
                        } else {
                            tracing::error!("upstream SSE stream read error: {}", e);
                        }
                        break;
                    }
                }
            }
            let remaining = buffer.trim().to_string();
            if !remaining.is_empty() {
                let _ = tx.send(remaining).await;
            }
            if let Some(ctx) = &ctx {
                tracing::debug!(
                    request_id = ctx.request_id,
                    bridge = %ctx.bridge,
                    provider = %ctx.provider,
                    url = %url,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    chunks = chunk_count,
                    lines = line_count,
                    bytes = byte_count,
                    saw_done,
                    "upstream SSE body closed"
                );
            }
        });

        Ok(rx)
    }
}

impl AuthHeaders {
    pub(crate) fn header_names(&self) -> String {
        let mut names = Vec::new();
        if self.bearer_key.is_some() {
            names.push("authorization".to_string());
        }
        names.extend(self.extra.iter().map(|(name, _)| name.clone()));
        names.join(",")
    }
}
