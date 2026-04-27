use super::{FormatAdapter, ProviderResponse};
use crate::bridge::chat_to_responses::chat_to_responses;
use crate::bridge::responses_to_chat::responses_to_chat;
use crate::client::{HttpClient, ProxyError};
use async_trait::async_trait;

pub struct ChatCompletionsAdapter;

#[async_trait]
impl FormatAdapter for ChatCompletionsAdapter {
    async fn send(
        &self,
        client: &HttpClient,
        base_url: &str,
        api_key: Option<&str>,
        responses_req: &crate::types::responses::ResponsesRequest,
    ) -> Result<ProviderResponse, ProxyError> {
        // Step 1: Responses → Chat
        let chat_req = responses_to_chat(responses_req);

        if responses_req.stream {
            // Streaming path
            let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
            let rx = client.post_streaming(&url, api_key, &chat_req).await?;
            Ok(ProviderResponse::Stream(rx))
        } else {
            // Non-streaming path
            let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
            let chat_resp: crate::types::chat::ChatCompletionResponse =
                client.post_json(&url, api_key, &chat_req).await?;

            // Step 4: Chat → Responses (with echo fields)
            let responses_resp = chat_to_responses(&chat_resp, Some(responses_req));
            Ok(ProviderResponse::Complete(Box::new(responses_resp)))
        }
    }
}
