use std::io::BufRead;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use crate::app::errors::AppError;
use crate::domain::session::{DeleteMode, SessionRecord, SessionStatus, VisibilityStatus};
use crate::domain::source::SourceDescriptor;
use crate::store::messages::MessageRecord;
use crate::store::sessions;
use serde_json::Value;

/// Discover all Claude Code session JSONL files (including subagent transcripts).
pub fn discover_sources() -> Result<Vec<SourceDescriptor>, AppError> {
    let claude_dir = dirs::home_dir()
        .ok_or_else(|| AppError::Internal("Cannot find home directory".to_string()))?
        .join(".claude");

    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();

    let entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| AppError::Internal(format!("Failed to read projects dir: {}", e)))?;

    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let project_dir = entry.path();

        let jsonl_files = match std::fs::read_dir(&project_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for file_entry in jsonl_files.flatten() {
            let path = file_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            // Only top-level session JSONL files (not in subdirectories)
            if path.parent() != Some(&project_dir) {
                continue;
            }

            let metadata = std::fs::metadata(&path)
                .map_err(|e| AppError::Internal(format!("Failed to stat {:?}: {}", path, e)))?;

            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .unwrap_or_default()
                        .to_rfc3339()
                })
                .unwrap_or_default();

            let fingerprint = compute_fingerprint(&path);

            sources.push(SourceDescriptor {
                source_type: "claude_transcript".to_string(),
                path: path.to_string_lossy().to_string(),
                agent: "claude_code".to_string(),
                fingerprint,
                last_modified: modified,
            });

            // Discover subagent transcripts
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let subagent_dir = project_dir.join(file_stem).join("subagents");
            if subagent_dir.exists() {
                if let Ok(sub_entries) = std::fs::read_dir(&subagent_dir) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                            continue;
                        }
                        let sub_meta = match std::fs::metadata(&sub_path) {
                            Ok(m) => m,
                            Err(_) => continue,
                        };
                        let sub_modified = sub_meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| {
                                DateTime::from_timestamp(d.as_secs() as i64, 0)
                                    .unwrap_or_default()
                                    .to_rfc3339()
                            })
                            .unwrap_or_default();

                        sources.push(SourceDescriptor {
                            source_type: "claude_subagent_transcript".to_string(),
                            path: sub_path.to_string_lossy().to_string(),
                            agent: "claude_code".to_string(),
                            fingerprint: compute_fingerprint(&sub_path),
                            last_modified: sub_modified,
                        });
                    }
                }
            }
        }
    }

    Ok(sources)
}

// --- Raw Entry types ---

#[derive(Debug)]
enum RawEntry {
    User {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        session_id: String,
        content: String,
        has_tool_result: bool,
        is_sidechain: bool,
        subagent_id: Option<String>,   // agentId from toolUseResult
    },
    Assistant {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        session_id: String,
        text_parts: Vec<String>,
        tool_names: Vec<String>,
        tool_inputs: Vec<String>,      // JSON string of tool input
        is_sidechain: bool,
        model: Option<String>,
    },
    Attachment {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        session_id: String,
        subtype: String,
        content: String,
        is_sidechain: bool,
    },
    System {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        session_id: String,
        subtype: String,
        content: String,
        is_sidechain: bool,
    },
    Summary {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        session_id: String,
        content: String,
        is_sidechain: bool,
    },
}

