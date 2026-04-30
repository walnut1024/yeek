use super::{FormatAdapter, ProviderResponse};
use crate::bridge::chat_to_responses::chat_to_responses;
use crate::bridge::responses_to_chat::responses_to_chat;
use crate::client::{AuthHeaders, HttpClient, ProxyError};
use crate::types::anthropic::*;
use crate::types::chat::*;
use crate::types::responses::ResponsesRequest;
use async_trait::async_trait;

pub struct AnthropicAdapter;

impl AnthropicAdapter {
    /// Step 2: Chat Completions → Anthropic Messages format.
    /// Mirrors LiteLLM's anthropic_messages_pt() pattern.
    pub fn chat_to_anthropic(chat_req: &ChatCompletionRequest) -> AnthropicRequest {
        let mut system: Option<String> = None;
        let mut messages: Vec<AnthropicMessage> = Vec::new();

        for msg in &chat_req.messages {
            match msg {
                ChatMessage::System { content } => {
                    system = Some(content.clone());
                }
                ChatMessage::User { content } => {
                    let blocks = Self::user_content_to_blocks(content);
                    // Merge with previous user message if it exists (Anthropic requirement)
                    if let Some(last) = messages.last_mut() {
                        if last.role == "user" {
                            Self::merge_user_content(last, blocks);
                            continue;
                        }
                    }
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(blocks),
                    });
                }
                ChatMessage::Assistant { content, tool_calls, .. } => {
                    let mut blocks: Vec<AnthropicContentBlock> = Vec::new();

                    if let Some(ref text) = content {
                        if !text.is_empty() {
                            blocks.push(AnthropicContentBlock::Text { text: text.clone() });
                        }
                    }

                    if let Some(ref tcs) = tool_calls {
                        for tc in tcs {
                            let input: serde_json::Value = serde_json::from_str(
                                &tc.function.arguments,
                            )
                            .unwrap_or(serde_json::Value::String(tc.function.arguments.clone()));
                            blocks.push(AnthropicContentBlock::ToolUse {
                                id: tc.id.clone(),
                                name: tc.function.name.clone(),
                                input,
                            });
                        }
                    }

                    if !blocks.is_empty() {
                        messages.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: AnthropicContent::Blocks(blocks),
                        });
                    }
                }
                ChatMessage::Tool { content, tool_call_id } => {
                    let text = match content {
                        ChatMessageContent::String(s) => s.clone(),
                        ChatMessageContent::Parts(parts) => parts
                            .iter()
                            .filter_map(|p| match p {
                                ContentPart::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(""),
                    };
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(vec![
                            AnthropicContentBlock::ToolResult {
                                tool_use_id: tool_call_id.clone(),
                                content: text,
                            },
                        ]),
                    });
                }
            }
        }

        let tools: Vec<AnthropicTool> = chat_req
            .tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.function.name.clone(),
                description: t.function.description.clone(),
                input_schema: t.function.parameters.clone(),
            })
            .collect();

        let max_tokens = if tools.is_empty() {
            chat_req.max_tokens.unwrap_or(4096)
        } else {
            chat_req.max_tokens.unwrap_or(16384)
        };

        AnthropicRequest {
            model: chat_req.model.clone(),
            max_tokens,
            system,
            messages,
            tools,
            tool_choice: chat_to_anthropic_tool_choice(&chat_req.tool_choice),
            stream: chat_req.stream,
            temperature: chat_req.temperature,
            top_p: chat_req.top_p,
        }
    }

    /// Step 3: Anthropic response → Chat Completions response.
    /// Mirrors LiteLLM's AnthropicConfig.transform_response() pattern.
    pub fn anthropic_to_chat(resp: &AnthropicResponse) -> ChatCompletionResponse {
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();
        let mut finish_reason = map_anthropic_stop_reason(resp.stop_reason.as_deref());

        for block in &resp.content {
            match block {
                AnthropicContentBlock::Text { text } => {
                    if !text.is_empty() {
                        text_content.push_str(text);
                    }
                }
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ChatToolCall {
                        id: id.clone(),
                        call_type: "function".to_string(),
                        function: ChatFunctionCall {
                            name: name.clone(),
                            arguments: serde_json::to_string(input).unwrap_or_default(),
                        },
                    });
                }
                _ => {}
            }
        }

        if !tool_calls.is_empty() {
            finish_reason = Some("tool_calls".to_string());
        }

        let usage = ChatUsage {
            prompt_tokens: resp.usage.input_tokens,
            completion_tokens: resp.usage.output_tokens,
            total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
        };

        ChatCompletionResponse {
            id: resp.id.clone(),
            object: "chat.completion".to_string(),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: resp.model.clone(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: "assistant".to_string(),
                    content: if text_content.is_empty() { None } else { Some(text_content) },
                    tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                    reasoning_content: None,
                    annotations: None,
                },
                finish_reason,
            }],
            usage: Some(usage),
        }
    }

    fn user_content_to_blocks(content: &ChatMessageContent) -> Vec<AnthropicContentBlock> {
        match content {
            ChatMessageContent::String(text) => {
                vec![AnthropicContentBlock::Text { text: text.clone() }]
            }
            ChatMessageContent::Parts(parts) => parts
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => {
                        AnthropicContentBlock::Text { text: text.clone() }
                    }
                    ContentPart::Image { image_url } => Self::image_to_anthropic(&image_url.url),
                    ContentPart::File { file } => {
                        AnthropicContentBlock::Text { text: format!("[File: {}]", file) }
                    }
                })
                .collect(),
        }
    }

    fn image_to_anthropic(url: &str) -> AnthropicContentBlock {
        if url.starts_with("data:") {
            if let Some(comma_pos) = url.find(',') {
                let media_part = &url[5..comma_pos];
                let media_type = media_part.trim_end_matches(";base64");
                let data = url[comma_pos + 1..].to_string();
                return AnthropicContentBlock::Image {
                    source: AnthropicImageSource {
                        source_type: "base64".to_string(),
                        media_type: media_type.to_string(),
                        data,
                    },
                };
            }
        }
        AnthropicContentBlock::Text { text: format!("[Image: {}]", url) }
    }

    fn merge_user_content(last: &mut AnthropicMessage, new_blocks: Vec<AnthropicContentBlock>) {
        match &mut last.content {
            AnthropicContent::Blocks(existing) => {
                existing.extend(new_blocks);
            }
            AnthropicContent::String(text) => {
                let mut blocks = vec![AnthropicContentBlock::Text { text: text.clone() }];
                blocks.extend(new_blocks);
                last.content = AnthropicContent::Blocks(blocks);
            }
        }
    }
}

