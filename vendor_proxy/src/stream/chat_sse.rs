use std::collections::HashMap;

use crate::bridge::chat_to_responses::map_finish_reason_to_status;
use crate::types::responses::ResponsesRequest;

/// Translate Chat Completions SSE chunks into Responses API SSE events.
pub struct ChatSseToResponsesTranslator {
    response_id: String,
    model: String,
    // Event sequence tracking
    sent_response_created: bool,
    sent_response_in_progress: bool,
    sent_output_item_added: bool,
    sent_content_part_added: bool,
    sent_output_text_done: bool,
    sent_content_part_done: bool,
    sent_output_item_done: bool,
    finished: bool,
    // Tool call tracking: index -> (call_id, name, accumulated_args)
    tool_calls: HashMap<u32, ToolCallState>,
    pending_tool_call_index: Option<u32>,
    // Accumulated text
    accumulated_text: String,
    // Item IDs
    message_item_id: String,
    // Sequence numbers
    sequence_number: u64,
    // Usage from final streaming chunk
    input_tokens: u32,
    output_tokens: u32,
    total_tokens: u32,
    // Reasoning (Task 3)
    reasoning_active: bool,
    reasoning_done_emitted: bool,
    reasoning_item_id: Option<String>,
    accumulated_reasoning: String,
    // Finish reason → status (Task 4)
    finish_reason: Option<String>,
    // Annotations (Task 5)
    annotations: Vec<serde_json::Value>,
    // Echo fields (Task 6)
    echo_json: serde_json::Value,
}

#[derive(Default)]
struct ToolCallState {
    call_id: String,
    name: String,
    arguments: String,
    item_id: String,
    sent_added: bool,
    sent_done: bool,
}

impl Default for ChatSseToResponsesTranslator {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ChatSseToResponsesTranslator {
    pub fn new(echo_req: Option<&ResponsesRequest>) -> Self {
        let echo_json = echo_req.map(build_echo_json).unwrap_or(serde_json::json!({}));
        Self {
            response_id: String::new(),
            model: String::new(),
            sent_response_created: false,
            sent_response_in_progress: false,
            sent_output_item_added: false,
            sent_content_part_added: false,
            sent_output_text_done: false,
            sent_content_part_done: false,
            sent_output_item_done: false,
            finished: false,
            tool_calls: HashMap::new(),
            pending_tool_call_index: None,
            accumulated_text: String::new(),
            message_item_id: format!("msg_{}", uuid::Uuid::new_v4()),
            sequence_number: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            reasoning_active: false,
            reasoning_done_emitted: false,
            reasoning_item_id: None,
            accumulated_reasoning: String::new(),
            finish_reason: None,
            annotations: Vec::new(),
            echo_json,
        }
    }

    fn message_output_index(&self) -> usize {
        if self.reasoning_done_emitted {
            1
        } else {
            0
        }
    }

    /// Feed a Chat Completion SSE line (the data payload as JSON string).
    /// Returns one or more Responses SSE event lines.
    pub fn feed(&mut self, chat_line: &str) -> Vec<String> {
        let mut events = Vec::new();

        if chat_line == "[DONE]" || chat_line == "data: [DONE]" {
            // Emit final events
            events.extend(self.emit_done_events());
            return events;
        }

        let data = if let Some(stripped) = chat_line.strip_prefix("data: ") {
            stripped
        } else {
            chat_line
        };

        let parsed: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return events,
        };

        // Extract response_id and model from first chunk
        if self.response_id.is_empty() {
            if let Some(id) = parsed.get("id").and_then(|v| v.as_str()) {
                self.response_id = format!("resp_{}", id);
            }
            if let Some(model) = parsed.get("model").and_then(|v| v.as_str()) {
                self.model = model.to_string();
            }
        }

        // Emit initial events (response.created, response.in_progress)
        events.extend(self.emit_initial_events());

        // Check for error
        if let Some(error) = parsed.get("error") {
            events.push(self.sse_event(serde_json::json!({
                "type": "error",
                "error": error,
            })));
            return events;
        }

        // Extract usage before processing choices (needed for response.completed)
        if let Some(usage) = parsed.get("usage") {
            self.input_tokens =
                usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            self.output_tokens =
                usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            self.total_tokens =
                usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            tracing::info!(
                "Stream usage: input={}, output={}, total={}",
                self.input_tokens,
                self.output_tokens,
                self.total_tokens
            );
        }