/// Parse a Claude Code session JSONL file into a session record and messages.
pub fn parse_session(
    path: &str,
    project_path: Option<&str>,
) -> Result<(SessionRecord, Vec<MessageRecord>), AppError> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::ParseError(format!("Failed to open {}: {}", path, e)))?;
    let reader = std::io::BufReader::new(file);

    let file_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut first_human_message: Option<String> = None;
    let mut custom_title: Option<String> = None;
    let mut model: Option<String> = None;
    let mut git_branch: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut ended_at: Option<String> = None;
    let status = SessionStatus::Complete;

    let mut raw_entries: Vec<RawEntry> = Vec::new();

    for line_result in reader.lines() {
        let line = line_result
            .map_err(|e| AppError::ParseError(format!("Failed to read {}: {}", path, e)))?;
        if line.trim().is_empty() {
            continue;
        }

        let entry: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let timestamp = entry
            .get("timestamp")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());
        let uuid = entry
            .get("uuid")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let session_id = entry
            .get("sessionId")
            .and_then(|s| s.as_str())
            .unwrap_or(&file_name)
            .to_string();
        let parent_uuid = entry
            .get("parentUuid")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let is_sidechain = entry
            .get("isSidechain")
            .and_then(|s| s.as_bool())
            .unwrap_or(false);

        if started_at.is_none() && timestamp.is_some() {
            started_at = timestamp.clone();
        }
        if timestamp.is_some() {
            ended_at = timestamp.clone();
        }

        if git_branch.is_none() {
            git_branch = entry
                .get("gitBranch")
                .and_then(|b| b.as_str())
                .map(|s| s.to_string());
        }

        match msg_type {
            "user" => {
                let is_meta = entry
                    .get("isMeta")
                    .and_then(|m| m.as_bool())
                    .unwrap_or(false);
                if is_meta {
                    continue;
                }

                let (content, has_tool_result, subagent_id) =
                    extract_user_content_v2(&entry);

                if first_human_message.is_none() && !content.is_empty() && !has_tool_result {
                    first_human_message = Some(content.clone());
                }

                raw_entries.push(RawEntry::User {
                    uuid,
                    parent_uuid,
                    timestamp,
                    session_id,
                    content,
                    has_tool_result,
                    is_sidechain,
                    subagent_id,
                });
            }
            "assistant" => {
                if let Some(msg) = entry.get("message") {
                    if model.is_none() {
                        model = msg.get("model").and_then(|m| m.as_str()).map(|s| s.to_string());
                    }

                    let text_parts = extract_assistant_text(msg);
                    let (tool_names, tool_inputs) = extract_tool_info(msg);

                    // Merge with previous assistant entry if consecutive
                    if let Some(RawEntry::Assistant { text_parts: prev_text, tool_names: prev_tools, tool_inputs: prev_inputs, .. }) = raw_entries.last_mut() {
                        prev_text.extend(text_parts);
                        prev_tools.extend(tool_names);
                        prev_inputs.extend(tool_inputs);
                    } else {
                        raw_entries.push(RawEntry::Assistant {
                            uuid,
                            parent_uuid,
                            timestamp,
                            session_id,
                            text_parts,
                            tool_names,
                            tool_inputs,
                            is_sidechain,
                            model: msg.get("model").and_then(|m| m.as_str()).map(|s| s.to_string()),
                        });
                    }
                }
            }
            "attachment" => {
                // Subtype is nested inside the attachment object
                let subtype = entry
                    .get("attachment")
                    .and_then(|a| a.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let content = extract_attachment_content(&entry, &subtype);

                raw_entries.push(RawEntry::Attachment {
                    uuid,
                    parent_uuid,
                    timestamp,
                    session_id,
                    subtype,
                    content,
                    is_sidechain,
                });
            }
            "system" => {
                let subtype = entry
                    .get("subtype")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let content = extract_system_content(&entry, &subtype);

                raw_entries.push(RawEntry::System {
                    uuid,
                    parent_uuid,
                    timestamp,
                    session_id,
                    subtype,
                    content,
                    is_sidechain,
                });
            }
            "summary" => {
                let content = entry
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .map(|s| truncate_preview(s, 2000).to_string())
                    .unwrap_or_default();

                if !content.is_empty() {
                    raw_entries.push(RawEntry::Summary {
                        uuid,
                        parent_uuid,
                        timestamp,
                        session_id,
                        content,
                        is_sidechain,
                    });
                }
            }
            "custom-title" => {
                custom_title = entry
                    .get("title")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());
            }
            // file-history-snapshot, last-prompt, queue-operation, agent-name: skip
            _ => {}
        }
    }

    // Phase 2: convert raw entries to MessageRecords
    let mut messages: Vec<MessageRecord> = Vec::new();

    for raw in &raw_entries {
        match raw {
            RawEntry::User { uuid, parent_uuid, timestamp, session_id, content, has_tool_result, is_sidechain, subagent_id } => {
                if *has_tool_result {
                    // Store tool_result as separate message with kind "tool_result"
                    messages.push(MessageRecord {
                        id: uuid.clone(),
                        session_id: session_id.clone(),
                        parent_id: parent_uuid.clone(),
                        role: "human".to_string(),
                        kind: "tool_result".to_string(),
                        content_preview: truncate_preview(content, 2000).to_string(),
                        timestamp: timestamp.clone(),
                        is_sidechain: *is_sidechain,
                        entry_type: "user".to_string(),
                        subtype: Some("tool_result".to_string()),
                        tool_name: None,
                        subagent_id: subagent_id.clone(),
                        model: None,
                        metadata: None,
                    });
                } else {
                    messages.push(MessageRecord {
                        id: uuid.clone(),
                        session_id: session_id.clone(),
                        parent_id: parent_uuid.clone(),
                        role: "human".to_string(),
                        kind: "message".to_string(),
                        content_preview: truncate_preview(content, 2000).to_string(),
                        timestamp: timestamp.clone(),
                        is_sidechain: *is_sidechain,
                        entry_type: "user".to_string(),
                        subtype: None,
                        tool_name: None,
                        subagent_id: None,
                        model: None,
                        metadata: None,
                    });
                }
            }
            RawEntry::Assistant { uuid, parent_uuid, timestamp, session_id, text_parts, tool_names, tool_inputs, is_sidechain, model: assistant_model } => {
                let (kind, preview) = if !tool_names.is_empty() {
                    let tools_preview = format!("Tool: {}", tool_names.join(", "));
                    if text_parts.is_empty() {
                        ("tool_use".to_string(), tools_preview)
                    } else {
                        let combined = format!("{}\n{}", text_parts.join("\n"), tools_preview);
                        ("tool_use".to_string(), combined)
                    }
                } else if !text_parts.is_empty() {
                    ("message".to_string(), text_parts.join("\n"))
                } else {
                    ("message".to_string(), String::new())
                };

                // Build metadata JSON with tool inputs
                let metadata = if !tool_inputs.is_empty() {
                    Some(serde_json::json!({
                        "tool_inputs": tool_inputs,
                    }).to_string())
                } else {
                    None
                };

                let tool_name_val = if tool_names.len() == 1 {
                    Some(tool_names[0].clone())
                } else if !tool_names.is_empty() {
                    Some(tool_names.join(","))
                } else {
                    None
                };

                messages.push(MessageRecord {
                    id: uuid.clone(),
                    session_id: session_id.clone(),
                    parent_id: parent_uuid.clone(),
                    role: "assistant".to_string(),
                    kind,
                    content_preview: truncate_preview(&preview, 2000).to_string(),
                    timestamp: timestamp.clone(),
                    is_sidechain: *is_sidechain,
                    entry_type: "assistant".to_string(),
                    subtype: None,
                    tool_name: tool_name_val,
                    subagent_id: None,
                    model: assistant_model.clone(),
                    metadata,
                });
            }
            RawEntry::Attachment { uuid, parent_uuid, timestamp, session_id, subtype, content, is_sidechain } => {
                // Skip noisy hook responses by default, keep meaningful ones
                let skip = matches!(
                    subtype.as_str(),
                    "async_hook_response" | "hook_success"
                );
                if skip && content.is_empty() {
                    continue;
                }

                messages.push(MessageRecord {
                    id: uuid.clone(),
                    session_id: session_id.clone(),
                    parent_id: parent_uuid.clone(),
                    role: "system".to_string(),
                    kind: "attachment".to_string(),
                    content_preview: truncate_preview(content, 2000).to_string(),
                    timestamp: timestamp.clone(),
                    is_sidechain: *is_sidechain,
                    entry_type: "attachment".to_string(),
                    subtype: Some(subtype.clone()),
                    tool_name: None,
                    subagent_id: None,
                    model: None,
                    metadata: None,
                });
            }
            RawEntry::System { uuid, parent_uuid, timestamp, session_id, subtype, content, is_sidechain } => {
                messages.push(MessageRecord {
                    id: uuid.clone(),
                    session_id: session_id.clone(),
                    parent_id: parent_uuid.clone(),
                    role: "system".to_string(),
                    kind: "system_event".to_string(),
                    content_preview: truncate_preview(content, 2000).to_string(),
                    timestamp: timestamp.clone(),
                    is_sidechain: *is_sidechain,
                    entry_type: "system".to_string(),
                    subtype: Some(subtype.clone()),
                    tool_name: None,
                    subagent_id: None,
                    model: None,
                    metadata: None,
                });
            }
            RawEntry::Summary { uuid, parent_uuid, timestamp, session_id, content, is_sidechain } => {
                messages.push(MessageRecord {
                    id: uuid.clone(),
                    session_id: session_id.clone(),
                    parent_id: parent_uuid.clone(),
                    role: "system".to_string(),
                    kind: "summary".to_string(),
                    content_preview: truncate_preview(content, 2000).to_string(),
                    timestamp: timestamp.clone(),
                    is_sidechain: *is_sidechain,
                    entry_type: "summary".to_string(),
                    subtype: None,
                    tool_name: None,
                    subagent_id: None,
                    model: None,
                    metadata: None,
                });
            }
        }
    }

    let file_modified = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let session_id = file_name.clone();

    let title = custom_title
        .or_else(|| first_human_message.as_deref().map(|s| truncate_preview(s, 200).to_string()));

    Ok((
        SessionRecord {
            id: session_id,
            agent: "claude_code".to_string(),
            project_path: project_path.map(|s| s.to_string()),
            title,
            model,
            git_branch,
            started_at,
            ended_at,
            status,
            visibility: VisibilityStatus::Visible,
            pinned: false,
            archived_at: None,
            deleted_at: None,
            delete_mode: DeleteMode::None,
            message_count: messages.len() as i64,
            updated_at: file_modified,
            parent_session_id: None,
        },
        messages,
    ))
}

