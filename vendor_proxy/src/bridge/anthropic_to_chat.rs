//! Convert Anthropic Messages request → Chat Completions request.
//!
//! Used when `/v1/messages` receives a request destined for a Chat Completions backend.

use crate::types::anthropic::*;
use crate::types::chat::*;

/// Transform an Anthropic Messages request into a Chat Completions request.
pub fn anthropic_to_chat(anthro: &AnthropicRequest) -> ChatCompletionRequest {
    let mut messages: Vec<ChatMessage> = Vec::new();

    // system → System message
    if let Some(ref sys) = anthro.system {
        if !sys.is_empty() {
            messages.push(ChatMessage::System { content: sys.clone() });
        }
    }

    // Anthropic messages → Chat messages
    for msg in &anthro.messages {
        match msg.role.as_str() {
            "user" => {
                let content = anthropic_content_to_chat(&msg.content);
                messages.push(ChatMessage::User { content });
            }
            "assistant" => {
                let (text, tool_calls) = anthropic_assistant_to_chat(&msg.content);
                messages.push(ChatMessage::Assistant {
                    content: text,
                    tool_calls,
                    reasoning_content: None,
                });
            }
            _ => {}
        }
    }

    // Anthropic tools → Chat tools
    let chat_tools: Vec<ChatTool> = anthro
        .tools
        .iter()
        .map(|t| ChatTool {
            tool_type: "function".to_string(),
            function: ChatToolFunction {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
                strict: None,
            },
        })
        .collect();

    let tool_choice = anthropic_to_chat_tool_choice(&anthro.tool_choice);

    ChatCompletionRequest {
        model: anthro.model.clone(),
        messages,
        tools: chat_tools,
        max_tokens: Some(anthro.max_tokens),
        stream: anthro.stream,
        temperature: anthro.temperature,
        top_p: anthro.top_p,
        tool_choice,
        stream_options: if anthro.stream {
            Some(StreamOptions { include_usage: true })
        } else {
            None
        },
        response_format: None,
        reasoning_effort: None,
        parallel_tool_calls: None,
        metadata: None,
        user: None,
        service_tier: None,
    }
}

/// Convert Anthropic content (user-facing) to Chat message content.
fn anthropic_content_to_chat(content: &AnthropicContent) -> ChatMessageContent {
    match content {
        AnthropicContent::String(s) => ChatMessageContent::String(s.clone()),
        AnthropicContent::Blocks(blocks) => {
            let mut parts: Vec<ContentPart> = Vec::new();
            let mut has_non_text = false;

            for block in blocks {
                match block {
                    AnthropicContentBlock::Text { text } => {
                        parts.push(ContentPart::Text { text: text.clone() });
                    }
                    AnthropicContentBlock::Image { source } => {
                        has_non_text = true;
                        let url = if source.source_type == "base64" {
                            format!("data:{};base64,{}", source.media_type, source.data)
                        } else {
                            format!("[Image: {}]", source.data)
                        };
                        parts.push(ContentPart::Image {
                            image_url: ImageUrl { url },
                        });
                    }
                    AnthropicContentBlock::ToolResult { tool_use_id, content: result_text } => {
                        // Tool result becomes a Tool message, but we're in user content here.
                        // Represent as text reference.
                        parts.push(ContentPart::Text {
                            text: format!("[Tool result for {}: {}]", tool_use_id, result_text),
                        });
                    }
                    AnthropicContentBlock::ToolUse { .. } => {
                        // Tool use in user content is unusual, skip
                    }
                }
            }

            if has_non_text || parts.len() > 1 {
                ChatMessageContent::Parts(parts)
            } else if parts.len() == 1 {
                match &parts[0] {
                    ContentPart::Text { text } => ChatMessageContent::String(text.clone()),
                    _ => ChatMessageContent::Parts(parts),
                }
            } else {
                ChatMessageContent::String(String::new())
            }
        }
    }
}

