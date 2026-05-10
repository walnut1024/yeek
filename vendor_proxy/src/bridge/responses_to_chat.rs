use crate::types::chat::*;
use crate::types::responses::*;

/// Transform a Responses API request into a Chat Completions request.
/// Mirrors LiteLLM's transform_responses_api_request_to_chat_completion_request.
pub fn responses_to_chat(req: &ResponsesRequest) -> ChatCompletionRequest {
    let mut messages: Vec<ChatMessage> = Vec::new();

    // instructions -> system message (if present)
    if let Some(ref instructions) = req.instructions {
        if !instructions.is_empty() {
            messages.push(ChatMessage::System { content: instructions.clone() });
        }
    }

    // input -> Chat messages
    let input_messages = transform_input(&req.input);
    messages.extend(input_messages);

    repair_orphaned_tool_calls(&mut messages);
    remove_orphaned_tool_results(&mut messages);
    dedup_tool_results(&mut messages);
    reorder_tool_messages(&mut messages);
    sanitize_empty_content(&mut messages);

    // tools: filter web_search, convert to chat format
    let chat_tools: Vec<ChatTool> = req
        .tools
        .iter()
        .filter(|t| !is_web_search_tool(&t.tool_type))
        .map(|t| ChatTool {
            tool_type: "function".to_string(),
            function: ChatToolFunction {
                name: t.name.clone(),
                description: t.description.clone().unwrap_or_default(),
                parameters: ensure_object_type(t.parameters.clone()),
                strict: t.strict,
            },
        })
        .collect();

    // Extract reasoning_effort from reasoning parameter
    let reasoning_effort = extract_reasoning_effort(&req.reasoning);

    // DeepSeek models in thinking mode require every assistant message to carry a
    // `reasoning_content` field (even empty). Codex TUI may not round-trip the
    // reasoning items/text blocks, so backfill missing reasoning_content with "".
    for msg in &mut messages {
        if let ChatMessage::Assistant { ref mut reasoning_content, .. } = msg {
            if reasoning_content.is_none() {
                *reasoning_content = Some(String::new());
            }
        }
    }

    // Transform text.format to response_format
    let response_format = text_to_response_format(&req.text);

    // Normalize tool_choice
    let tool_choice = normalize_tool_choice(&req.tool_choice);

    ChatCompletionRequest {
        model: req.model.clone(),
        messages,
        tools: chat_tools,
        max_tokens: req.max_output_tokens,
        stream: req.stream,
        temperature: req.temperature,
        top_p: req.top_p,
        tool_choice,
        stream_options: if req.stream { Some(StreamOptions { include_usage: true }) } else { None },
        response_format,
        reasoning_effort,
        parallel_tool_calls: req.parallel_tool_calls,
        metadata: req.metadata.clone(),
        user: req.user.clone(),
        service_tier: req.service_tier.clone(),
    }
}

/// Transform a content block array from a Responses API message into ChatMessageContent.
/// Handles input_text, input_image, input_file, and unknown types.
fn transform_content_blocks(blocks: &[serde_json::Value]) -> ChatMessageContent {
    let mut parts: Vec<ContentPart> = Vec::new();
    let mut has_non_text = false;

    for block in blocks {
        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match block_type {
            "input_text" => {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    parts.push(ContentPart::Text { text: text.to_string() });
                }
            }
            "input_image" => {
                has_non_text = true;
                let url = block
                    .get("image_url")
                    .map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string()))
                    .unwrap_or_default();
                let detail = block.get("detail").and_then(|v| v.as_str()).unwrap_or("auto");
                parts.push(ContentPart::Image {
                    image_url: ImageUrl {
                        url: if detail != "auto" && !url.contains("?detail=") {
                            format!("{}?detail={}", url, detail)
                        } else {
                            url
                        },
                    },
                });
            }
            "input_file" => {
                has_non_text = true;
                let file_id = block.get("file_id").cloned();
                let file_data = block.get("file_data").cloned();
                let mut file_obj = serde_json::Map::new();
                if let Some(id) = file_id {
                    file_obj.insert("file_id".to_string(), id);
                }
                if let Some(data) = file_data {
                    file_obj.insert("file_data".to_string(), data);
                }
                parts.push(ContentPart::File { file: serde_json::Value::Object(file_obj) });
            }
            _ => {
                // Unknown type with text field -> text content part
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    parts.push(ContentPart::Text { text: text.to_string() });
                }
            }
        }
    }

    if has_non_text || parts.len() > 1 {
        ChatMessageContent::Parts(parts)
    } else if parts.len() == 1 {
        // Single text part -> plain string for cleaner serialization
        match &parts[0] {
            ContentPart::Text { text } => ChatMessageContent::String(text.clone()),
            _ => ChatMessageContent::Parts(parts),
        }
    } else {
        ChatMessageContent::String(String::new())
    }
}