        // Process choices
        if let Some(choices) = parsed.get("choices").and_then(|v| v.as_array()) {
            for choice in choices {
                let delta = choice.get("delta");

                // Reasoning content (Task 3) — check before regular content
                if let Some(reasoning) =
                    delta.and_then(|d| d.get("reasoning_content")).and_then(|v| v.as_str())
                {
                    if !reasoning.is_empty() {
                        events.extend(self.emit_reasoning_start_if_needed());
                        self.accumulated_reasoning.push_str(reasoning);
                        self.sequence_number += 1;
                        events.push(self.sse_event(serde_json::json!({
                            "type": "response.reasoning_summary_text.delta",
                            "item_id": self.reasoning_item_id,
                            "output_index": 0,
                            "summary_index": 0,
                            "delta": reasoning,
                        })));
                    }
                }

                // Text delta
                if let Some(content) = delta.and_then(|d| d.get("content")).and_then(|v| v.as_str())
                {
                    if !content.is_empty() {
                        events.extend(self.emit_reasoning_done_if_needed());

                        events.extend(self.emit_message_start_events());

                        self.accumulated_text.push_str(content);
                        self.sequence_number += 1;

                        events.push(self.sse_event(serde_json::json!({
                            "type": "response.output_text.delta",
                            "item_id": self.message_item_id,
                            "output_index": self.message_output_index(),
                            "content_index": 0,
                            "delta": content,
                        })));
                    }
                }

                // Annotations (Task 5)
                if let Some(annots) =
                    delta.and_then(|d| d.get("annotations")).and_then(|v| v.as_array())
                {
                    for annot in annots {
                        let transformed = transform_annotation(annot);
                        self.annotations.push(transformed.clone());
                        self.sequence_number += 1;
                        events.push(self.sse_event(serde_json::json!({
                            "type": "response.output_text.annotation.added",
                            "item_id": self.message_item_id,
                            "output_index": self.message_output_index(),
                            "content_index": 0,
                            "annotation_index": self.annotations.len() - 1,
                            "annotation": transformed,
                        })));
                    }
                }

                // Tool call delta
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

                        // Track this tool call - update state then extract values
                        let should_emit_added;
                        let item_id_clone;
                        let call_id_clone;
                        let name_clone;
                        {
                            let state =
                                self.tool_calls.entry(index).or_insert_with(|| ToolCallState {
                                    call_id: String::new(),
                                    name: String::new(),
                                    arguments: String::new(),
                                    item_id: format!("fc_{}", uuid::Uuid::new_v4()),
                                    sent_added: false,
                                    sent_done: false,
                                });

                            if !tc_id.is_empty() {
                                state.call_id = tc_id;
                            }
                            if !name.is_empty() {
                                state.name = name;
                            }

                            should_emit_added = !state.sent_added && !state.name.is_empty();
                            if should_emit_added {
                                state.sent_added = true;
                            }
                            if !args.is_empty() {
                                state.arguments.push_str(&args);
                            }
                            item_id_clone = state.item_id.clone();
                            call_id_clone = state.call_id.clone();
                            name_clone = state.name.clone();
                        }

                        // Emit output_item.added for function_call
                        if should_emit_added {
                            self.sequence_number += 1;
                            events.push(self.sse_event(serde_json::json!({
                                "type": "response.output_item.added",
                                "output_index": index,
                                "item": {
                                    "type": "function_call",
                                    "id": item_id_clone,
                                    "call_id": call_id_clone,
                                    "name": name_clone,
                                    "arguments": "",
                                    "status": "in_progress",
                                },
                            })));
                        }

                        // Emit argument deltas in ~10-char chunks (matching OpenAI behavior)
                        if !args.is_empty() {
                            for chunk in split_into_chunks(&args, 10) {
                                self.sequence_number += 1;
                                let iid = self
                                    .tool_calls
                                    .get(&index)
                                    .map(|s| s.item_id.clone())
                                    .unwrap_or_default();
                                events.push(self.sse_event(serde_json::json!({
                                    "type": "response.function_call_arguments.delta",
                                    "item_id": iid,
                                    "output_index": index,
                                    "delta": chunk,
                                })));
                            }
                        }

                        self.pending_tool_call_index = Some(index);
                    }
                }

