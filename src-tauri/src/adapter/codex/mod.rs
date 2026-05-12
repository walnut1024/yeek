use std::io::BufRead;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::app::errors::AppError;
use crate::domain::session::{DeleteMode, SessionRecord, SessionStatus, VisibilityStatus};
use crate::domain::source::SourceDescriptor;
use crate::store::messages::MessageRecord;
use crate::store::sessions;

// ── Discovery ──

pub(crate) fn discover_sources() -> Result<Vec<SourceDescriptor>, AppError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Internal("Cannot find home directory".to_string()))?;

    let mut sources = Vec::new();
    scan_dir(&home.join(".codex").join("sessions"), &mut sources)?;
    scan_dir(&home.join(".codex").join("archived_sessions"), &mut sources)?;
    Ok(sources)
}

fn scan_dir(root: &Path, out: &mut Vec<SourceDescriptor>) -> Result<(), AppError> {
    if !root.exists() {
        return Ok(());
    }
    let entries = walkdir_jsonl(root);
    for path in entries {
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.file_type().is_file() {
            continue;
        }
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !file_name.starts_with("rollout-") || !file_name.ends_with(".jsonl") {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default().to_rfc3339()
            })
            .unwrap_or_default();

        out.push(SourceDescriptor {
            source_type: "codex_transcript".to_string(),
            path: path.to_string_lossy().to_string(),
            agent: "codex".to_string(),
            fingerprint: compute_fingerprint(&path),
            last_modified: modified,
        });
    }
    Ok(())
}

fn walkdir_jsonl(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    result.push(path);
                }
            }
        }
    }
    result
}

pub(crate) fn source_descriptor_from_path(path: &Path) -> Option<SourceDescriptor> {
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return None;
    }
    let path_str = path.to_string_lossy();
    // Must live under ~/.codex/sessions/ or ~/.codex/archived_sessions/
    if !path_str.contains("/.codex/sessions/") && !path_str.contains("/.codex/archived_sessions/") {
        return None;
    }
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !file_name.starts_with("rollout-") {
        return None;
    }

    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default().to_rfc3339())
        .unwrap_or_default();

    Some(SourceDescriptor {
        source_type: "codex_transcript".to_string(),
        path: path_str.to_string(),
        agent: "codex".to_string(),
        fingerprint: compute_fingerprint(path),
        last_modified: modified,
    })
}

pub(crate) fn compute_fingerprint(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) => {
            let len = m.len();
            let modified = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("{}:{}", len, modified)
        },
        Err(_) => "unknown".to_string(),
    }
}

// ── Parsing ──