/// Transform Responses API input into Chat messages.
/// Uses dynamic string matching on item["type"], matching LiteLLM's
/// _transform_response_input_param_to_chat_completion_message pattern.
fn transform_input(input: &serde_json::Value) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = Vec::new();

    match input {
        serde_json::Value::String(text) => {
            messages.push(ChatMessage::User { content: ChatMessageContent::String(text.clone()) });
        }
        serde_json::Value::Array(items) => {
            let mut pending_tool_calls: Vec<ChatToolCall> = Vec::new();
            let mut pending_reasoning: Option<String> = None;
            let mut seen_call_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            for item in items {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match item_type {
                    "message" => {
                        flush_tool_calls(
                            &mut pending_tool_calls,
                            &mut messages,
                            &mut pending_reasoning,
                        );
                        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("");

                        // Extract reasoning_text from content blocks before transforming
                        let content_blocks = item.get("content").and_then(|v| v.as_array());
                        let (reasoning_text_from_content, non_reasoning_blocks): (
                            Option<String>,
                            Vec<&serde_json::Value>,
                        ) = match content_blocks {
                            Some(blocks) => {
                                let mut reasoning = None;
                                let mut others = Vec::new();
                                for block in blocks {
                                    if block.get("type").and_then(|v| v.as_str())
                                        == Some("reasoning_text")
                                    {
                                        if let Some(text) =
                                            block.get("text").and_then(|v| v.as_str())
                                        {
                                            reasoning = Some(text.to_string());
                                        }
                                    } else {
                                        others.push(block);
                                    }
                                }
                                (reasoning, others)
                            }
                            None => (None, vec![]),
                        };

                        let content = if !non_reasoning_blocks.is_empty() {
                            transform_content_blocks(
                                &non_reasoning_blocks.iter().cloned().cloned().collect::<Vec<_>>(),
                            )
                        } else if content_blocks.is_some() {
                            // All blocks were reasoning_text, content is empty
                            ChatMessageContent::String(String::new())
                        } else {
                            ChatMessageContent::String(String::new())
                        };

                        // Prefer reasoning_text from content blocks over standalone reasoning item
                        let reasoning = if reasoning_text_from_content.is_some() {
                            pending_reasoning = None; // discard standalone reasoning, use embedded
                            reasoning_text_from_content
                        } else {
                            pending_reasoning.take()
                        };

                        match role {
                            "user" | "developer" => {
                                messages.push(ChatMessage::User { content });
                            }
                            "assistant" => {
                                let text = match &content {
                                    ChatMessageContent::String(s) => {
                                        if s.is_empty() {
                                            None
                                        } else {
                                            Some(s.clone())
                                        }
                                    }
                                    ChatMessageContent::Parts(parts) => {
                                        let text: String = parts
                                            .iter()
                                            .filter_map(|p| match p {
                                                ContentPart::Text { text } => Some(text.as_str()),
                                                _ => None,
                                            })
                                            .collect::<Vec<_>>()
                                            .join("");
                                        if text.is_empty() {
                                            None
                                        } else {
                                            Some(text)
                                        }
                                    }
                                };
                                // If the last message is an Assistant with tool_calls (just
                                // flushed above), merge content into it instead of pushing a
                                // new message, so Tool messages follow tool_calls immediately.
                                let merged = match messages.last_mut() {
                                    Some(ChatMessage::Assistant {
                                        content: ref mut existing_content,
                                        tool_calls: ref existing_tc,
                                        ref mut reasoning_content,
                                    }) if existing_tc.is_some() && existing_content.is_none() => {
                                        *existing_content = text.clone();
                                        if reasoning.is_some() {
                                            *reasoning_content = reasoning.clone();
                                        }
                                        true
                                    }
                                    _ => false,
                                };
                                if !merged {
                                    messages.push(ChatMessage::Assistant {
                                        content: text,
                                        tool_calls: None,
                                        reasoning_content: reasoning,
                                    });
                                }
                            }
                            _ => {
                                pending_reasoning = None;
                            }
                        }
                    }
                    "function_call" => {
                        let call_id =
                            item.get("call_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let name =
                            item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let arguments = item
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if !call_id.is_empty() && seen_call_ids.contains(&call_id) {
                            continue;
                        }
                        if !call_id.is_empty() {
                            seen_call_ids.insert(call_id.clone());
                        }

                        pending_tool_calls.push(ChatToolCall {
                            id: call_id,
                            call_type: "function".to_string(),
                            function: ChatFunctionCall { name, arguments },
                        });
                    }
                    "function_call_output" => {
                        flush_tool_calls(
                            &mut pending_tool_calls,
                            &mut messages,
                            &mut pending_reasoning,
                        );
                        let call_id =
                            item.get("call_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if call_id.is_empty() {
                            continue;
                        }
                        let content = normalize_tool_output(&item["output"]);
                        messages.push(ChatMessage::Tool { content, tool_call_id: call_id });
                    }
                    "custom_tool_call" => {
                        let call_id =
                            item.get("call_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let name =
                            item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let input =
                            item.get("input").and_then(|v| v.as_str()).unwrap_or("").to_string();

                        if !call_id.is_empty() && seen_call_ids.contains(&call_id) {
                            continue;
                        }
                        if !call_id.is_empty() {
                            seen_call_ids.insert(call_id.clone());
                        }

                        pending_tool_calls.push(ChatToolCall {
                            id: call_id,
                            call_type: "function".to_string(),
                            function: ChatFunctionCall { name, arguments: input },
                        });
                    }
                    "custom_tool_call_output" => {
                        flush_tool_calls(
                            &mut pending_tool_calls,
                            &mut messages,
                            &mut pending_reasoning,
                        );
                        let call_id =
                            item.get("call_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if call_id.is_empty() {
                            continue;
                        }
                        let content = normalize_tool_output(&item["output"]);
                        messages.push(ChatMessage::Tool { content, tool_call_id: call_id });
                    }
                    "local_shell_call" => {
                        flush_tool_calls(
                            &mut pending_tool_calls,
                            &mut messages,
                            &mut pending_reasoning,
                        );
                        let status =
                            item.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let cmd = item
                            .get("action")
                            .and_then(|v| v.get("command"))
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
                            })
                            .unwrap_or_else(|| "?".to_string());
                        messages.push(ChatMessage::User {
                            content: ChatMessageContent::String(format!(
                                "[Shell executed: {} — status: {}]",
                                cmd, status
                            )),
                        });
                    }
                    "reasoning" => {
                        pending_reasoning = extract_reasoning_text(item);
                    }
                    _ => {} // compaction_summary, ghost_snapshot, tool_search_*, web_search_*, etc.
                }
            }

            flush_tool_calls(&mut pending_tool_calls, &mut messages, &mut pending_reasoning);
        }
        _ => {}
    }

    messages
}