/// Extract text and tool_calls from an Anthropic assistant message's content blocks.
fn anthropic_assistant_to_chat(
    content: &AnthropicContent,
) -> (Option<String>, Option<Vec<ChatToolCall>>) {
    let blocks = match content {
        AnthropicContent::String(s) => {
            return (if s.is_empty() { None } else { Some(s.clone()) }, None);
        }
        AnthropicContent::Blocks(b) => b,
    };

    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ChatToolCall> = Vec::new();

    for block in blocks {
        match block {
            AnthropicContentBlock::Text { text } => {
                if !text.is_empty() {
                    text_parts.push(text.clone());
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

    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };
    let tc = if tool_calls.is_empty() { None } else { Some(tool_calls) };

    (text, tc)
}

/// Convert Anthropic tool_choice → Chat Completions tool_choice.
fn anthropic_to_chat_tool_choice(
    tool_choice: &Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match tool_choice {
        None => None,
        Some(serde_json::Value::Object(obj)) => {
            let tc_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match tc_type {
                "auto" => Some(serde_json::json!("auto")),
                "none" => Some(serde_json::json!("none")),
                "any" => Some(serde_json::json!("required")),
                "tool" => {
                    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                        Some(serde_json::json!({
                            "type": "function",
                            "function": {"name": name}
                        }))
                    } else {
                        Some(serde_json::json!("required"))
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_anthropic_req() -> AnthropicRequest {
        AnthropicRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            system: None,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            stream: false,
            temperature: None,
            top_p: None,
        }
    }

    #[test]
    fn test_basic_user_message() {
        let req = AnthropicRequest {
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::String("Hello".to_string()),
            }],
            ..make_anthropic_req()
        };

        let chat = anthropic_to_chat(&req);
        assert_eq!(chat.messages.len(), 1);
        match &chat.messages[0] {
            ChatMessage::User { content } => {
                assert_eq!(content, &ChatMessageContent::String("Hello".to_string()));
            }
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_system_message() {
        let req = AnthropicRequest {
            system: Some("You are helpful".to_string()),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::String("Hi".to_string()),
            }],
            ..make_anthropic_req()
        };

        let chat = anthropic_to_chat(&req);
        assert_eq!(chat.messages.len(), 2);
        assert!(matches!(chat.messages[0], ChatMessage::System { .. }));
    }

    #[test]
    fn test_assistant_with_tool_use() {
        let req = AnthropicRequest {
            messages: vec![AnthropicMessage {
                role: "assistant".to_string(),
                content: AnthropicContent::Blocks(vec![
                    AnthropicContentBlock::Text { text: "Let me check.".to_string() },
                    AnthropicContentBlock::ToolUse {
                        id: "tu_1".to_string(),
                        name: "get_weather".to_string(),
                        input: serde_json::json!({"city": "Paris"}),
                    },
                ]),
            }],
            ..make_anthropic_req()
        };

        let chat = anthropic_to_chat(&req);
        assert_eq!(chat.messages.len(), 1);
        match &chat.messages[0] {
            ChatMessage::Assistant { content, tool_calls, .. } => {
                assert_eq!(content.as_deref(), Some("Let me check."));
                let tc = tool_calls.as_ref().unwrap();
                assert_eq!(tc.len(), 1);
                assert_eq!(tc[0].function.name, "get_weather");
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_tool_choice_auto() {
        let req = AnthropicRequest {
            tool_choice: Some(serde_json::json!({"type": "auto"})),
            ..make_anthropic_req()
        };
        let chat = anthropic_to_chat(&req);
        assert_eq!(chat.tool_choice, Some(serde_json::json!("auto")));
    }

    #[test]
    fn test_tool_choice_tool_with_name() {
        let req = AnthropicRequest {
            tool_choice: Some(serde_json::json!({"type": "tool", "name": "my_func"})),
            ..make_anthropic_req()
        };
        let chat = anthropic_to_chat(&req);
        assert_eq!(
            chat.tool_choice,
            Some(serde_json::json!({"type": "function", "function": {"name": "my_func"}}))
        );
    }

    #[test]
    fn test_tools_conversion() {
        let req = AnthropicRequest {
            tools: vec![AnthropicTool {
                name: "search".to_string(),
                description: "Search the web".to_string(),
                input_schema: serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}}),
            }],
            ..make_anthropic_req()
        };
        let chat = anthropic_to_chat(&req);
        assert_eq!(chat.tools.len(), 1);
        assert_eq!(chat.tools[0].function.name, "search");
        assert_eq!(chat.tools[0].tool_type, "function");
    }

    #[test]
    fn test_stream_options() {
        let req = AnthropicRequest { stream: true, ..make_anthropic_req() };
        let chat = anthropic_to_chat(&req);
        assert!(chat.stream_options.is_some());
        assert!(chat.stream_options.as_ref().unwrap().include_usage);
    }
}
