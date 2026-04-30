//! Provider format adapters (Chat Completions, Anthropic Messages).
//! Each adapter transforms a Responses API request into a provider-specific
//! format and sends it, then converts the response back.

use crate::types::responses::ResponsesResponse;
use async_trait::async_trait;

/// The result of sending a request: either a full response
/// or a stream of SSE events.
pub enum ProviderResponse {
    Complete(Box<ResponsesResponse>),
    Stream(tokio::sync::mpsc::Receiver<String>),
}

#[async_trait]
pub trait FormatAdapter: Send + Sync {
    /// Transform Responses → provider format, send to provider,
    /// transform response back → Responses format.
    async fn send(
        &self,
        client: &crate::client::HttpClient,
        base_url: &str,
        api_key: Option<&str>,
        responses_req: &crate::types::responses::ResponsesRequest,
    ) -> Result<ProviderResponse, crate::client::ProxyError>;
}

pub mod anthropic;
pub mod chat_completions;