/// Ensure every assistant message with tool_calls has corresponding Tool messages.
/// For any tool_call_id without a following Tool message, insert a dummy placeholder.
/// Mirrors LiteLLM's `sanitize_messages_for_tool_calling` / `_add_missing_tool_results`.
fn repair_orphaned_tool_calls(messages: &mut Vec<ChatMessage>) {
    let mut insertions: Vec<(usize, ChatMessage)> = Vec::new();

    let mut i = 0;
    while i < messages.len() {
        let tool_call_ids: Vec<String> = match &messages[i] {
            ChatMessage::Assistant { tool_calls: Some(tcs), .. } => {
                tcs.iter().map(|tc| tc.id.clone()).filter(|id| !id.is_empty()).collect()
            }
            _ => {
                i += 1;
                continue;
            }
        };

        if tool_call_ids.is_empty() {
            i += 1;
            continue;
        }

        // Collect tool_call_ids that have corresponding Tool messages following this assistant
        let mut found_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut j = i + 1;
        while j < messages.len() {
            match &messages[j] {
                ChatMessage::Tool { tool_call_id, .. } => {
                    found_ids.insert(tool_call_id.clone());
                }
                ChatMessage::Assistant { .. } => break,
                _ => {}
            }
            j += 1;
        }

        let missing: Vec<&String> =
            tool_call_ids.iter().filter(|id| !found_ids.contains(*id)).collect();

        if !missing.is_empty() {
            // Insert dummy tool results right after the last tool message (or after the assistant)
            let insert_pos = j;
            for (offset, missing_id) in missing.iter().enumerate() {
                let dummy = ChatMessage::Tool {
                    tool_call_id: (*missing_id).clone(),
                    content: ChatMessageContent::String(
                        "[System: Tool execution skipped/interrupted. No result provided.]"
                            .to_string(),
                    ),
                };
                insertions.push((insert_pos + offset, dummy));
            }
            i = j;
        } else {
            i += 1;
        }
    }

    // Insert in reverse order to preserve positions
    for (pos, msg) in insertions.into_iter().rev() {
        messages.insert(pos, msg);
    }
}