                // Finish reason (Task 4) — store for status mapping
                if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                    self.finish_reason = Some(reason.to_string());
                    events.extend(self.emit_done_events());
                }
            }
        }

        events
    }

    /// Emit response.created and response.in_progress events (once).
    fn emit_initial_events(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if !self.sent_response_created {
            self.sent_response_created = true;
            self.sequence_number += 1;
            let mut response_obj = serde_json::json!({
                "id": self.response_id,
                "object": "response",
                "status": "in_progress",
                "model": self.model,
                "output": [],
            });
            if let Some(obj) = response_obj.as_object_mut() {
                if let Some(echo) = self.echo_json.as_object() {
                    for (k, v) in echo {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
            events.push(self.sse_event(serde_json::json!({
                "type": "response.created",
                "response": response_obj,
            })));
        }
        if !self.sent_response_in_progress {
            self.sent_response_in_progress = true;
            self.sequence_number += 1;
            events.push(self.sse_event(serde_json::json!({
                "type": "response.in_progress",
                "response": {
                    "id": self.response_id,
                    "object": "response",
                    "status": "in_progress",
                    "model": self.model,
                    "output": [],
                },
            })));
        }
        events
    }

    /// Emit output_item.added and content_part.added for text message (once).
    fn emit_message_start_events(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        let msg_idx = self.message_output_index();
        if !self.sent_output_item_added {
            self.sent_output_item_added = true;
            self.sequence_number += 1;
            events.push(self.sse_event(serde_json::json!({
                "type": "response.output_item.added",
                "output_index": msg_idx,
                "item": {
                    "type": "message",
                    "id": self.message_item_id,
                    "status": "in_progress",
                    "role": "assistant",
                    "content": [],
                },
            })));
        }
        if !self.sent_content_part_added {
            self.sent_content_part_added = true;
            self.sequence_number += 1;
            events.push(self.sse_event(serde_json::json!({
                "type": "response.content_part.added",
                "item_id": self.message_item_id,
                "output_index": msg_idx,
                "content_index": 0,
                "part": {
                    "type": "output_text",
                    "text": "",
                    "annotations": [],
                },
            })));
        }
        events
    }

    /// Emit the final done events and response.completed.
    fn emit_done_events(&mut self) -> Vec<String> {
        if self.finished {
            return vec![];
        }
        let mut events = Vec::new();

        // Close reasoning if still active (Task 3)
        events.extend(self.emit_reasoning_done_if_needed());

        // Determine status from finish_reason (Task 4)
        let status = map_finish_reason_to_status(self.finish_reason.as_deref());

        // Emit tool call done events
        let mut tool_indices: Vec<u32> = self.tool_calls.keys().copied().collect();
        tool_indices.sort();
        for idx in &tool_indices {
            let (should_emit, item_id, call_id, name, arguments);
            {
                let state = self.tool_calls.get_mut(idx).unwrap();
                should_emit = !state.sent_done && state.sent_added;
                if should_emit {
                    state.sent_done = true;
                }
                item_id = state.item_id.clone();
                call_id = state.call_id.clone();
                name = state.name.clone();
                arguments = state.arguments.clone();
            }
            if should_emit {
                self.sequence_number += 1;
                events.push(self.sse_event(serde_json::json!({
                    "type": "response.function_call_arguments.done",
                    "item_id": item_id,
                    "output_index": *idx,
                    "arguments": arguments,
                })));
                self.sequence_number += 1;
                events.push(self.sse_event(serde_json::json!({
                    "type": "response.output_item.done",
                    "output_index": *idx,
                    "item": {
                        "type": "function_call",
                        "id": item_id,
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments,
                        "status": status,
                    },
                })));
            }
        }

        // Build annotations JSON (Task 5)
        let annots_json = serde_json::Value::Array(self.annotations.clone());
        let msg_idx = self.message_output_index();

        // Emit text done events
        if self.sent_content_part_added && !self.sent_output_text_done {
            self.sent_output_text_done = true;
            self.sequence_number += 1;
            events.push(self.sse_event(serde_json::json!({
                "type": "response.output_text.done",
                "item_id": self.message_item_id,
                "output_index": msg_idx,
                "content_index": 0,
                "text": self.accumulated_text,
            })));
        }
        if self.sent_content_part_added && !self.sent_content_part_done {
            self.sent_content_part_done = true;
            self.sequence_number += 1;
            events.push(self.sse_event(serde_json::json!({
                "type": "response.content_part.done",
                "item_id": self.message_item_id,
                "output_index": msg_idx,
                "content_index": 0,
                "part": {
                    "type": "output_text",
                    "text": self.accumulated_text,
                    "annotations": annots_json,
                },
            })));
        }
        if self.sent_output_item_added && !self.sent_output_item_done {
            self.sent_output_item_done = true;
            self.sequence_number += 1;
            events.push(self.sse_event(serde_json::json!({
                "type": "response.output_item.done",
                "output_index": msg_idx,
                "item": {
                    "type": "message",
                    "id": self.message_item_id,
                    "status": status,
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": self.accumulated_text,
                        "annotations": annots_json,
                    }],
                },
            })));
        }

        // Emit response.completed
        self.finished = true;
        self.sequence_number += 1;

        // Build output items
        let mut output_items = Vec::new();
        // Reasoning item (Task 3)
        if self.reasoning_done_emitted {
            output_items.push(serde_json::json!({
                "type": "reasoning",
                "id": self.reasoning_item_id,
                "status": status,
                "role": "assistant",
                "summary": [{
                    "type": "summary_text",
                    "text": self.accumulated_reasoning,
                }],
            }));
        }
        // Text message item
        if self.sent_output_item_added {
            let mut msg_content = Vec::new();
            // Keep reasoning in its own output item only, not embedded in the
            // message content. Mixed content blocks (reasoning_text + output_text)
            // may cause some Clients to fall back to raw-text rendering.
            msg_content.push(serde_json::json!({
                "type": "output_text",
                "text": self.accumulated_text,
                "annotations": annots_json,
            }));
            output_items.push(serde_json::json!({
                "type": "message",
                "id": self.message_item_id,
                "status": status,
                "role": "assistant",
                "content": msg_content,
            }));
        }
        for idx in &tool_indices {
            let state = self.tool_calls.get(idx).unwrap();
            output_items.push(serde_json::json!({
                "type": "function_call",
                "id": state.item_id,
                "call_id": state.call_id,
                "name": state.name,
                "arguments": state.arguments,
                "status": status,
            }));
        }

        // Build usage object if we have token counts
        let usage = if self.total_tokens > 0 {
            Some(serde_json::json!({
                "input_tokens": self.input_tokens,
                "output_tokens": self.output_tokens,
                "total_tokens": self.total_tokens,
            }))
        } else {
            None
        };

        let mut response_obj = serde_json::json!({
            "id": self.response_id,
            "object": "response",
            "status": status,
            "model": self.model,
            "output": output_items,
        });
        if let Some(u) = usage {
            response_obj["usage"] = u;
        }
        // Merge echo fields (Task 6)
        if let Some(obj) = response_obj.as_object_mut() {
            if let Some(echo) = self.echo_json.as_object() {
                for (k, v) in echo {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        events.push(self.sse_event(serde_json::json!({
            "type": "response.completed",
            "response": response_obj,
        })));

        events
    }

    fn sse_event(&self, data: serde_json::Value) -> String {
        format!(
            "data: {}",
            serde_json::to_string(&data)
                .unwrap_or_else(|_| r#"{"error":"json serialize"}"#.to_string())
        )
    }

    fn emit_reasoning_start_if_needed(&mut self) -> Vec<String> {
        if self.reasoning_active {
            return vec![];
        }
        self.reasoning_active = true;
        self.reasoning_item_id = Some(format!("rs_{}", uuid::Uuid::new_v4()));
        self.sequence_number += 1;
        vec![self.sse_event(serde_json::json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": {
                "type": "reasoning",
                "id": self.reasoning_item_id,
                "status": "in_progress",
                "role": "assistant",
                "content": [],
            },
        }))]
    }

    fn emit_reasoning_done_if_needed(&mut self) -> Vec<String> {
        if !self.reasoning_active || self.reasoning_done_emitted {
            return vec![];
        }
        self.reasoning_done_emitted = true;
        let mut events = Vec::new();
        let item_id = self.reasoning_item_id.clone().unwrap_or_default();

        self.sequence_number += 1;
        events.push(self.sse_event(serde_json::json!({
            "type": "response.reasoning_summary_text.done",
            "item_id": item_id,
            "output_index": 0,
            "summary_index": 0,
            "text": self.accumulated_reasoning,
        })));

        self.sequence_number += 1;
        events.push(self.sse_event(serde_json::json!({
            "type": "response.reasoning_summary_part.done",
            "item_id": item_id,
            "output_index": 0,
            "summary_index": 0,
            "part": {
                "type": "summary_text",
                "text": self.accumulated_reasoning,
            },
        })));

        self.sequence_number += 1;
        events.push(self.sse_event(serde_json::json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": {
                "type": "reasoning",
                "id": item_id,
                "status": "completed",
                "role": "assistant",
                "summary": [{
                    "type": "summary_text",
                    "text": self.accumulated_reasoning,
                }],
            },
        })));

        events
    }
}