/// Parse a subagent transcript. Returns (SessionRecord, Vec<MessageRecord>).
/// The session_id is "{parent_session_id}:{agentId}" and parent_session_id is set.
pub fn parse_subagent_session(
    path: &str,
    parent_session_id: &str,
    agent_id: &str,
) -> Result<(SessionRecord, Vec<MessageRecord>), AppError> {
    let (mut record, messages) = parse_session(path, None)?;

    // Override session metadata for subagent
    let sub_session_id = format!("{}:{}", parent_session_id, agent_id);
    record.id = sub_session_id.clone();
    record.parent_session_id = Some(parent_session_id.to_string());
    record.agent = "claude_code_subagent".to_string();

    // Update session_id on all messages
    let messages = messages
        .into_iter()
        .map(|mut m| {
            m.session_id = sub_session_id.clone();
            m
        })
        .collect();

    Ok((record, messages))
}

/// Extract display content from a user message.
/// Returns (content_preview, has_tool_result, subagent_id).
fn extract_user_content_v2(entry: &Value) -> (String, bool, Option<String>) {
    let message = match entry.get("message") {
        Some(m) => m,
        None => return (String::new(), false, None),
    };

    let content = match message.get("content") {
        Some(c) => c,
        None => return (String::new(), false, None),
    };

    // Check for subagent metadata in toolUseResult
    let subagent_id = entry
        .get("toolUseResult")
        .and_then(|r| r.get("agentId"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string());

    if let Some(s) = content.as_str() {
        return (s.to_string(), false, subagent_id);
    }

    if let Some(arr) = content.as_array() {
        let mut text_parts = Vec::new();
        let mut tool_result_parts = Vec::new();

        for item in arr {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("tool_result") => {
                    // content can be string or nested array
                    if let Some(text) = item.get("content").and_then(|c| c.as_str()) {
                        tool_result_parts.push(truncate_preview(text, 200).to_string());
                    } else if let Some(nested) = item.get("content").and_then(|c| c.as_array()) {
                        for sub in nested {
                            if sub.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = sub.get("text").and_then(|t| t.as_str()) {
                                    tool_result_parts.push(truncate_preview(text, 200).to_string());
                                }
                            }
                        }
                    }
                }
                Some("text") => {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
                _ => {}
            }
        }

        if !tool_result_parts.is_empty() {
            return (tool_result_parts.join("; "), true, subagent_id);
        }
        return (text_parts.join("\n"), false, subagent_id);
    }

    (String::new(), false, subagent_id)
}