/// Remove Tool messages whose tool_call_id has no matching tool_call in any
/// preceding assistant message. Anthropic rejects orphaned tool_results.
/// Mirrors LiteLLM's `_is_orphaned_tool_result` (factory.py).
fn remove_orphaned_tool_results(messages: &mut Vec<ChatMessage>) {
    // Build set of all tool_call_ids across all assistant messages
    let mut all_tool_call_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for msg in messages.iter() {
        if let ChatMessage::Assistant { tool_calls: Some(tcs), .. } = msg {
            for tc in tcs {
                if !tc.id.is_empty() {
                    all_tool_call_ids.insert(tc.id.clone());
                }
            }
        }
    }

    messages.retain(|msg| match msg {
        ChatMessage::Tool { tool_call_id, .. } => all_tool_call_ids.contains(tool_call_id),
        _ => true,
    });
}

/// Deduplicate Tool messages: within each contiguous block after an assistant
/// message, keep only the last occurrence per tool_call_id.
/// Anthropic rejects "each tool_use must have a single result".
/// Mirrors LiteLLM's Case D in `sanitize_messages_for_tool_calling`.
fn dedup_tool_results(messages: &mut Vec<ChatMessage>) {
    let mut remove_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // Track tool_call_id -> first-seen index, reset at each assistant boundary
    let mut seen_in_block: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (idx, msg) in messages.iter().enumerate() {
        match msg {
            ChatMessage::Tool { tool_call_id, .. } if !tool_call_id.is_empty() => {
                if let Some(&prev_idx) = seen_in_block.get(tool_call_id) {
                    // Mark the earlier occurrence for removal (keep latest)
                    remove_indices.insert(prev_idx);
                }
                seen_in_block.insert(tool_call_id.clone(), idx);
            }
            ChatMessage::Assistant { .. } | ChatMessage::System { .. } => {
                seen_in_block.clear();
            }
            _ => {}
        }
    }

    if !remove_indices.is_empty() {
        let mut i = 0;
        messages.retain(|_| {
            let keep = !remove_indices.contains(&i);
            i += 1;
            keep
        });
    }
}

/// Ensure Tool messages immediately follow their preceding Assistant message.
/// Some providers (DeepSeek) reject [Assistant(tool_calls), User, Tool] —
/// the Tool must come right after the Assistant with no intervening messages.
fn reorder_tool_messages(messages: &mut Vec<ChatMessage>) {
    let mut i = 0;
    while i < messages.len() {
        // Find an Assistant with tool_calls
        let has_tool_calls = match &messages[i] {
            ChatMessage::Assistant { tool_calls: Some(tcs), .. } => !tcs.is_empty(),
            _ => false,
        };
        if !has_tool_calls {
            i += 1;
            continue;
        }

        // Collect all Tool messages and non-tool messages between this Assistant and the next
        let mut tools: Vec<ChatMessage> = Vec::new();
        let mut others: Vec<ChatMessage> = Vec::new();
        let mut j = i + 1;
        let mut needs_reorder = false;

        while j < messages.len() {
            match &messages[j] {
                ChatMessage::Tool { .. } => {
                    if !others.is_empty() {
                        needs_reorder = true;
                    }
                    tools.push(messages[j].clone());
                }
                ChatMessage::Assistant { .. } => break,
                _ => {
                    others.push(messages[j].clone());
                }
            }
            j += 1;
        }

        if needs_reorder {
            // Replace messages[i+1..j] with: tools first, then others
            let range_len = j - (i + 1);
            for _ in 0..range_len {
                messages.remove(i + 1);
            }
            let mut insert_pos = i + 1;
            for msg in tools.into_iter().chain(others.into_iter()) {
                messages.insert(insert_pos, msg);
                insert_pos += 1;
            }
        }

        i += 1;
    }
}

/// Replace empty content strings with a single space.
/// Anthropic and some providers reject `content: ""`.
/// Mirrors LiteLLM's `_sanitize_empty_text_content`.
fn sanitize_empty_content(messages: &mut Vec<ChatMessage>) {
    for msg in messages.iter_mut() {
        match msg {
            ChatMessage::User { content } => {
                if let ChatMessageContent::String(s) = content {
                    if s.is_empty() {
                        *s = " ".to_string();
                    }
                }
            }
            ChatMessage::Assistant { content, tool_calls: None, .. } => match content {
                None => *content = Some(" ".to_string()),
                Some(s) if s.is_empty() => *content = Some(" ".to_string()),
                _ => {}
            },
            _ => {}
        }
    }
}

