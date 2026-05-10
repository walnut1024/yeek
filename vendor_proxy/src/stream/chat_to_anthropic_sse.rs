//! Translate Chat Completions SSE chunks into Anthropic Messages SSE events.
//!
//! Used when `/v1/messages` proxies to a Chat Completions backend.

/// Translates Chat Completions SSE chunks → Anthropic SSE events.
pub struct ChatToAnthropicSseTranslator {
    response_id: String,
    model: String,
    sent_message_start: bool,
    text_block_started: bool,
    current_block_index: u32,
    tool_states: std::collections::HashMap<u32, ToolState>,
    input_tokens: u32,
    output_tokens: u32,
    finished: bool,
}

#[derive(Default)]
struct ToolState {
    id: String,
    name: String,
    block_started: bool,
}

impl Default for ChatToAnthropicSseTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatToAnthropicSseTranslator {
    pub fn new() -> Self {
        Self {
            response_id: String::new(),
            model: String::new(),
            sent_message_start: false,
            text_block_started: false,
            current_block_index: 0,
            tool_states: std::collections::HashMap::new(),
            input_tokens: 0,
            output_tokens: 0,
            finished: false,
        }
    }

    /// Feed a raw Chat SSE data line, return Anthropic SSE event lines.
    pub fn feed(&mut self, line: &str) -> Vec<String> {
        let mut events = Vec::new();

        if line == "[DONE]" || line == "data: [DONE]" {
            events.extend(self.emit_close_events(None));
            return events;
        }

        let data = line.strip_prefix("data: ").unwrap_or(line);
        let parsed: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return events,
        };

        if self.response_id.is_empty() {
            self.response_id =
                parsed.get("id").and_then(|v| v.as_str()).unwrap_or("msg_unknown").to_string();
            self.model =
                parsed.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        }

        if let Some(usage) = parsed.get("usage") {
            self.input_tokens =
                usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            self.output_tokens =
                usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        }

        if let Some(choices) = parsed.get("choices").and_then(|v| v.as_array()) {
            for choice in choices {
                let delta = choice.get("delta");

                // Text content
                if let Some(content) = delta.and_then(|d| d.get("content")).and_then(|v| v.as_str())
                {
                    if !content.is_empty() {
                        events.extend(self.emit_message_start());
                        events.extend(self.emit_text_block_start());
                        events.push(sse_event(serde_json::json!({
                            "type": "content_block_delta",
                            "index": self.current_block_index,
                            "delta": {"type": "text_delta", "text": content}
                        })));
                    }
                }

                // Tool calls
                if let Some(tool_calls) =
                    delta.and_then(|d| d.get("tool_calls")).and_then(|v| v.as_array())
                {
                    for tc in tool_calls {
                        let index = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let func = tc.get("function");
                        let name = func
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let args = func
                            .and_then(|f| f.get("arguments"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        events.extend(self.emit_message_start());

                        let state = self.tool_states.entry(index).or_default();
                        if !tc_id.is_empty() {
                            state.id = tc_id;
                        }
                        if !name.is_empty() {
                            state.name = name;
                        }

                        let should_start = !state.block_started && !state.name.is_empty();
                        if should_start {
                            state.block_started = true;
                        }
                        let tool_id = state.id.clone();
                        let tool_name = state.name.clone();

                        if should_start {
                            self.current_block_index = index;

                            // Close any open text block first
                            if self.text_block_started {
                                events.push(sse_event(serde_json::json!({
                                    "type": "content_block_stop",
                                    "index": 0,
                                })));
                                self.text_block_started = false;
                            }

                            events.push(sse_event(serde_json::json!({
                                "type": "content_block_start",
                                "index": index,
                                "content_block": {
                                    "type": "tool_use",
                                    "id": tool_id,
                                    "name": tool_name,
                                    "input": {},
                                }
                            })));
                        }

                        if !args.is_empty() {
                            events.push(sse_event(serde_json::json!({
                                "type": "content_block_delta",
                                "index": index,
                                "delta": {"type": "input_json_delta", "partial_json": args}
                            })));
                        }
                    }
                }

                // Finish reason
                if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                    events.extend(self.emit_close_events(Some(reason)));
                }
            }
        }

        events
    }

    fn emit_message_start(&mut self) -> Vec<String> {
        if self.sent_message_start {
            return vec![];
        }
        self.sent_message_start = true;
        vec![sse_event(serde_json::json!({
            "type": "message_start",
            "message": {
                "id": format!("msg_{}", self.response_id),
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": self.model,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": self.input_tokens, "output_tokens": 0}
            }
        }))]
    }

    fn emit_text_block_start(&mut self) -> Vec<String> {
        if self.text_block_started {
            return vec![];
        }
        self.text_block_started = true;
        self.current_block_index = 0;
        vec![sse_event(serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": ""}
        }))]
    }

    fn emit_close_events(&mut self, finish_reason: Option<&str>) -> Vec<String> {
        if self.finished {
            return vec![];
        }
        self.finished = true;
        let mut events = Vec::new();

        events.extend(self.emit_message_start());

        let stop_reason = match finish_reason {
            Some("stop") => "end_turn",
            Some("length") => "max_tokens",
            Some("tool_calls") | Some("function_call") => "tool_use",
            _ => "end_turn",
        };

        // Close text block
        if self.text_block_started {
            events.push(sse_event(serde_json::json!({
                "type": "content_block_stop",
                "index": 0,
            })));
        }

        // Close tool blocks
        let mut tool_indices: Vec<u32> = self.tool_states.keys().copied().collect();
        tool_indices.sort();
        for idx in &tool_indices {
            events.push(sse_event(serde_json::json!({
                "type": "content_block_stop",
                "index": idx,
            })));
        }

        // message_delta with stop_reason and usage
        events.push(sse_event(serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": stop_reason, "stop_sequence": null},
            "usage": {"output_tokens": self.output_tokens}
        })));

        // message_stop
        events.push(sse_event(serde_json::json!({
            "type": "message_stop"
        })));

        events
    }
}