fn build_echo_json(req: &ResponsesRequest) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(ref v) = req.instructions {
        map.insert("instructions".into(), serde_json::Value::String(v.clone()));
    }
    if let Some(v) = req.temperature {
        map.insert("temperature".into(), serde_json::json!(v));
    }
    if let Some(v) = req.top_p {
        map.insert("top_p".into(), serde_json::json!(v));
    }
    if !req.tools.is_empty() {
        let tools: Vec<_> = req
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": t.tool_type,
                    "name": t.name,
                })
            })
            .collect();
        map.insert("tools".into(), serde_json::Value::Array(tools));
    }
    if let Some(ref v) = req.tool_choice {
        map.insert("tool_choice".into(), v.clone());
    }
    if let Some(ref v) = req.reasoning {
        map.insert("reasoning".into(), v.clone());
    }
    if let Some(v) = req.store {
        map.insert("store".into(), serde_json::json!(v));
    }
    if let Some(ref v) = req.metadata {
        map.insert("metadata".into(), v.clone());
    }
    if let Some(ref v) = req.truncation {
        map.insert("truncation".into(), serde_json::Value::String(v.clone()));
    }
    if let Some(v) = req.max_output_tokens {
        map.insert("max_output_tokens".into(), serde_json::json!(v));
    }
    if let Some(v) = req.parallel_tool_calls {
        map.insert("parallel_tool_calls".into(), serde_json::json!(v));
    }
    serde_json::Value::Object(map)
}