/// Flush pending tool calls into a single assistant message.
/// Mirrors LiteLLM's consecutive function_call merging pattern.
fn flush_tool_calls(
    pending: &mut Vec<ChatToolCall>,
    messages: &mut Vec<ChatMessage>,
    pending_reasoning: &mut Option<String>,
) {
    if pending.is_empty() {
        return;
    }
    // If the last message is already an assistant with tool_calls, merge into it
    if let Some(ChatMessage::Assistant { content, tool_calls, .. }) = messages.last_mut() {
        if tool_calls.is_some() || content.is_none() {
            let merged = std::mem::take(pending);
            match tool_calls {
                Some(existing) => existing.extend(merged),
                None => *tool_calls = Some(merged),
            }
            return;
        }
    }
    let merged = std::mem::take(pending);
    let reasoning = pending_reasoning.take();
    messages.push(ChatMessage::Assistant {
        content: None,
        tool_calls: Some(merged),
        reasoning_content: reasoning,
    });
}

/// Extract reasoning text from a Responses API reasoning item.
/// Tries `content` blocks first, falls back to `summary` blocks.
fn extract_reasoning_text(item: &serde_json::Value) -> Option<String> {
    // Try content blocks
    if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
        let text: String = content
            .iter()
            .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            return Some(text);
        }
    }
    // Fall back to summary blocks
    if let Some(summary) = item.get("summary").and_then(|v| v.as_array()) {
        let text: String = summary
            .iter()
            .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// Normalize tool call output content.
/// Mirrors LiteLLM's _normalize_function_call_output_to_tool_content.
/// Returns multimodal content when images are present, plain string otherwise.
fn normalize_tool_output(output: &serde_json::Value) -> ChatMessageContent {
    match output {
        serde_json::Value::String(s) => ChatMessageContent::String(s.clone()),
        serde_json::Value::Array(parts) => {
            let mut text_parts: Vec<String> = Vec::new();
            let mut content_parts: Vec<ContentPart> = Vec::new();
            let mut has_images = false;

            for part in parts {
                if let Some(obj) = part.as_object() {
                    let part_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match part_type {
                        "input_text" | "output_text" | "text" => {
                            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                                text_parts.push(text.to_string());
                                content_parts.push(ContentPart::Text { text: text.to_string() });
                            }
                        }
                        "input_image" | "image_url" => {
                            has_images = true;
                            let url = obj
                                .get("image_url")
                                .or_else(|| obj.get("url"))
                                .map(|v| {
                                    v.as_str()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| v.to_string())
                                })
                                .unwrap_or_default();
                            content_parts.push(ContentPart::Image { image_url: ImageUrl { url } });
                        }
                        _ => {}
                    }
                }
            }

            if has_images {
                ChatMessageContent::Parts(content_parts)
            } else if !text_parts.is_empty() {
                ChatMessageContent::String(text_parts.join(""))
            } else {
                ChatMessageContent::String(output.to_string())
            }
        }
        _ => ChatMessageContent::String(output.to_string()),
    }
}

/// Normalize tool_choice from various formats to Chat Completion format.
/// Mirrors LiteLLM's _transform_tool_choice.
pub fn normalize_tool_choice(tool_choice: &Option<serde_json::Value>) -> Option<serde_json::Value> {
    match tool_choice {
        None => None,
        Some(serde_json::Value::String(s)) => Some(serde_json::Value::String(s.clone())),
        Some(serde_json::Value::Object(obj)) => {
            let tc_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

            // If it has a function with name, pass through as-is
            if let Some(func) = obj.get("function") {
                if let Some(func_obj) = func.as_object() {
                    if func_obj.get("name").and_then(|v| v.as_str()).is_some() {
                        return Some(serde_json::Value::Object(obj.clone()));
                    }
                }
            }

            // Handle dict formats without function name
            match tc_type {
                "auto" => Some(serde_json::Value::String("auto".to_string())),
                "none" => Some(serde_json::Value::String("none".to_string())),
                "required" | "tool" | "any" => {
                    Some(serde_json::Value::String("required".to_string()))
                }
                "function" => {
                    // function type without name - fall back to required
                    Some(serde_json::Value::String("required".to_string()))
                }
                _ => Some(serde_json::Value::Object(obj.clone())),
            }
        }
        Some(v) => Some(v.clone()),
    }
}

/// Extract reasoning_effort from the reasoning parameter.
/// Always returns a string value. Returns None if effort is "none" or absent.
fn extract_reasoning_effort(reasoning: &Option<serde_json::Value>) -> Option<serde_json::Value> {
    let effort = match reasoning {
        None => return None,
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Object(obj)) => match obj.get("effort").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return None,
        },
        Some(v) => return Some(v.clone()),
    };
    if effort == "none" {
        return None;
    }
    Some(serde_json::Value::String(effort))
}

