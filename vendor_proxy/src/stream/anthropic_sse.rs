use crate::types::anthropic::AnthropicSseEvent;

/// Translate Anthropic SSE events into Chat Completions SSE chunks.
/// Accumulates state across events (tracking current content block type).
pub struct AnthropicSseTranslator {
    response_id: String,
    model: String,
    current_block_type: Option<String>,
    current_block_index: u32,
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    current_tool_input: String,
    input_tokens: u32,
    output_tokens: u32,
    finish_reason: Option<String>,
}

impl Default for AnthropicSseTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicSseTranslator {
    pub fn new() -> Self {
        Self {
            response_id: String::new(),
            model: String::new(),
            current_block_type: None,
            current_block_index: 0,
            current_tool_id: None,
            current_tool_name: None,
            current_tool_input: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            finish_reason: None,
        }
    }

    /// Feed an Anthropic SSE event, return one or more Chat Completion SSE lines.
    pub fn feed(&mut self, event: AnthropicSseEvent) -> Vec<String> {
        let mut chunks = Vec::new();

        match event {
            AnthropicSseEvent::MessageStart { message } => {
                self.response_id = message.id.clone();
                self.model = message.model.clone();
                self.input_tokens = message.usage.input_tokens;
                chunks.push(self.chat_chunk_json(&ChatDelta {
                    content: Some(String::new()),
                    tool_calls: None,
                }));
            }
            AnthropicSseEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                self.current_block_index = index;
                match content_block.block_type.as_str() {
                    "text" => {
                        self.current_block_type = Some("text".to_string());
                    }
                    "tool_use" => {
                        self.current_block_type = Some("tool_use".to_string());
                        self.current_tool_id = content_block.id.clone();
                        self.current_tool_name = content_block.name.clone();
                        self.current_tool_input = String::new();
                    }
                    _ => {}
                }
            }
            AnthropicSseEvent::ContentBlockDelta { index, delta } => {
                self.current_block_index = index;
                match delta {
                    crate::types::anthropic::AnthropicSseDelta::TextDelta { text } => {
                        chunks.push(self.chat_chunk_json(&ChatDelta {
                            content: Some(text),
                            tool_calls: None,
                        }));
                    }
                    crate::types::anthropic::AnthropicSseDelta::InputJsonDelta { partial_json } => {
                        self.current_tool_input.push_str(&partial_json);
                        chunks.push(self.chat_chunk_json(&ChatDelta {
                            content: None,
                            tool_calls: Some(vec![ChatToolCallDelta {
                                index: self.current_block_index,
                                id: self.current_tool_id.clone(),
                                function: Some(ChatFunctionDelta {
                                    name: self.current_tool_name.clone(),
                                    arguments: partial_json,
                                }),
                            }]),
                        }));
                    }
                }
            }
            AnthropicSseEvent::ContentBlockStop { .. } => {
                if self.current_block_type.as_deref() == Some("tool_use") {
                    chunks.push(self.chat_chunk_json(&ChatDelta {
                        content: None,
                        tool_calls: Some(vec![ChatToolCallDelta {
                            index: self.current_block_index,
                            id: self.current_tool_id.clone(),
                            function: Some(ChatFunctionDelta {
                                name: self.current_tool_name.clone(),
                                arguments: String::new(),
                            }),
                        }]),
                    }));
                }
                self.current_block_type = None;
                self.current_tool_id = None;
                self.current_tool_name = None;
                self.current_tool_input.clear();
            }
            AnthropicSseEvent::MessageDelta { delta, usage } => {
                self.finish_reason = delta.stop_reason.map(|r| map_anthropic_stop_to_chat(&r));
                if let Some(u) = usage {
                    self.output_tokens = u.output_tokens;
                }
            }
            AnthropicSseEvent::MessageStop => {
                let finish_reason = self
                    .finish_reason
                    .clone()
                    .unwrap_or_else(|| "stop".to_string());
                chunks.extend(self.final_chunk_json(&finish_reason));
            }
            AnthropicSseEvent::Error { error } => {
                chunks.push(format!(
                    r#"{{"error":{{"message":"{}","type":"{}"}}}}"#,
                    error.message, error.error_type
                ));
            }
        }

