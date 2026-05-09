//! Convert Chat Completions response → Anthropic Messages response.
//!
//! Used when `/v1/messages` receives a response from a Chat Completions backend.

use crate::types::anthropic::*;
use crate::types::chat::*;

/// Transform a Chat Completions response into an Anthropic Messages response.
pub fn chat_to_anthropic_response(chat: &ChatCompletionResponse) -> AnthropicResponse {
    let mut content: Vec<AnthropicContentBlock> = Vec::new();
    let mut stop_reason = "end_turn".to_string();

    for choice in &chat.choices {
        // Text content
        if let Some(ref text) = choice.message.content {
            if !text.is_empty() {
                content.push(AnthropicContentBlock::Text { text: text.clone() });
            }
        }

        // Tool calls → tool_use blocks
        if let Some(ref tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                content.push(AnthropicContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                });
            }
            stop_reason = "tool_use".to_string();
        }

        // Map finish_reason
        if let Some(ref reason) = choice.finish_reason {
            stop_reason = map_chat_finish_reason(reason);
        }
    }

    let usage = chat.usage.as_ref().map(|u| AnthropicUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
    }).unwrap_or(AnthropicUsage {
        input_tokens: 0,
        output_tokens: 0,
    });

    AnthropicResponse {
        id: chat.id.clone(),
        response_type: "message".to_string(),
        model: chat.model.clone(),
        content,
        stop_reason: Some(stop_reason),
        usage,
    }
}

fn map_chat_finish_reason(reason: &str) -> String {
    match reason {
        "stop" => "end_turn".to_string(),
        "length" => "max_tokens".to_string(),
        "tool_calls" | "function_call" => "tool_use".to_string(),
        _ => "end_turn".to_string(),
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
            usage: Some(ChatUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        }
    }

    #[test]
    fn test_text_response() {
        let chat = make_chat_response("Hello!", "stop");
        let resp = chat_to_anthropic_response(&chat);
        assert_eq!(resp.response_type, "message");
        assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(resp.content.len(), 1);
        match &resp.content[0] {
            AnthropicContentBlock::Text { text } => assert_eq!(text, "Hello!"),
            _ => panic!("Expected text block"),
        }
    }

    #[test]
    fn test_tool_use_response() {
        let chat = ChatCompletionResponse {
            id: "chat-2".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![ChatToolCall {
                        id: "call_1".to_string(),
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
            usage: Some(ChatUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };

        let resp = chat_to_anthropic_response(&chat);
        assert_eq!(resp.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(resp.content.len(), 1);
        match &resp.content[0] {
            AnthropicContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "get_weather");
                assert_eq!(input["city"], "Paris");
            }
            _ => panic!("Expected tool_use block"),
        }
    }

    #[test]
    fn test_length_finish_reason() {
        let chat = make_chat_response("truncated", "length");
        let resp = chat_to_anthropic_response(&chat);
        assert_eq!(resp.stop_reason.as_deref(), Some("max_tokens"));
    }

    #[test]
    fn test_usage_conversion() {
        let chat = make_chat_response("Hi", "stop");
        let resp = chat_to_anthropic_response(&chat);
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 5);
    }

    #[test]
    fn test_empty_content() {
        let chat = ChatCompletionResponse {
            id: "chat-3".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: None,
                    reasoning_content: None,
                    annotations: None,
                },
                finish_reason: None,
            }],
            usage: None,
        };

        let resp = chat_to_anthropic_response(&chat);
        assert!(resp.content.is_empty());
        assert_eq!(resp.usage.input_tokens, 0);
    }
}
