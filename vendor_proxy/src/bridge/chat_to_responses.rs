use crate::types::chat::*;
use crate::types::responses::*;

/// Transform a Chat Completions response into a Responses API response.
/// Mirrors LiteLLM's transform_chat_completion_response_to_responses_api_response.
/// If `original_req` is provided, echo fields are included in the response.
pub fn chat_to_responses(
    chat: &ChatCompletionResponse,
    original_req: Option<&ResponsesRequest>,
) -> ResponsesResponse {
    let mut output: Vec<ResponsesOutputItem> = Vec::new();
    let mut overall_status: Option<String> = None;

    for choice in &chat.choices {
        let choice_status = map_finish_reason_to_status(choice.finish_reason.as_deref());
        overall_status = Some(choice_status.clone());

        // reasoning_content -> Reasoning output item
        if let Some(ref reasoning) = choice.message.reasoning_content {
            if !reasoning.is_empty() {
                output.push(ResponsesOutputItem::Reasoning {
                    id: Some(format!("rs_{}", &chat.id)),
                    status: Some(choice_status.clone()),
                    role: "assistant".to_string(),
                    summary: vec![ReasoningSummaryBlock::SummaryText { text: reasoning.clone() }],
                });
            }
        }

        // Text content -> output message
        if let Some(ref content) = choice.message.content {
            if !content.is_empty() {
                let annotations = choice.message.annotations.clone().unwrap_or_default();
                let content_blocks =
                    vec![OutputContentBlock::OutputText { text: content.clone(), annotations }];

                output.push(ResponsesOutputItem::Message {
                    id: Some(format!("msg_{}", &chat.id)),
                    status: Some(choice_status.clone()),
                    role: choice.message.role.clone(),
                    content: content_blocks,
                });
            }
        }

        // Tool calls -> function_call output items
        if let Some(ref tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                output.push(ResponsesOutputItem::FunctionCall {
                    id: Some(format!("fc_{}", tc.id)),
                    status: Some(choice_status.clone()),
                    call_id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments: tc.function.arguments.clone(),
                });
            }
        }
    }

    let usage = chat.usage.as_ref().map(|u| ResponsesUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
        input_tokens_details: None,
        output_tokens_details: None,
    });

    // Extract echo fields from original request
    let (
        metadata,
        temperature,
        top_p,
        tool_choice,
        truncation,
        reasoning,
        text,
        store,
        max_output_tokens,
        previous_response_id,
        parallel_tool_calls,
    ) = match original_req {
        Some(req) => (
            req.metadata.clone(),
            req.temperature,
            req.top_p,
            req.tool_choice.clone(),
            req.truncation.clone(),
            req.reasoning.clone(),
            req.text.clone(),
            req.store,
            req.max_output_tokens,
            req.previous_response_id.clone(),
            req.parallel_tool_calls,
        ),
        None => (None, None, None, None, None, None, None, None, None, None, None),
    };

    ResponsesResponse {
        id: format!("resp_{}", chat.id),
        object: "response".to_string(),
        created_at: chat.created,
        model: chat.model.clone(),
        status: overall_status.unwrap_or_else(|| "completed".to_string()),
        output,
        usage,
        error: None,
        metadata,
        incomplete_details: None,
        parallel_tool_calls,
        temperature,
        top_p,
        tool_choice,
        truncation,
        reasoning,
        text,
        store,
        max_output_tokens,
        previous_response_id,
    }
}