        chunks
    }

    fn chat_chunk_json(&self, delta: &ChatDelta) -> String {
        let choices = vec![ChatStreamChoice {
            index: 0,
            delta,
            finish_reason: None,
        }];
        let chunk = ChatCompletionStreamChunk {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: now_unix(),
            model: self.model.clone(),
            choices,
            usage: None,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        format!("data: {}", json)
    }

    fn final_chunk_json(&self, finish_reason: &str) -> Vec<String> {
        let choices = vec![ChatStreamChoice {
            index: 0,
            delta: &ChatDelta {
                content: None,
                tool_calls: None,
            },
            finish_reason: Some(finish_reason.to_string()),
        }];
        let chunk = ChatCompletionStreamChunk {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: now_unix(),
            model: self.model.clone(),
            choices,
            usage: Some(ChatStreamUsage {
                prompt_tokens: self.input_tokens,
                completion_tokens: self.output_tokens,
                total_tokens: self.input_tokens + self.output_tokens,
            }),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        vec![format!("data: {}", json), "data: [DONE]".to_string()]
    }
}

fn map_anthropic_stop_to_chat(reason: &str) -> String {
    match reason {
        "end_turn" | "stop_sequence" => "stop".to_string(),
        "max_tokens" => "length".to_string(),
        "tool_use" => "tool_calls".to_string(),
        _ => "stop".to_string(),
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::anthropic::*;

    fn parse_events(lines: &[String]) -> Vec<serde_json::Value> {
        lines
            .iter()
            .filter_map(|l| {
                let data = l.strip_prefix("data: ").unwrap_or(l);
                serde_json::from_str(data).ok()
            })
            .collect()
    }

    #[test]
    fn test_text_stream_produces_chat_chunks() {
        let mut t = AnthropicSseTranslator::new();
        let mut all = Vec::new();

        all.extend(t.feed(AnthropicSseEvent::MessageStart {
            message: AnthropicMessageStart {
                id: "msg_1".to_string(),
                model: "test".to_string(),
                usage: AnthropicUsage {
                    input_tokens: 10,
                    output_tokens: 0,
                },
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockStart {
            index: 0,
            content_block: AnthropicSseContentBlock {
                block_type: "text".to_string(),
                id: None,
                name: None,
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockDelta {
            index: 0,
            delta: AnthropicSseDelta::TextDelta {
                text: "Hello".to_string(),
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockStop { index: 0 }));
        all.extend(t.feed(AnthropicSseEvent::MessageDelta {
            delta: AnthropicMessageDelta {
                stop_reason: Some("end_turn".to_string()),
            },
            usage: None,
        }));
        all.extend(t.feed(AnthropicSseEvent::MessageStop));

        let events = parse_events(&all);
        assert_eq!(events.len(), 3); // initial chunk, text delta, final chunk
        assert_eq!(
            events[0]["choices"][0]["delta"]["content"].as_str(),
            Some("")
        );
        assert_eq!(
            events[1]["choices"][0]["delta"]["content"].as_str(),
            Some("Hello")
        );
        assert_eq!(
            events[2]["choices"][0]["finish_reason"].as_str(),
            Some("stop")
        );
    }

    #[test]
    fn test_tool_use_stream_produces_tool_call_chunks() {
        let mut t = AnthropicSseTranslator::new();
        let mut all = Vec::new();

        all.extend(t.feed(AnthropicSseEvent::MessageStart {
            message: AnthropicMessageStart {
                id: "msg_1".to_string(),
                model: "test".to_string(),
                usage: AnthropicUsage {
                    input_tokens: 10,
                    output_tokens: 0,
                },
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockStart {
            index: 0,
            content_block: AnthropicSseContentBlock {
                block_type: "tool_use".to_string(),
                id: Some("tu_1".to_string()),
                name: Some("get_weather".to_string()),
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockDelta {
            index: 0,
            delta: AnthropicSseDelta::InputJsonDelta {
                partial_json: r#"{"city""#.to_string(),
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockDelta {
            index: 0,
            delta: AnthropicSseDelta::InputJsonDelta {
                partial_json: r#":"Paris"}"#.to_string(),
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockStop { index: 0 }));
        all.extend(t.feed(AnthropicSseEvent::MessageDelta {
            delta: AnthropicMessageDelta {
                stop_reason: Some("tool_use".to_string()),
            },
            usage: Some(AnthropicMessageUsage { output_tokens: 20 }),
        }));
        all.extend(t.feed(AnthropicSseEvent::MessageStop));

        let events = parse_events(&all);
        // Check tool call chunks exist
        let tool_chunks: Vec<_> = events
            .iter()
            .filter(|e| {
                e.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("tool_calls"))
                    .is_some()
            })
            .collect();
        assert!(tool_chunks.len() > 0);

        // Verify the first tool chunk has correct index
        let first_tc = tool_chunks[0]
            .get("choices")
            .unwrap()
            .get(0)
            .unwrap()
            .get("delta")
            .unwrap()
            .get("tool_calls")
            .unwrap()
            .get(0)
            .unwrap();
        assert_eq!(first_tc["index"], 0);
    }

    #[test]
    fn test_tool_use_index_from_content_block_start() {
        let mut t = AnthropicSseTranslator::new();
        let mut all = Vec::new();

        // Start with index 1 (second tool)
        all.extend(t.feed(AnthropicSseEvent::ContentBlockStart {
            index: 1,
            content_block: AnthropicSseContentBlock {
                block_type: "tool_use".to_string(),
                id: Some("tu_2".to_string()),
                name: Some("second_tool".to_string()),
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockDelta {
            index: 1,
            delta: AnthropicSseDelta::InputJsonDelta {
                partial_json: "{}".to_string(),
            },
        }));
        all.extend(t.feed(AnthropicSseEvent::ContentBlockStop { index: 1 }));

        let events = parse_events(&all);
        let tool_chunks: Vec<u64> = events
            .iter()
            .filter_map(|e| {
                e.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("tool_calls"))
                    .and_then(|tc| tc.get(0))
                    .and_then(|t| t.get("index"))
                    .and_then(|i| i.as_u64())
            })
            .collect();
        assert!(!tool_chunks.is_empty());
        assert_eq!(tool_chunks[0], 1);
    }

    #[test]
    fn test_stop_reason_mapping() {
        assert_eq!(map_anthropic_stop_to_chat("end_turn"), "stop");
        assert_eq!(map_anthropic_stop_to_chat("stop_sequence"), "stop");
        assert_eq!(map_anthropic_stop_to_chat("max_tokens"), "length");
        assert_eq!(map_anthropic_stop_to_chat("tool_use"), "tool_calls");
        assert_eq!(map_anthropic_stop_to_chat("unknown"), "stop");
    }

    #[test]
    fn test_final_chunk_has_usage() {
        let mut t = AnthropicSseTranslator::new();
        t.input_tokens = 15;
        t.response_id = "msg_1".to_string();
        t.model = "test".to_string();

        let events = t.feed(AnthropicSseEvent::MessageStop);
        let parsed = parse_events(&events);
        let final_chunk = parsed.iter().find(|e| {
            e.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("finish_reason"))
                .is_some()
        });
        assert!(final_chunk.is_some());
        let usage = final_chunk.unwrap().get("usage").unwrap();
        assert_eq!(usage["prompt_tokens"], 15);
    }
}

// Chat Completions streaming types (for SSE serialization)
#[derive(serde::Serialize)]
struct ChatCompletionStreamChunk<'a> {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatStreamChoice<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<ChatStreamUsage>,
}

#[derive(serde::Serialize)]
struct ChatStreamChoice<'a> {
    index: u32,
    delta: &'a ChatDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(serde::Serialize)]
struct ChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(serde::Serialize)]
struct ChatToolCallDelta {
    index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<ChatFunctionDelta>,
}

#[derive(serde::Serialize)]
struct ChatFunctionDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    arguments: String,
}

#[derive(serde::Serialize)]
struct ChatStreamUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
