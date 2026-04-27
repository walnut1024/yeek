use reqwest::Client;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Stream error: {0}")]
    Stream(String),
    #[error("Provider error: {status} — {message}")]
    ProviderError { status: u16, message: String },
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
        Self {
            bearer_key: key.map(|k| k.to_string()),
            extra: vec![],
        }
    }

    pub fn anthropic(api_key: Option<&str>) -> Self {
        let mut extra = vec![("anthropic-version".to_string(), "2023-06-01".to_string())];
        if let Some(key) = api_key {
            extra.push(("x-api-key".to_string(), key.to_string()));
        }
        Self {
            bearer_key: None,
            extra,
        }
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
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");
        Self { inner }
    }

    /// POST JSON, get JSON back (non-streaming)
    pub async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &T,
    ) -> Result<R, ProxyError> {
        self.post_json_with_headers(url, &AuthHeaders::bearer(api_key), body)
            .await
    }

    /// POST JSON with custom headers, get JSON back (non-streaming)
    pub async fn post_json_with_headers<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        auth: &AuthHeaders,
        body: &T,
    ) -> Result<R, ProxyError> {
        let mut req = self.inner.post(url).json(body);

        if let Some(ref key) = auth.bearer_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        for (name, value) in &auth.extra {
            req = req.header(name.as_str(), value.as_str());
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status.is_success() {
            Ok(resp.json().await?)
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(ProxyError::ProviderError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// POST JSON, get SSE stream back
    pub async fn post_streaming<T: serde::Serialize>(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &T,
    ) -> Result<tokio::sync::mpsc::Receiver<String>, ProxyError> {
        self.post_streaming_with_headers(url, &AuthHeaders::bearer(api_key), body)
            .await
    }

    /// POST JSON with custom headers, get SSE stream back
    pub async fn post_streaming_with_headers<T: serde::Serialize>(
        &self,
        url: &str,
        auth: &AuthHeaders,
        body: &T,
    ) -> Result<tokio::sync::mpsc::Receiver<String>, ProxyError> {
        use tokio::sync::mpsc;

        let mut req = self
            .inner
            .post(url)
            .json(body)
            .header("Accept", "text/event-stream");

        if let Some(ref key) = auth.bearer_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        for (name, value) in &auth.extra {
            req = req.header(name.as_str(), value.as_str());
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProxyError::ProviderError {
                status: status.as_u16(),
                message: body,
            });
        }

        let (tx, rx) = mpsc::channel(64);
        let mut stream = resp.bytes_stream();

        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();
                            if !line.is_empty() && tx.send(line).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Stream error: {}", e);
                        break;
                    }
                }
            }
            let remaining = buffer.trim().to_string();
            if !remaining.is_empty() {
                let _ = tx.send(remaining).await;
            }
        });

        Ok(rx)
    }
}