/// Map Anthropic stop_reason → Chat Completions finish_reason.
fn map_anthropic_stop_reason(reason: Option<&str>) -> Option<String> {
    match reason {
        Some("end_turn") | Some("stop_sequence") => Some("stop".to_string()),
        Some("max_tokens") => Some("length".to_string()),
        Some("tool_use") => Some("tool_calls".to_string()),
        _ => Some("stop".to_string()),
    }
}

/// Convert Chat Completions tool_choice to Anthropic tool_choice format.
/// Chat: "auto" | "none" | "required" | {"type":"function","function":{"name":"X"}}
/// Anthropic: {"type":"auto"} | {"type":"none"} | {"type":"any"} | {"type":"tool","name":"X"}
fn chat_to_anthropic_tool_choice(
    tool_choice: &Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match tool_choice {
        None => None,
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "auto" => Some(serde_json::json!({"type": "auto"})),
            "none" => Some(serde_json::json!({"type": "none"})),
            "required" => Some(serde_json::json!({"type": "any"})),
            _ => None,
        },
        Some(serde_json::Value::Object(obj)) => {
            let tc_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if tc_type == "function" {
                if let Some(name) =
                    obj.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str())
                {
                    return Some(serde_json::json!({"type": "tool", "name": name}));
                }
            }
            None
        }
        _ => None,
    }
}

#[async_trait]
impl FormatAdapter for AnthropicAdapter {
    async fn send(
        &self,
        client: &HttpClient,
        base_url: &str,
        api_key: Option<&str>,
        responses_req: &ResponsesRequest,
    ) -> Result<ProviderResponse, ProxyError> {
        // Step 1: Responses → Chat
        let chat_req = responses_to_chat(responses_req);

        // Step 2: Chat → Anthropic
        let anthropic_req = Self::chat_to_anthropic(&chat_req);

        // Debug: log the Anthropic request (truncated)
        let req_json = serde_json::to_string(&anthropic_req).unwrap_or_default();
        let preview = if req_json.len() > 2000 { &req_json[..2000] } else { &req_json };
        tracing::info!("Anthropic request ({} bytes): {}", req_json.len(), preview);

        let auth = AuthHeaders::anthropic(api_key);

        if responses_req.stream {
            // Streaming path
            let url = format!("{}/messages", base_url.trim_end_matches('/'));
            let rx = client.post_streaming_with_headers(&url, &auth, &anthropic_req).await?;
            Ok(ProviderResponse::Stream(rx))
        } else {
            // Non-streaming path
            let url = format!("{}/messages", base_url.trim_end_matches('/'));
            let anthropic_resp: AnthropicResponse =
                client.post_json_with_headers(&url, &auth, &anthropic_req).await?;

            // Step 3: Anthropic → Chat
            let chat_resp = Self::anthropic_to_chat(&anthropic_resp);

            // Step 4: Chat → Responses (with echo fields)
            let responses_resp = chat_to_responses(&chat_resp, Some(responses_req));
            Ok(ProviderResponse::Complete(Box::new(responses_resp)))
        }
    }
}