/// Extract text content from an assistant message (excluding thinking).
fn extract_assistant_text(message: &Value) -> Vec<String> {
    let content = match message.get("content") {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut texts = Vec::new();

    if let Some(arr) = content.as_array() {
        for block in arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    texts.push(text.to_string());
                }
            }
        }
    } else if let Some(s) = content.as_str() {
        texts.push(s.to_string());
    }

    texts
}

/// Extract tool names and input JSON from an assistant message.
fn extract_tool_info(message: &Value) -> (Vec<String>, Vec<String>) {
    let content = match message.get("content") {
        Some(c) => c,
        None => return (Vec::new(), Vec::new()),
    };

    let mut names = Vec::new();
    let mut inputs = Vec::new();

    if let Some(arr) = content.as_array() {
        for block in arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                    names.push(name.to_string());
                }
                if let Some(input) = block.get("input") {
                    inputs.push(truncate_preview(&input.to_string(), 500).to_string());
                }
            }
        }
    }

    (names, inputs)
}

/// Extract display content from an attachment entry.
fn extract_attachment_content(entry: &Value, subtype: &str) -> String {
    // Attachment data is nested inside entry["attachment"]
    let attachment = entry.get("attachment").cloned().unwrap_or(Value::Null);
    let att = &attachment;

    match subtype {
        "file" | "edited_text_file" => {
            let path = att
                .get("path")
                .or_else(|| att.get("filePath"))
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let content = att
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            if path.is_empty() {
                truncate_preview(content, 500).to_string()
            } else if content.is_empty() {
                path.to_string()
            } else {
                format!("{}: {}", path, truncate_preview(content, 300))
            }
        }
        "plan_mode" | "plan_mode_exit" | "plan_mode_reentry" => {
            att.get("reason")
                .or_else(|| att.get("content"))
                .and_then(|r| r.as_str())
                .unwrap_or(subtype)
                .to_string()
        }
        "date_change" => {
            att.get("newDate")
                .and_then(|d| d.as_str())
                .unwrap_or(subtype)
                .to_string()
        }
        "task_reminder" => {
            att.get("content")
                .and_then(|c| c.as_str())
                .unwrap_or(subtype)
                .to_string()
        }
        "hook_success" | "async_hook_response" => {
            let hook_name = att
                .get("hookName")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            if hook_name.is_empty() {
                String::new()
            } else {
                format!("Hook: {}", hook_name)
            }
        }
        "hook_additional_context" => {
            // These are just context injections, not useful to display
            String::new()
        }
        _ => {
            att.get("content")
                .and_then(|c| c.as_str())
                .or_else(|| att.get("text").and_then(|t| t.as_str()))
                .map(|s| truncate_preview(s, 500).to_string())
                .unwrap_or_else(|| subtype.to_string())
        }
    }
}