/// Transform Responses API text.format parameter to Chat Completion response_format.
/// Mirrors LiteLLM's _transform_text_format_to_response_format.
pub fn text_to_response_format(text: &Option<serde_json::Value>) -> Option<serde_json::Value> {
    match text {
        None => None,
        Some(serde_json::Value::Object(obj)) => {
            let format_param = obj.get("format")?;
            let format_obj = format_param.as_object()?;
            let format_type = format_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match format_type {
                "json_schema" => Some(serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": format_obj.get("name").and_then(|v| v.as_str()).unwrap_or("response_schema"),
                        "schema": format_obj.get("schema").cloned().unwrap_or(serde_json::json!({})),
                        "strict": format_obj.get("strict").and_then(|v| v.as_bool()).unwrap_or(false),
                    }
                })),
                "json_object" => Some(serde_json::json!({"type": "json_object"})),
                "text" => None,
                _ => None,
            }
        }
        _ => None,
    }
}

/// Check if tool type is a web search tool (should be filtered from chat tools).
fn is_web_search_tool(tool_type: &str) -> bool {
    matches!(tool_type, "web_search" | "web_search_preview")
}

/// Anthropic requires input_schema.type == "object"
pub fn ensure_object_type(params: Option<serde_json::Value>) -> serde_json::Value {
    match params {
        Some(mut v) => {
            if let Some(obj) = v.as_object_mut() {
                if !obj.contains_key("type") {
                    obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                }
            }
            v
        }
        None => serde_json::json!({"type": "object", "properties": {}}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req() -> ResponsesRequest {
        ResponsesRequest {
            model: "test".to_string(),
            instructions: None,
            input: serde_json::Value::Array(vec![]),
            tools: vec![],
            max_output_tokens: None,
            stream: false,
            temperature: None,
            top_p: None,
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
        }
    }

    #[test]
    fn test_instructions_becomes_system_message() {
        let req = ResponsesRequest {
            instructions: Some("You are helpful".to_string()),
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [
                    {"type": "input_text", "text": "Hi"}
                ]}
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 2);
        assert!(matches!(chat.messages[0], ChatMessage::System { .. }));
        assert!(matches!(chat.messages[1], ChatMessage::User { .. }));
    }

    #[test]
    fn test_tool_call_input_becomes_assistant_with_tool_calls() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{\"city\":\"Paris\"}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "Sunny"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 2);
        match &chat.messages[0] {
            ChatMessage::Assistant { content, tool_calls, .. } => {
                assert!(content.is_none());
                assert_eq!(tool_calls.as_ref().unwrap().len(), 1);
                assert_eq!(tool_calls.as_ref().unwrap()[0].function.name, "get_weather");
            }
            _ => panic!("Expected assistant message"),
        }
        match &chat.messages[1] {
            ChatMessage::Tool { content, tool_call_id } => {
                assert_eq!(content, &ChatMessageContent::String("Sunny".to_string()));
                assert_eq!(tool_call_id, "call_1");
            }
            _ => panic!("Expected tool message"),
        }
    }

    #[test]
    fn test_consecutive_function_calls_merged() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
                {"type": "function_call", "call_id": "call_2", "name": "tool_b", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "result_a"},
                {"type": "function_call_output", "call_id": "call_2", "output": "result_b"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 3);
        match &chat.messages[0] {
            ChatMessage::Assistant { tool_calls, .. } => {
                assert_eq!(tool_calls.as_ref().unwrap().len(), 2);
            }
            _ => panic!("Expected assistant message with merged tool calls"),
        }
    }

    #[test]
    fn test_tool_choice_string_passthrough() {
        assert_eq!(
            normalize_tool_choice(&Some(serde_json::json!("auto"))),
            Some(serde_json::json!("auto"))
        );
    }

    #[test]
    fn test_tool_choice_dict_auto() {
        assert_eq!(
            normalize_tool_choice(&Some(serde_json::json!({"type": "auto"}))),
            Some(serde_json::json!("auto"))
        );
    }

    #[test]
    fn test_tool_choice_dict_tool() {
        assert_eq!(
            normalize_tool_choice(&Some(serde_json::json!({"type": "tool"}))),
            Some(serde_json::json!("required"))
        );
    }

    #[test]
    fn test_tool_choice_dict_function_with_name() {
        let input = serde_json::json!({"type": "function", "function": {"name": "my_func"}});
        assert_eq!(normalize_tool_choice(&Some(input.clone())), Some(input));
    }

    #[test]
    fn test_text_to_response_format_json_schema() {
        let text = Some(serde_json::json!({
            "format": {
                "type": "json_schema",
                "name": "my_schema",
                "schema": {"type": "object", "properties": {}},
                "strict": true
            }
        }));
        let result = text_to_response_format(&text).unwrap();
        assert_eq!(result["type"], "json_schema");
        assert_eq!(result["json_schema"]["name"], "my_schema");
        assert_eq!(result["json_schema"]["strict"], true);
    }

    #[test]
    fn test_text_to_response_format_text_returns_none() {
        let text = Some(serde_json::json!({"format": {"type": "text"}}));
        assert!(text_to_response_format(&text).is_none());
    }

    #[test]
    fn test_web_search_tools_filtered() {
        let req = ResponsesRequest {
            tools: vec![
                ResponsesTool {
                    tool_type: "web_search_preview".to_string(),
                    name: String::new(),
                    description: None,
                    parameters: None,
                    strict: None,
                },
                ResponsesTool {
                    tool_type: "function".to_string(),
                    name: "my_func".to_string(),
                    description: Some("A function".to_string()),
                    parameters: Some(serde_json::json!({"type": "object", "properties": {}})),
                    strict: None,
                },
            ],
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.tools.len(), 1);
        assert_eq!(chat.tools[0].function.name, "my_func");
    }

    #[test]
    fn test_reasoning_effort_extraction() {
        assert_eq!(
            extract_reasoning_effort(&Some(serde_json::json!("high"))),
            Some(serde_json::json!("high"))
        );
        assert_eq!(extract_reasoning_effort(&Some(serde_json::json!("none"))), None);
        assert_eq!(
            extract_reasoning_effort(&Some(serde_json::json!({"effort": "medium"}))),
            Some(serde_json::json!("medium"))
        );
        assert_eq!(extract_reasoning_effort(&Some(serde_json::json!({"effort": "none"}))), None);
        assert_eq!(
            extract_reasoning_effort(&Some(
                serde_json::json!({"effort": "high", "summary": "detailed"})
            )),
            Some(serde_json::json!("high"))
        );
    }

    #[test]
    fn test_tool_call_dedup() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        match &chat.messages[0] {
            ChatMessage::Assistant { tool_calls, .. } => {
                assert_eq!(tool_calls.as_ref().unwrap().len(), 1);
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_string_input() {
        let req = ResponsesRequest {
            input: serde_json::Value::String("What is 2+2?".to_string()),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 1);
        match &chat.messages[0] {
            ChatMessage::User { content } => match content {
                ChatMessageContent::String(s) => assert_eq!(s, "What is 2+2?"),
                _ => panic!("Expected string content"),
            },
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_custom_tool_call() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "custom_tool_call", "call_id": "mcp_1", "name": "read_file", "input": "{\"path\":\"/tmp/a\"}"},
                {"type": "custom_tool_call_output", "call_id": "mcp_1", "output": "file content here"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 2);
        match &chat.messages[0] {
            ChatMessage::Assistant { tool_calls, .. } => {
                let tc = &tool_calls.as_ref().unwrap()[0];
                assert_eq!(tc.function.name, "read_file");
                assert_eq!(tc.function.arguments, "{\"path\":\"/tmp/a\"}");
            }
            _ => panic!("Expected assistant with tool_calls"),
        }
        match &chat.messages[1] {
            ChatMessage::Tool { content, tool_call_id } => {
                assert_eq!(content, &ChatMessageContent::String("file content here".to_string()));
                assert_eq!(tool_call_id, "mcp_1");
            }
            _ => panic!("Expected tool message"),
        }
    }

    #[test]
    fn test_local_shell_call() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "local_shell_call", "call_id": "sh_1", "status": "completed",
                 "action": {"type": "exec", "command": ["ls", "-la"]}},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 1);
        match &chat.messages[0] {
            ChatMessage::User { content } => match content {
                ChatMessageContent::String(s) => {
                    assert!(s.contains("ls -la"));
                    assert!(s.contains("completed"));
                }
                _ => panic!("Expected string content"),
            },
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_reasoning_becomes_assistant() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "reasoning", "id": "rs_1", "summary": [
                    {"type": "summary_text", "text": "I need to check the weather first."}
                ]},
                {"type": "message", "role": "assistant", "content": [
                    {"type": "output_text", "text": "Let me check the weather."}
                ]},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 1);
        match &chat.messages[0] {
            ChatMessage::Assistant { content, tool_calls, reasoning_content } => {
                assert_eq!(content.as_deref(), Some("Let me check the weather."));
                assert_eq!(
                    reasoning_content.as_deref(),
                    Some("I need to check the weather first.")
                );
                assert!(tool_calls.is_none());
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_unknown_type_ignored() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "ghost_snapshot", "ghost_commit": {"id": "abc", "parent": null, "preexisting_untracked_files": [], "preexisting_untracked_dirs": []}},
                {"type": "compaction_summary", "encrypted_content": "AAAA"},
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hello"}]},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        assert_eq!(chat.messages.len(), 1);
        assert!(matches!(chat.messages[0], ChatMessage::User { .. }));
    }

    #[test]
    fn test_orphaned_tool_calls_get_dummy_results() {
        // function_call without function_call_output → should get a dummy tool message
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hi"}]},
                {"type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{}"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // System not present, so: [User, Assistant{tool_calls}, Tool{dummy}]
        assert_eq!(chat.messages.len(), 3);
        match &chat.messages[2] {
            ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_1");
                match content {
                    ChatMessageContent::String(s) => assert!(s.contains("skipped")),
                    _ => panic!("Expected string content"),
                }
            }
            _ => panic!("Expected dummy tool message"),
        }
    }

    #[test]
    fn test_partial_orphaned_tool_calls() {
        // Two tool_calls, only one has output
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hi"}]},
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
                {"type": "function_call", "call_id": "call_2", "name": "tool_b", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "result_a"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // [User, Assistant{2 tool_calls}, Tool{call_1}, Tool{dummy for call_2}]
        assert_eq!(chat.messages.len(), 4);
        // call_1 result
        match &chat.messages[2] {
            ChatMessage::Tool { tool_call_id, .. } => {
                assert_eq!(tool_call_id, "call_1");
            }
            _ => panic!("Expected tool message for call_1"),
        }
        // call_2 dummy
        match &chat.messages[3] {
            ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_2");
                match content {
                    ChatMessageContent::String(s) => assert!(s.contains("skipped")),
                    _ => panic!("Expected string content"),
                }
            }
            _ => panic!("Expected dummy tool message for call_2"),
        }
    }

    #[test]
    fn test_no_dummy_when_all_tool_results_present() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "ok"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // No dummy inserted — just [Assistant{tool_calls}, Tool{call_1}]
        assert_eq!(chat.messages.len(), 2);
    }

    #[test]
    fn test_empty_content_sanitized_to_space() {
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": ""}]},
                {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": ""}]},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // User empty content -> " "
        match &chat.messages[0] {
            ChatMessage::User { content } => match content {
                ChatMessageContent::String(s) => assert_eq!(s, " "),
                _ => panic!("Expected string content"),
            },
            _ => panic!("Expected user message"),
        }
        // Assistant empty content (no tool_calls) -> " "
        match &chat.messages[1] {
            ChatMessage::Assistant { content, tool_calls, .. } => {
                assert!(tool_calls.is_none());
                assert_eq!(content.as_deref(), Some(" "));
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_empty_content_with_tool_calls_preserved() {
        // Assistant with tool_calls and empty content should NOT be replaced
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hi"}]},
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        match &chat.messages[1] {
            ChatMessage::Assistant { content, tool_calls, .. } => {
                assert!(tool_calls.is_some());
                assert!(content.is_none()); // tool_calls assistant keeps empty content
            }
            _ => panic!("Expected assistant with tool_calls"),
        }
    }

    #[test]
    fn test_orphaned_tool_result_removed() {
        // Tool message with call_id that has no matching assistant tool_call → removed
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hi"}]},
                {"type": "function_call_output", "call_id": "ghost_call", "output": "orphan"},
                {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Done"}]},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // orphaned tool result removed: [User, Assistant("Done")]
        assert_eq!(chat.messages.len(), 2);
        match &chat.messages[0] {
            ChatMessage::User { .. } => {}
            _ => panic!("Expected user message"),
        }
        match &chat.messages[1] {
            ChatMessage::Assistant { content, tool_calls, .. } => {
                assert!(tool_calls.is_none());
                assert_eq!(content.as_deref(), Some("Done"));
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_duplicate_tool_results_deduped() {
        // Two tool results for same call_id → keep only the last
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "first"},
                {"type": "function_call_output", "call_id": "call_1", "output": "second"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // [Assistant{tool_calls}, Tool{call_1="second"}] — first deduplicated
        assert_eq!(chat.messages.len(), 2);
        match &chat.messages[1] {
            ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_1");
                match content {
                    ChatMessageContent::String(s) => assert_eq!(s, "second"),
                    _ => panic!("Expected string content"),
                }
            }
            _ => panic!("Expected tool message"),
        }
    }

    #[test]
    fn test_tool_result_kept_when_matching_tool_call_exists() {
        // Tool result with valid matching tool_call → kept
        let req = ResponsesRequest {
            input: serde_json::json!([
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hi"}]},
                {"type": "function_call", "call_id": "call_1", "name": "tool_a", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "ok"},
            ]),
            ..make_req()
        };

        let chat = responses_to_chat(&req);
        // [User, Assistant{tool_calls}, Tool{call_1}]
        assert_eq!(chat.messages.len(), 3);
        match &chat.messages[2] {
            ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_1");
                match content {
                    ChatMessageContent::String(s) => assert_eq!(s, "ok"),
                    _ => panic!("Expected string content"),
                }
            }
            _ => panic!("Expected tool message"),
        }
    }
}