pub(crate) fn parse_session(
    path: &str,
    project_path: Option<&str>,
) -> Result<(SessionRecord, Vec<MessageRecord>), AppError> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::ParseError(format!("Failed to open {}: {}", path, e)))?;
    let reader = std::io::BufReader::new(file);

    let file_name =
        Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

    let mut session_id: Option<String> = None;
    let mut title: Option<String> = None;
    let mut model: Option<String> = None;
    let mut git_branch: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut ended_at: Option<String> = None;
    let mut is_complete = false;
    let mut latest_cwd: Option<String> = None;
    let mut messages: Vec<MessageRecord> = Vec::new();
    let mut line_no: usize = 0;

    // Track assistant message timestamps+content to deduplicate event_msg agent_message
    // vs response_item message pairs that carry identical content.
    let mut seen_assistant_texts: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line_result in reader.lines() {
        let line = line_result
            .map_err(|e| AppError::ParseError(format!("Failed to read {}: {}", path, e)))?;
        if line.trim().is_empty() {
            continue;
        }
        line_no += 1;

        let entry: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let line_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let payload = entry.get("payload").cloned().unwrap_or(Value::Null);
        let timestamp = entry.get("timestamp").and_then(|t| t.as_str()).map(|s| s.to_string());

        if started_at.is_none() && timestamp.is_some() {
            started_at = timestamp.clone();
        }

        match line_type {
            "session_meta" => {
                if session_id.is_none() {
                    session_id = payload.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                if git_branch.is_none() {
                    git_branch = payload
                        .get("git")
                        .and_then(|g| g.get("branch"))
                        .and_then(|b| b.as_str())
                        .map(|s| s.to_string());
                }
                if model.is_none() {
                    model = payload.get("model").and_then(|m| m.as_str()).map(|s| s.to_string());
                }
                let cwd = payload.get("cwd").and_then(|c| c.as_str()).map(|s| s.to_string());
                if cwd.is_some() {
                    latest_cwd = cwd;
                }
            },
            "turn_context" => {
                // Update model from turn_context (preferred over session_meta)
                if let Some(m) = payload.get("model").and_then(|m| m.as_str()) {
                    model = Some(m.to_string());
                }
                let cwd = payload.get("cwd").and_then(|c| c.as_str()).map(|s| s.to_string());
                if cwd.is_some() {
                    latest_cwd = cwd;
                }
            },
            "event_msg" => {
                let event_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match event_type {
                    "user_message" => {
                        let msg = payload.get("message").and_then(|m| m.as_str()).unwrap_or("");
                        if title.is_none() && !msg.is_empty() {
                            title = Some(truncate_to_chars(msg, 120).to_string());
                        }
                        let sid = session_id.as_deref().unwrap_or(&file_name);
                        messages.push(MessageRecord {
                            id: format!("{}:codex:{}:message", sid, line_no),
                            session_id: sid.to_string(),
                            parent_id: None,
                            role: "human".to_string(),
                            kind: "message".to_string(),
                            content_preview: truncate_to_chars(msg, 2000).to_string(),
                            timestamp: timestamp.clone(),
                            is_sidechain: false,
                            entry_type: "user".to_string(),
                            subtype: None,
                            tool_name: None,
                            subagent_id: None,
                            model: None,
                            metadata: None,
                        });
                    },
                    "agent_message" => {
                        let msg = payload.get("message").and_then(|m| m.as_str()).unwrap_or("");
                        let dedup_key = format!("{}:{}", timestamp.as_deref().unwrap_or(""), msg);
                        if !msg.is_empty() && !seen_assistant_texts.contains(&dedup_key) {
                            seen_assistant_texts.insert(dedup_key);
                            let sid = session_id.as_deref().unwrap_or(&file_name);
                            let mut meta_obj = serde_json::Map::new();
                            if let Some(phase) = payload.get("phase").and_then(|p| p.as_str()) {
                                meta_obj.insert("phase".to_string(), Value::String(phase.to_string()));
                            }
                            messages.push(MessageRecord {
                                id: format!("{}:codex:{}:message", sid, line_no),
                                session_id: sid.to_string(),
                                parent_id: None,
                                role: "assistant".to_string(),
                                kind: "message".to_string(),
                                content_preview: truncate_to_chars(msg, 2000).to_string(),
                                timestamp: timestamp.clone(),
                                is_sidechain: false,
                                entry_type: "assistant".to_string(),
                                subtype: None,
                                tool_name: None,
                                subagent_id: None,
                                model: model.clone(),
                                metadata: if meta_obj.is_empty() {
                                    None
                                } else {
                                    Some(Value::Object(meta_obj).to_string())
                                },
                            });
                        }
                    },
                    "task_complete" => {
                        is_complete = true;
                        if let Some(ts) = &timestamp {
                            ended_at = Some(ts.clone());
                        }
                    },
                    "exec_command_end" => {
                        let stdout = payload.get("stdout").and_then(|s| s.as_str()).unwrap_or("");
                        let stderr = payload.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
                        let mut content = String::new();
                        if !stdout.is_empty() {
                            content.push_str(stdout);
                        }
                        if !stderr.is_empty() {
                            if !content.is_empty() {
                                content.push('\n');
                            }
                            content.push_str(stderr);
                        }
                        if content.is_empty() {
                            continue;
                        }
                        let sid = session_id.as_deref().unwrap_or(&file_name);
                        messages.push(MessageRecord {
                            id: format!("{}:codex:{}:tool_result", sid, line_no),
                            session_id: sid.to_string(),
                            parent_id: None,
                            role: "assistant".to_string(),
                            kind: "tool_result".to_string(),
                            content_preview: truncate_to_chars(&content, 2000).to_string(),
                            timestamp: timestamp.clone(),
                            is_sidechain: false,
                            entry_type: "tool_result".to_string(),
                            subtype: None,
                            tool_name: Some("shell".to_string()),
                            subagent_id: None,
                            model: None,
                            metadata: None,
                        });
                    },
                    _ => {},
                }
            },
            "response_item" => {
                let item_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let sid = session_id.as_deref().unwrap_or(&file_name);

                match item_type {
                    "message" => {
                        let role = payload.get("role").and_then(|r| r.as_str()).unwrap_or("");
                        if role != "assistant" {
                            continue;
                        }
                        let content_arr = payload.get("content").and_then(|c| c.as_array());
                        let text = match content_arr {
                            Some(arr) => arr
                                .iter()
                                .filter_map(|block| {
                                    if block.get("type").and_then(|t| t.as_str()) == Some("output_text")
                                    {
                                        block.get("text").and_then(|t| t.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(""),
                            None => String::new(),
                        };
                        if text.is_empty() {
                            continue;
                        }
                        // Deduplicate: skip if an agent_message at the same timestamp already emitted this text
                        let dedup_key = format!("{}:{}", timestamp.as_deref().unwrap_or(""), &text);
                        if seen_assistant_texts.contains(&dedup_key) {
                            continue;
                        }
                        seen_assistant_texts.insert(dedup_key);
                        messages.push(MessageRecord {
                            id: format!("{}:codex:{}:message", sid, line_no),
                            session_id: sid.to_string(),
                            parent_id: None,
                            role: "assistant".to_string(),
                            kind: "message".to_string(),
                            content_preview: truncate_to_chars(&text, 2000).to_string(),
                            timestamp: timestamp.clone(),
                            is_sidechain: false,
                            entry_type: "assistant".to_string(),
                            subtype: None,
                            tool_name: None,
                            subagent_id: None,
                            model: model.clone(),
                            metadata: None,
                        });
                    },
                    "function_call" => {
                        let tool_name = payload.get("name").and_then(|n| n.as_str()).unwrap_or("Tool");
                        let arguments = payload.get("arguments").and_then(|a| a.as_str()).unwrap_or("");
                        let mut meta_obj = serde_json::Map::new();
                        if let Some(call_id) = payload.get("call_id").and_then(|c| c.as_str()) {
                            meta_obj.insert("call_id".to_string(), Value::String(call_id.to_string()));
                        }
                        if let Some(ns) = payload.get("namespace").and_then(|n| n.as_str()) {
                            meta_obj.insert("namespace".to_string(), Value::String(ns.to_string()));
                        }
                        messages.push(MessageRecord {
                            id: format!("{}:codex:{}:tool_use", sid, line_no),
                            session_id: sid.to_string(),
                            parent_id: None,
                            role: "assistant".to_string(),
                            kind: "tool_use".to_string(),
                            content_preview: if arguments.is_empty() {
                                format!("Tool: {}", tool_name)
                            } else {
                                format!("Tool: {}\n{}", tool_name, truncate_to_chars(arguments, 2000))
                            },
                            timestamp: timestamp.clone(),
                            is_sidechain: false,
                            entry_type: "assistant".to_string(),
                            subtype: None,
                            tool_name: Some(tool_name.to_string()),
                            subagent_id: None,
                            model: None,
                            metadata: if meta_obj.is_empty() {
                                None
                            } else {
                                Some(Value::Object(meta_obj).to_string())
                            },
                        });
                    },
                    "function_call_output" => {
                        let output = payload.get("output").and_then(|o| o.as_str()).unwrap_or("");
                        let mut meta_obj = serde_json::Map::new();
                        if let Some(call_id) = payload.get("call_id").and_then(|c| c.as_str()) {
                            meta_obj.insert("call_id".to_string(), Value::String(call_id.to_string()));
                        }
                        messages.push(MessageRecord {
                            id: format!("{}:codex:{}:tool_result", sid, line_no),
                            session_id: sid.to_string(),
                            parent_id: None,
                            role: "human".to_string(),
                            kind: "tool_result".to_string(),
                            content_preview: truncate_to_chars(output, 2000).to_string(),
                            timestamp: timestamp.clone(),
                            is_sidechain: false,
                            entry_type: "tool_result".to_string(),
                            subtype: None,
                            tool_name: None,
                            subagent_id: None,
                            model: None,
                            metadata: if meta_obj.is_empty() {
                                None
                            } else {
                                Some(Value::Object(meta_obj).to_string())
                            },
                        });
                    },
                    "reasoning" => {
                        let summary = payload.get("summary").and_then(|s| s.as_array());
                        let text = match summary {
                            Some(arr) => arr
                                .iter()
                                .filter_map(|block| {
                                    block
                                        .get("text")
                                        .and_then(|t| t.as_str())
                                        .or_else(|| block.get("summary_text").and_then(|t| t.as_str()))
                                })
                                .collect::<Vec<_>>()
                                .join(""),
                            None => String::new(),
                        };
                        if text.is_empty() {
                            continue;
                        }
                        messages.push(MessageRecord {
                            id: format!("{}:codex:{}:reasoning", sid, line_no),
                            session_id: sid.to_string(),
                            parent_id: None,
                            role: "assistant".to_string(),
                            kind: "message".to_string(),
                            content_preview: truncate_to_chars(&text, 2000).to_string(),
                            timestamp: timestamp.clone(),
                            is_sidechain: false,
                            entry_type: "reasoning".to_string(),
                            subtype: None,
                            tool_name: None,
                            subagent_id: None,
                            model: None,
                            metadata: None,
                        });
                    },
                    _ => {},
                }
            },
            "compacted" | "token_count" | _ => {},
        }
    }

    // Resolve session_id: prefer session_meta, fallback to UUID from filename
    let resolved_session_id = session_id.clone().unwrap_or_else(|| {
        extract_uuid_from_filename(&file_name).unwrap_or_else(|| file_name.clone())
    });

    // Update all message session_ids
    for msg in &mut messages {
        msg.session_id = resolved_session_id.clone();
        msg.id = msg.id.replace(
            session_id.as_deref().unwrap_or(&file_name),
            &resolved_session_id,
        );
    }

    // Resolve project_path
    let resolved_project_path = project_path
        .map(|s| s.to_string())
        .or_else(|| latest_cwd.take());

    let file_modified = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let status = if is_complete {
        SessionStatus::Complete
    } else {
        SessionStatus::Active
    };

    Ok((
        SessionRecord {
            id: resolved_session_id,
            agent: "codex".to_string(),
            project_path: resolved_project_path,
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

/// Extract UUID from rollout filename like `rollout-2026-05-12T19-22-05-019e1bec-3436-7062-bca5-cc4d3502b6bd`
fn extract_uuid_from_filename(stem: &str) -> Option<String> {
    // Pattern: rollout-YYYY-MM-DDThh-mm-ss-{uuid}
    let after_date_time = stem.strip_prefix("rollout-")?;
    // Find the UUID part after the datetime prefix (format: YYYY-MM-DDThh-mm-ss-)
    let parts: Vec<&str> = after_date_time.splitn(2, '-').collect();
    if parts.len() < 2 {
        return None;
    }
    // The datetime part is like "2026-05-12T19-22-05", take what's after it
    let after_t = after_date_time.split('T').nth(1)?;
    // after_t is "19-22-05-019e1bec-3436-7062-bca5-cc4d3502b6bd"
    // Skip 3 time segments (hh-mm-ss)
    let segments: Vec<&str> = after_t.split('-').collect();
    if segments.len() > 3 {
        Some(segments[3..].join("-"))
    } else {
        None
    }
}

fn truncate_to_chars(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ── Indexing ──

pub struct IndexResult {
    pub indexed: i64,
    pub updated: i64,
    pub errors: i64,
}

pub(crate) fn index_sources<F>(
    conn: &rusqlite::Connection,
    sources: &[SourceDescriptor],
    on_progress: F,
) -> Result<IndexResult, AppError>
where
    F: Fn(i64),
{
    let mut indexed = 0i64;
    let mut updated = 0i64;
    let mut errors = 0i64;

    let existing_fingerprints: std::collections::HashMap<String, String> = {
        let mut stmt =
            conn.prepare("SELECT path, fingerprint FROM sources WHERE status = 'active'")?;
        let rows =
            stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    conn.execute_batch("BEGIN")?;

    for (i, source) in sources.iter().enumerate() {
        if let Some(stored_fp) = existing_fingerprints.get(&source.path) {
            if *stored_fp == source.fingerprint {
                on_progress((i + 1) as i64);
                continue;
            }
        }

        let sp = format!("sp_codex_{}", i);
        conn.execute_batch(&format!("SAVEPOINT {}", sp))?;

        match index_single_source(conn, source, &existing_fingerprints) {
            Ok(is_update) => {
                conn.execute_batch(&format!("RELEASE {}", sp))?;
                if is_update {
                    updated += 1;
                } else {
                    indexed += 1;
                }
            },
            Err(e) => {
                tracing::error!("Failed to index codex source {}: {}", source.path, e);
                conn.execute_batch(&format!("ROLLBACK TO {}", sp))?;
                errors += 1;
            },
        }

        on_progress((i + 1) as i64);
    }

    crate::store::actions::record_action(
        conn,
        None,
        "codex_sync_completed",
        Some(&format!("indexed={}, updated={}, errors={}", indexed, updated, errors)),
    )?;

    conn.execute_batch("COMMIT")?;

    if indexed + updated > 0 {
        if let Err(e) =
            conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES ('rebuild');")
        {
            tracing::warn!("Codex FTS rebuild failed: {}", e);
        }
    }

    Ok(IndexResult { indexed, updated, errors })
}

fn index_single_source(
    conn: &rusqlite::Connection,
    source: &SourceDescriptor,
    existing_fingerprints: &std::collections::HashMap<String, String>,
) -> Result<bool, AppError> {
    let is_update = existing_fingerprints.contains_key(&source.path);

    let (record, messages) = parse_session(&source.path, None)?;

    sessions::upsert_session(conn, &record)?;
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

    Ok(is_update)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_codex_rollout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout-2026-05-12T19-22-05-019e1bec-3436-7062-bca5-cc4d3502b6bd.jsonl");
        std::fs::write(
            &path,
            r#"{"timestamp":"2026-05-12T11:22:05.000Z","type":"session_meta","payload":{"id":"019e1bec-3436-7062-bca5-cc4d3502b6bd","timestamp":"2026-05-12T11:22:05.000Z","cwd":"/tmp/project","model_provider":"openai","git":{"branch":"main"}}}
{"timestamp":"2026-05-12T11:22:06.000Z","type":"event_msg","payload":{"type":"user_message","message":"请 review","images":[],"local_images":[]}}
{"timestamp":"2026-05-12T11:22:07.000Z","type":"turn_context","payload":{"cwd":"/tmp/project","model":"gpt-5.1-codex-mini"}}
{"timestamp":"2026-05-12T11:22:08.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"我会检查。"}],"phase":"final"}}
{"timestamp":"2026-05-12T11:22:09.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"rg foo\"}","call_id":"call_1"}}
{"timestamp":"2026-05-12T11:22:10.000Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_1","output":"Output:\nfoo\n"}}
{"timestamp":"2026-05-12T11:22:11.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn_1","completed_at":1778584931,"duration_ms":10}}
"#,
        ).unwrap();

        let (session, messages) = parse_session(path.to_str().unwrap(), None).unwrap();
        assert_eq!(session.id, "019e1bec-3436-7062-bca5-cc4d3502b6bd");
        assert_eq!(session.agent, "codex");
        assert_eq!(session.project_path.as_deref(), Some("/tmp/project"));
        assert_eq!(session.title.as_deref(), Some("请 review"));
        assert_eq!(session.model.as_deref(), Some("gpt-5.1-codex-mini"));
        assert_eq!(session.git_branch.as_deref(), Some("main"));
        assert!(matches!(session.status, SessionStatus::Complete));
        assert_eq!(messages.iter().filter(|m| m.kind == "message").count(), 2);
        assert!(messages.iter().any(|m| m.kind == "tool_use" && m.tool_name.as_deref() == Some("exec_command")));
        assert!(messages.iter().any(|m| m.kind == "tool_result"));
    }

    #[test]
    fn skips_unknown_line_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout-2026-05-12T19-22-05-abc.jsonl");
        std::fs::write(
            &path,
            r#"{"timestamp":"2026-05-12T11:22:05.000Z","type":"session_meta","payload":{"id":"test-id","cwd":"/tmp"}}
{"timestamp":"2026-05-12T11:22:06.000Z","type":"unknown_type","payload":{}}
not-json-at-all
{"timestamp":"2026-05-12T11:22:07.000Z","type":"event_msg","payload":{"type":"user_message","message":"hello"}}
"#,
        ).unwrap();

        let (session, messages) = parse_session(path.to_str().unwrap(), None).unwrap();
        assert_eq!(session.id, "test-id");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content_preview, "hello");
    }

    #[test]
    fn extract_uuid_from_filename_works() {
        let stem = "rollout-2026-05-12T19-22-05-019e1bec-3436-7062-bca5-cc4d3502b6bd";
        assert_eq!(
            extract_uuid_from_filename(stem),
            Some("019e1bec-3436-7062-bca5-cc4d3502b6bd".to_string())
        );
    }

    #[test]
    fn extract_uuid_returns_none_for_short_names() {
        assert_eq!(extract_uuid_from_filename("rollout-short"), None);
    }

    #[test]
    fn source_descriptor_rejects_non_codex_paths() {
        let path = Path::new("/Users/test/.claude/projects/abc/session.jsonl");
        assert!(source_descriptor_from_path(path).is_none());
    }

    #[test]
    fn active_status_without_task_complete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout-2026-05-12T19-22-05-abc.jsonl");
        std::fs::write(
            &path,
            r#"{"timestamp":"2026-05-12T11:22:05.000Z","type":"session_meta","payload":{"id":"active-test","cwd":"/tmp"}}
{"timestamp":"2026-05-12T11:22:06.000Z","type":"event_msg","payload":{"type":"user_message","message":"still going"}}
"#,
        ).unwrap();

        let (session, _) = parse_session(path.to_str().unwrap(), None).unwrap();
        assert!(matches!(session.status, SessionStatus::Active));
    }
}