fn sse_event(data: serde_json::Value) -> String {
    let json = serde_json::to_string(&data)
        .unwrap_or_else(|_| r#"{"error":"json serialize"}"#.to_string());
    format!("data: {}", json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_chunk(id: &str, content: &str) -> String {
        format!(
            "data: {}",
            serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": 1700000000,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {"content": content}, "finish_reason": null}]
            })
        )
    }

    fn make_finish_chunk(id: &str) -> String {
        format!(
            "data: {}",
            serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": 1700000000,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            })
        )
    }

    fn make_tool_chunk(id: &str, index: u32, tc_id: &str, name: &str, args: &str) -> String {
        format!(
            "data: {}",
            serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": 1700000000,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {
                    "tool_calls": [{"index": index, "id": tc_id, "function": {"name": name, "arguments": args}}]
                }, "finish_reason": null}]
            })
        )
    }

    fn event_types(events: &[String]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| {
                let data = e.strip_prefix("data: ").unwrap_or(e);
                serde_json::from_str::<serde_json::Value>(data)
                    .ok()
                    .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
            })
            .collect()
    }

    #[test]
    fn test_text_streaming() {
        let mut t = ChatToAnthropicSseTranslator::new();
        let mut all = Vec::new();

        all.extend(t.feed(&make_text_chunk("chat-1", "Hello")));
        all.extend(t.feed(&make_text_chunk("chat-1", " world")));
        all.extend(t.feed(&make_finish_chunk("chat-1")));

        let types = event_types(&all);
        assert!(types.contains(&"message_start".to_string()));
        assert!(types.contains(&"content_block_start".to_string()));
        assert!(types.contains(&"content_block_delta".to_string()));
        assert!(types.contains(&"content_block_stop".to_string()));
        assert!(types.contains(&"message_delta".to_string()));
        assert!(types.contains(&"message_stop".to_string()));
    }

    #[test]
    fn test_message_start_first() {
        let mut t = ChatToAnthropicSseTranslator::new();
        let events = t.feed(&make_text_chunk("chat-1", "Hi"));
        let types = event_types(&events);
        assert_eq!(types[0], "message_start");
        assert_eq!(types[1], "content_block_start");
    }

    #[test]
    fn test_stop_reason_mapping() {
        let mut t = ChatToAnthropicSseTranslator::new();
        let finish = format!(
            "data: {}",
            serde_json::json!({
                "id": "chat-1",
                "object": "chat.completion.chunk",
                "created": 1700000000,
                "model": "test",
                "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            })
        );
        t.feed(&make_text_chunk("chat-1", "Hi"));
        let events = t.feed(&finish);

        let delta_event = events.iter().find_map(|e| {
            let data = e.strip_prefix("data: ")?;
            let v: serde_json::Value = serde_json::from_str(data).ok()?;
            (v.get("type")?.as_str()? == "message_delta").then_some(v)
        });
        assert_eq!(delta_event.unwrap()["delta"]["stop_reason"].as_str(), Some("tool_use"));
    }

    #[test]
    fn test_tool_use_streaming() {
        let mut t = ChatToAnthropicSseTranslator::new();
        let mut all = Vec::new();

        all.extend(t.feed(&make_tool_chunk("chat-1", 0, "call_1", "get_weather", "")));
        all.extend(t.feed(&make_tool_chunk("chat-1", 0, "", "", r#"{"city""#)));
        all.extend(t.feed(&make_tool_chunk("chat-1", 0, "", "", r#":"Paris"}"#)));
        all.extend(t.feed(&make_finish_chunk("chat-1")));

        let types = event_types(&all);
        assert!(types.contains(&"message_start".to_string()));
        assert!(types.contains(&"content_block_start".to_string()));
        assert!(types.contains(&"content_block_delta".to_string()));
        assert!(types.contains(&"content_block_stop".to_string()));
        assert!(types.contains(&"message_delta".to_string()));
        assert!(types.contains(&"message_stop".to_string()));
    }

    #[test]
    fn test_done_sentinel() {
        let mut t = ChatToAnthropicSseTranslator::new();
        let events = t.feed("[DONE]");
        let types = event_types(&events);
        assert!(types.contains(&"message_start".to_string()));
        assert!(types.contains(&"message_stop".to_string()));
    }
}