/// Extract display content from a system entry.
fn extract_system_content(entry: &Value, subtype: &str) -> String {
    match subtype {
        "stop_hook_summary" => {
            let hooks = entry
                .get("hookInfos")
                .and_then(|h| h.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            if hooks.is_empty() {
                "Stop hook summary".to_string()
            } else {
                format!("Stop hooks: {}", hooks)
            }
        }
        "turn_duration" => {
            let duration_ms = entry
                .get("durationMs")
                .and_then(|d| d.as_i64())
                .unwrap_or(0);
            let cost = entry.get("costUsd").and_then(|c| c.as_f64());
            let mut s = format!("Turn: {}ms", duration_ms);
            if let Some(c) = cost {
                s.push_str(&format!(" (${:.4})", c));
            }
            s
        }
        "local_command" => {
            entry
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or(subtype)
                .to_string()
        }
        "compact_boundary" => {
            "Conversation compacted".to_string()
        }
        _ => {
            entry
                .get("content")
                .and_then(|c| c.as_str())
                .or_else(|| entry.get("text").and_then(|t| t.as_str()))
                .map(|s| truncate_preview(s, 500).to_string())
                .unwrap_or_else(|| subtype.to_string())
        }
    }
}

/// Decode a Claude projects directory name back to a path.
fn decode_project_dir(name: &str) -> String {
    if name.starts_with('-') {
        format!("/{}", &name[1..].replace('-', "/"))
    } else {
        name.replace('-', "/")
    }
}

/// Compute a simple fingerprint for change detection.
fn compute_fingerprint(path: &Path) -> String {
    let metadata = std::fs::metadata(path);
    match metadata {
        Ok(m) => {
            let len = m.len();
            let modified = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("{}:{}", len, modified)
        }
        Err(_) => "unknown".to_string(),
    }
}

fn truncate_preview(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len.saturating_sub(3);
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Index all discovered Claude Code sources into the store.
pub fn index_all(conn: &rusqlite::Connection) -> Result<IndexResult, AppError> {
    let sources = discover_sources()?;
    let mut indexed = 0i64;
    let mut updated = 0i64;
    let mut errors = 0i64;
    let mut skipped = 0i64;

    // Load existing fingerprints for incremental sync
    let existing_fingerprints: std::collections::HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT path, fingerprint FROM sources WHERE status = 'active'")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // Wrap entire sync in a single transaction for performance
    conn.execute_batch("BEGIN")?;

    let result = (|| -> Result<(), AppError> {
        for source in &sources {
            // Skip unchanged files (incremental sync)
            if let Some(stored_fp) = existing_fingerprints.get(&source.path) {
                if *stored_fp == source.fingerprint {
                    skipped += 1;
                    continue;
                }
            }

            let project_path = extract_project_path_from_source(&source.path);

            if source.source_type == "claude_subagent_transcript" {
                let parent_session_id = extract_parent_session_id(&source.path);
                let agent_id = extract_agent_id(&source.path);

                if let (Some(parent_id), Some(agent_id)) = (&parent_session_id, &agent_id) {
                    match parse_subagent_session(&source.path, parent_id, agent_id) {
                        Ok((record, messages)) => {
                            sessions::upsert_session(conn, &record)?;
                            if existing_fingerprints.contains_key(&source.path) {
                                updated += 1;
                            } else {
                                indexed += 1;
                            }
                            for msg in &messages {
                                crate::store::messages::upsert_message(conn, msg)?;
                            }
                            crate::store::sources::upsert_source(conn, source)?;
                            crate::store::sources::link_session_source(
                                conn,
                                &record.id,
                                &source.fingerprint,
                                &source.source_type,
                                &source.path,
                                "file_safe",
                            )?;
                        }
                        Err(_) => {
                            errors += 1;
                        }
                    }
                }
            } else {
                match parse_session(&source.path, project_path.as_deref()) {
                    Ok((record, messages)) => {
                        sessions::upsert_session(conn, &record)?;
                        if existing_fingerprints.contains_key(&source.path) {
                            updated += 1;
                        } else {
                            indexed += 1;
                        }
                        for msg in &messages {
                            crate::store::messages::upsert_message(conn, msg)?;
                        }
                        crate::store::sources::upsert_source(conn, source)?;
                        crate::store::sources::link_session_source(
                            conn,
                            &record.id,
                            &source.fingerprint,
                            &source.source_type,
                            &source.path,
                            "file_safe",
                        )?;
                    }
                    Err(_) => {
                        errors += 1;
                    }
                }
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(_) => {
            conn.execute_batch("ROLLBACK")?;
        }
    };

    crate::store::actions::record_action(
        conn,
        None,
        "sync_completed",
        Some(&format!(
            "indexed={}, updated={}, skipped={}, errors={}",
            indexed, updated, skipped, errors
        )),
    )?;

    Ok(IndexResult {
        indexed,
        updated,
        errors,
    })
}

fn extract_project_path_from_source(path: &str) -> Option<String> {
    let path_buf = PathBuf::from(path);
    let project_dir_name = path_buf.parent()?.file_name()?.to_str()?;
    Some(decode_project_dir(project_dir_name))
}

/// Extract parent session ID from subagent path.
/// Path format: ~/.claude/projects/{project-dir}/{sessionId}/subagents/agent-{agentId}.jsonl
fn extract_parent_session_id(path: &str) -> Option<String> {
    let path_buf = PathBuf::from(path);
    // Go up: agent-{id}.jsonl -> subagents -> {sessionId} -> {project-dir}
    let subagents_dir = path_buf.parent()?; // subagents/
    let session_dir = subagents_dir.parent()?; // {sessionId}/
    let session_id = session_dir.file_name()?.to_str()?;
    Some(session_id.to_string())
}

/// Extract agent ID from subagent path.
/// Path format: .../subagents/agent-{agentId}.jsonl
fn extract_agent_id(path: &str) -> Option<String> {
    let path_buf = PathBuf::from(path);
    let file_name = path_buf.file_stem()?.to_str()?;
    if file_name.starts_with("agent-") {
        Some(file_name[6..].to_string())
    } else {
        None
    }
}

pub struct IndexResult {
    pub indexed: i64,
    pub updated: i64,
    pub errors: i64,
}