fn transform_annotation(annot: &serde_json::Value) -> serde_json::Value {
    let annot_type = annot.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if annot_type == "url_citation" {
        if let Some(citation) = annot.get("url_citation") {
            let url = citation.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let title = citation.get("title").and_then(|v| v.as_str()).unwrap_or("");
            return serde_json::json!({
                "type": "url_citation",
                "start_index": 0,
                "end_index": 0,
                "url": url,
                "title": title,
            });
        }
    }
    annot.clone()
}

/// Split a string into chunks of approximately `chunk_size` characters.
fn split_into_chunks(s: &str, chunk_size: usize) -> Vec<&str> {
    if chunk_size == 0 || s.is_empty() {
        return vec![];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < s.len() {
        let end = std::cmp::min(start + chunk_size, s.len());
        // Ensure we don't split in the middle of a multi-byte char
        let end = match s.is_char_boundary(end) {
            true => end,
            false => {
                let mut e = end;
                while e > start && !s.is_char_boundary(e) {
                    e -= 1;
                }
                e
            }
        };
        chunks.push(&s[start..end]);
        start = end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_chunk(id: &str, content: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "test-model",
            "choices": [{"index": 0, "delta": {"content": content}, "finish_reason": null}]
        })
        .to_string()
    }

    fn make_finish_chunk(id: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "test-model",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })
        .to_string()
    }

    fn make_tool_call_chunk(id: &str, index: u32, tc_id: &str, name: &str, args: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "test-model",
            "choices": [{"index": 0, "delta": {
                "tool_calls": [{"index": index, "id": tc_id, "function": {"name": name, "arguments": args}}]
            }, "finish_reason": null}]
        }).to_string()
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
    fn test_text_streaming_produces_correct_events() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let mut all_events = Vec::new();

        all_events.extend(t.feed(&make_text_chunk("chat-1", "Hello")));
        all_events.extend(t.feed(&make_text_chunk("chat-1", " world")));
        all_events.extend(t.feed(&make_finish_chunk("chat-1")));

        let types = event_types(&all_events);
        assert!(types.contains(&"response.created".to_string()));
        assert!(types.contains(&"response.in_progress".to_string()));
        assert!(types.contains(&"response.output_item.added".to_string()));
        assert!(types.contains(&"response.content_part.added".to_string()));
        assert!(types.contains(&"response.output_text.delta".to_string()));
        assert!(types.contains(&"response.output_text.done".to_string()));
        assert!(types.contains(&"response.content_part.done".to_string()));
        assert!(types.contains(&"response.output_item.done".to_string()));
        assert!(types.contains(&"response.completed".to_string()));
    }

    #[test]
    fn test_response_created_first() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let events = t.feed(&make_text_chunk("chat-1", "Hi"));
        let types = event_types(&events);
        assert_eq!(types[0], "response.created");
        assert_eq!(types[1], "response.in_progress");
    }

    #[test]
    fn test_completed_is_last_event() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_text_chunk("chat-1", "Hi"));
        let events = t.feed(&make_finish_chunk("chat-1"));
        let types = event_types(&events);
        assert_eq!(types.last(), Some(&"response.completed".to_string()));
    }

    #[test]
    fn test_response_id_prefix() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_text_chunk("chat-abc", "Hi"));
        assert!(t.response_id.starts_with("resp_"));
    }

    #[test]
    fn test_tool_call_streaming() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let mut all_events = Vec::new();

        all_events.extend(t.feed(&make_tool_call_chunk("chat-1", 0, "call_1", "get_weather", "")));
        all_events.extend(t.feed(&make_tool_call_chunk("chat-1", 0, "", "", r#"{"city""#)));
        all_events.extend(t.feed(&make_tool_call_chunk("chat-1", 0, "", "", r#":"Paris"}"#)));
        all_events.extend(t.feed(&make_finish_chunk("chat-1")));

        let types = event_types(&all_events);
        assert!(types.contains(&"response.output_item.added".to_string()));
        assert!(types.contains(&"response.function_call_arguments.delta".to_string()));
        assert!(types.contains(&"response.function_call_arguments.done".to_string()));
        assert!(types.contains(&"response.completed".to_string()));
    }

    #[test]
    fn test_multi_tool_call_indices() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let mut all_events = Vec::new();

        all_events.extend(t.feed(&make_tool_call_chunk("chat-1", 0, "call_1", "tool_a", "")));
        all_events.extend(t.feed(&make_tool_call_chunk("chat-1", 1, "call_2", "tool_b", "")));
        all_events.extend(t.feed(&make_finish_chunk("chat-1")));

        // Verify two different tool call item IDs
        let added_indices: Vec<u32> = all_events
            .iter()
            .filter_map(|e| {
                let data = e.strip_prefix("data: ").unwrap_or(e);
                let v: serde_json::Value = serde_json::from_str(data).ok()?;
                if v.get("type")?.as_str()? == "response.output_item.added" {
                    v.get("output_index")?.as_u64().map(|i| i as u32)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(added_indices.len(), 2);
        assert_ne!(added_indices[0], added_indices[1]);
    }

    #[test]
    fn test_completed_event_has_output() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_text_chunk("chat-1", "Hello"));
        let events = t.feed(&make_finish_chunk("chat-1"));

        let completed: Option<serde_json::Value> = events.iter().find_map(|e| {
            let data = e.strip_prefix("data: ").unwrap_or(e);
            let v: serde_json::Value = serde_json::from_str(data).ok()?;
            (v.get("type")?.as_str()? == "response.completed").then_some(v)
        });

        let completed = completed.unwrap();
        let resp = completed.get("response").unwrap();
        assert_eq!(resp.get("status").unwrap().as_str(), Some("completed"));
        assert!(!resp.get("output").unwrap().as_array().unwrap().is_empty());
    }

    #[test]
    fn test_completed_event_has_usage() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_text_chunk("chat-1", "Hi"));
        let events = t.feed(&make_finish_chunk("chat-1"));

        let completed: Option<serde_json::Value> = events.iter().find_map(|e| {
            let data = e.strip_prefix("data: ").unwrap_or(e);
            let v: serde_json::Value = serde_json::from_str(data).ok()?;
            (v.get("type")?.as_str()? == "response.completed").then_some(v)
        });

        let completed = completed.unwrap();
        let resp = completed.get("response").unwrap();
        assert!(resp.get("usage").is_some());
        assert_eq!(resp["usage"]["input_tokens"], 10);
        assert_eq!(resp["usage"]["output_tokens"], 5);
    }

    #[test]
    fn test_done_sentinel_handling() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let events = t.feed("[DONE]");
        let types = event_types(&events);
        assert!(types.contains(&"response.completed".to_string()));
    }

    // --- Task 3: Reasoning tests ---

    fn make_reasoning_chunk(id: &str, reasoning: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "test-model",
            "choices": [{"index": 0, "delta": {"reasoning_content": reasoning}, "finish_reason": null}]
        })
        .to_string()
    }

    #[test]
    fn test_reasoning_events_before_text() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let mut all_events = Vec::new();

        all_events.extend(t.feed(&make_reasoning_chunk("chat-1", "Thinking")));
        all_events.extend(t.feed(&make_reasoning_chunk("chat-1", " hard")));
        all_events.extend(t.feed(&make_text_chunk("chat-1", "Answer")));
        all_events.extend(t.feed(&make_finish_chunk("chat-1")));

        let types = event_types(&all_events);

        // Reasoning events
        assert!(types.contains(&"response.reasoning_summary_text.delta".to_string()));
        assert!(types.contains(&"response.reasoning_summary_text.done".to_string()));
        assert!(types.contains(&"response.reasoning_summary_part.done".to_string()));

        // Text events
        assert!(types.contains(&"response.output_text.delta".to_string()));
        assert!(types.contains(&"response.output_text.done".to_string()));

        // Reasoning output_item.done comes before text output_item.done
        let reasoning_done_pos =
            types.iter().position(|t| t == "response.reasoning_summary_text.done").unwrap();
        let text_done_pos = types.iter().position(|t| t == "response.output_text.done").unwrap();
        assert!(reasoning_done_pos < text_done_pos);
    }

    #[test]
    fn test_reasoning_item_in_completed_output() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_reasoning_chunk("chat-1", "Hmm"));
        t.feed(&make_text_chunk("chat-1", "Ok"));
        let events = t.feed(&make_finish_chunk("chat-1"));

        let completed = find_completed(&events);
        let output = completed["response"]["output"].as_array().unwrap();
        assert!(output[0].get("type").unwrap().as_str() == Some("reasoning"));
        assert!(output[1].get("type").unwrap().as_str() == Some("message"));
    }

    // --- Task 4: Finish reason → status tests ---

    fn make_finish_chunk_with_reason(id: &str, reason: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "test-model",
            "choices": [{"index": 0, "delta": {}, "finish_reason": reason}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })
        .to_string()
    }

    #[test]
    fn test_length_finish_reason_gives_incomplete_status() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_text_chunk("chat-1", "Hello"));
        let events = t.feed(&make_finish_chunk_with_reason("chat-1", "length"));

        let completed = find_completed(&events);
        assert_eq!(completed["response"]["status"].as_str(), Some("incomplete"));
    }

    #[test]
    fn test_stop_finish_reason_gives_completed_status() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        t.feed(&make_text_chunk("chat-1", "Hello"));
        let events = t.feed(&make_finish_chunk_with_reason("chat-1", "stop"));

        let completed = find_completed(&events);
        assert_eq!(completed["response"]["status"].as_str(), Some("completed"));
    }

    // --- Task 5: Annotations tests ---

    fn make_text_chunk_with_annotations(id: &str, content: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": content,
                    "annotations": [{"type": "url_citation", "url_citation": {"url": "https://example.com", "title": "Example"}}]
                },
                "finish_reason": null
            }]
        })
        .to_string()
    }

    #[test]
    fn test_annotation_events_emitted() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let mut all_events = Vec::new();

        all_events.extend(t.feed(&make_text_chunk_with_annotations("chat-1", "See ")));
        all_events.extend(t.feed(&make_finish_chunk("chat-1")));

        let types = event_types(&all_events);
        assert!(types.contains(&"response.output_text.annotation.added".to_string()));
    }

    #[test]
    fn test_url_citation_transformed() {
        let mut t = ChatSseToResponsesTranslator::new(None);
        let mut all_events = Vec::new();

        all_events.extend(t.feed(&make_text_chunk_with_annotations("chat-1", "See ")));
        all_events.extend(t.feed(&make_finish_chunk("chat-1")));

        // Check annotation.added event
        let annot_event = all_events
            .iter()
            .find_map(|e| {
                let data = e.strip_prefix("data: ")?;
                let v: serde_json::Value = serde_json::from_str(data).ok()?;
                (v.get("type")?.as_str()? == "response.output_text.annotation.added").then_some(v)
            })
            .unwrap();

        let annot = &annot_event["annotation"];
        assert_eq!(annot["type"].as_str(), Some("url_citation"));
        assert_eq!(annot["url"].as_str(), Some("https://example.com"));
        assert_eq!(annot["title"].as_str(), Some("Example"));

        // Check content_part.done includes annotations
        let content_done = find_event_by_type(&all_events, "response.content_part.done");
        let annots = content_done["part"]["annotations"].as_array().unwrap();
        assert_eq!(annots.len(), 1);
    }

    // --- Task 6: Echo fields tests ---

    #[test]
    fn test_echo_fields_in_response_created() {
        let req = ResponsesRequest {
            model: "test".to_string(),
            instructions: Some("Be helpful".to_string()),
            temperature: Some(0.7),
            top_p: None,
            input: serde_json::json!("hi"),
            tools: vec![],
            max_output_tokens: None,
            stream: true,
            tool_choice: None,
            previous_response_id: None,
            metadata: None,
            truncation: None,
            include: None,
            store: None,
            reasoning: None,
            parallel_tool_calls: None,
            service_tier: None,
            text: None,
            user: None,
        };
        let mut t = ChatSseToResponsesTranslator::new(Some(&req));
        let events = t.feed(&make_text_chunk("chat-1", "Hi"));

        let created = find_event_by_type(&events, "response.created");
        assert_eq!(created["response"]["instructions"].as_str(), Some("Be helpful"));
        assert_eq!(created["response"]["temperature"].as_f64(), Some(0.7));
    }

    #[test]
    fn test_echo_fields_in_response_completed() {
        let req = ResponsesRequest {
            model: "test".to_string(),
            instructions: None,
            temperature: None,
            top_p: Some(0.9),
            input: serde_json::json!("hi"),
            tools: vec![],
            max_output_tokens: Some(100),
            stream: true,
            tool_choice: None,
            previous_response_id: None,
            metadata: None,
            truncation: Some("auto".to_string()),
            include: None,
            store: Some(true),
            reasoning: None,
            parallel_tool_calls: None,
            service_tier: None,
            text: None,
            user: None,
        };
        let mut t = ChatSseToResponsesTranslator::new(Some(&req));
        t.feed(&make_text_chunk("chat-1", "Hi"));
        let events = t.feed(&make_finish_chunk("chat-1"));

        let completed = find_completed(&events);
        assert_eq!(completed["response"]["top_p"].as_f64(), Some(0.9));
        assert_eq!(completed["response"]["max_output_tokens"].as_u64(), Some(100));
        assert_eq!(completed["response"]["truncation"].as_str(), Some("auto"));
        assert_eq!(completed["response"]["store"].as_bool(), Some(true));
    }

    fn find_completed(events: &[String]) -> serde_json::Value {
        find_event_by_type(events, "response.completed")
    }

    fn find_event_by_type(events: &[String], event_type: &str) -> serde_json::Value {
        events
            .iter()
            .find_map(|e| {
                let data = e.strip_prefix("data: ")?;
                let v: serde_json::Value = serde_json::from_str(data).ok()?;
                (v.get("type")?.as_str()? == event_type).then_some(v)
            })
            .unwrap()
    }

    #[test]
    fn test_split_into_chunks() {
        assert_eq!(split_into_chunks("abcdefghij", 5), vec!["abcde", "fghij"]);
        assert_eq!(split_into_chunks("abc", 5), vec!["abc"]);
        assert_eq!(split_into_chunks("", 5), Vec::<&str>::new());
    }
}