/// Map Chat Completions finish_reason -> Responses API status.
/// Mirrors LiteLLM's _map_chat_completion_finish_reason_to_responses_status.
pub fn map_finish_reason_to_status(reason: Option<&str>) -> String {
    match reason {
        Some("stop") | Some("tool_calls") | Some("function_call") => "completed".to_string(),
        Some("length") | Some("content_filter") | Some("refusal") => "incomplete".to_string(),
        _ => "completed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chat_response(content: &str, finish_reason: &str) -> ChatCompletionResponse {
        ChatCompletionResponse {
            id: "chat-1".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: "assistant".to_string(),
                    content: Some(content.to_string()),
                    tool_calls: None,
                    reasoning_content: None,
                    annotations: None,
                },
                finish_reason: Some(finish_reason.to_string()),
            }],
            usage: Some(ChatUsage { prompt_tokens: 10, completion_tokens: 3, total_tokens: 13 }),
        }
    }

    #[test]
    fn test_chat_text_to_responses() {
        let chat = make_chat_response("Hello!", "stop");
        let resp = chat_to_responses(&chat, None);
        assert_eq!(resp.status, "completed");
        assert_eq!(resp.output.len(), 1);
        match &resp.output[0] {
            ResponsesOutputItem::Message { id, status, content, .. } => {
                assert!(id.as_ref().unwrap().starts_with("msg_"));
                assert_eq!(status.as_deref(), Some("completed"));
                if let OutputContentBlock::OutputText { text, .. } = &content[0] {
                    assert_eq!(text, "Hello!");
                }
            }
            _ => panic!("Expected message output"),
        }
        assert!(resp.id.starts_with("resp_"));
    }

    #[test]
    fn test_chat_finish_reason_incomplete() {
        let chat = make_chat_response("truncated...", "length");
        let resp = chat_to_responses(&chat, None);
        assert_eq!(resp.status, "incomplete");
    }

    #[test]
    fn test_refusal_is_incomplete() {
        assert_eq!(map_finish_reason_to_status(Some("refusal")), "incomplete");
    }

    #[test]
    fn test_reasoning_content_becomes_output_item() {
        let chat = ChatCompletionResponse {
            id: "chat-rs".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: "assistant".to_string(),
                    content: Some("The answer is 42.".to_string()),
                    tool_calls: None,
                    reasoning_content: Some("Let me think...".to_string()),
                    annotations: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(ChatUsage { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 }),
        };

        let resp = chat_to_responses(&chat, None);
        assert_eq!(resp.output.len(), 2);

        // First output should be reasoning
        match &resp.output[0] {
            ResponsesOutputItem::Reasoning { id, status, summary, .. } => {
                assert!(id.as_ref().unwrap().starts_with("rs_"));
                assert_eq!(status.as_deref(), Some("completed"));
                match &summary[0] {
                    ReasoningSummaryBlock::SummaryText { text } => {
                        assert_eq!(text, "Let me think...");
                    }
                }
            }
            _ => panic!("Expected reasoning output item"),
        }

        // Second should be message
        match &resp.output[1] {
            ResponsesOutputItem::Message { .. } => {}
            _ => panic!("Expected message output item"),
        }
    }

    #[test]
    fn test_function_call_has_id_and_status() {
        let chat = ChatCompletionResponse {
            id: "chat-fc".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![ChatToolCall {
                        id: "call_abc".to_string(),
                        call_type: "function".to_string(),
                        function: ChatFunctionCall {
                            name: "get_weather".to_string(),
                            arguments: r#"{"city":"Paris"}"#.to_string(),
                        },
                    }]),
                    reasoning_content: None,
                    annotations: None,
                },
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: Some(ChatUsage { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 }),
        };

        let resp = chat_to_responses(&chat, None);
        match &resp.output[0] {
            ResponsesOutputItem::FunctionCall { id, status, call_id, name, .. } => {
                assert!(id.as_ref().unwrap().starts_with("fc_"));
                assert_eq!(status.as_deref(), Some("completed"));
                assert_eq!(call_id, "call_abc");
                assert_eq!(name, "get_weather");
            }
            _ => panic!("Expected function_call output"),
        }
    }

    #[test]
    fn test_resp_id_prefix() {
        let chat = make_chat_response("Hi", "stop");
        let resp = chat_to_responses(&chat, None);
        assert_eq!(resp.id, "resp_chat-1");
        assert_eq!(resp.object, "response");
    }

    #[test]
    fn test_echo_fields_from_request() {
        let chat = make_chat_response("Hi", "stop");
        let req = ResponsesRequest {
            model: "test".to_string(),
            instructions: Some("Be helpful".to_string()),
            input: serde_json::Value::Array(vec![]),
            tools: vec![],
            max_output_tokens: Some(100),
            stream: false,
            temperature: Some(0.7),
            top_p: Some(0.9),
            tool_choice: Some(serde_json::json!("auto")),
            previous_response_id: Some("resp_prev".to_string()),
            metadata: Some(serde_json::json!({"key": "value"})),
            truncation: Some("auto".to_string()),
            include: None,
            store: Some(true),
            reasoning: Some(serde_json::json!({"effort": "high"})),
            parallel_tool_calls: Some(true),
            service_tier: None,
            text: None,
            user: None,
        };
        let resp = chat_to_responses(&chat, Some(&req));
        assert_eq!(resp.temperature, Some(0.7));
        assert_eq!(resp.top_p, Some(0.9));
        assert_eq!(resp.max_output_tokens, Some(100));
        assert_eq!(resp.previous_response_id, Some("resp_prev".to_string()));
        assert_eq!(resp.store, Some(true));
        assert_eq!(resp.parallel_tool_calls, Some(true));
        assert_eq!(resp.truncation, Some("auto".to_string()));
        assert!(resp.metadata.is_some());
        assert!(resp.reasoning.is_some());
    }
}
