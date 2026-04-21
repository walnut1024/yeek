use std::collections::HashMap;

use rusqlite::params;
use serde::Serialize;

use crate::app::errors::AppError;

#[derive(Debug, Serialize, Clone)]
pub struct MessageRecord {
    pub id: String,
    pub session_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub kind: String,
    pub content_preview: String,
    pub timestamp: Option<String>,
    pub is_sidechain: bool,
    pub entry_type: String,
    pub subtype: Option<String>,
    pub tool_name: Option<String>,
    pub subagent_id: Option<String>,
    pub model: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MessagePreview {
    pub role: String,
    pub content_preview: String,
}

pub fn upsert_message(
    conn: &rusqlite::Connection,
    msg: &MessageRecord,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO messages (id, session_id, parent_id, role, kind, content_preview, timestamp, is_sidechain, entry_type, subtype, tool_name, subagent_id, model, metadata)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           parent_id=excluded.parent_id,
           content_preview=excluded.content_preview,
           kind=excluded.kind,
           entry_type=excluded.entry_type,
           subtype=excluded.subtype,
           tool_name=excluded.tool_name,
           subagent_id=excluded.subagent_id,
           model=excluded.model,
           metadata=excluded.metadata",
        params![
            msg.id,
            msg.session_id,
            msg.parent_id,
            msg.role,
            msg.kind,
            msg.content_preview,
            msg.timestamp,
            msg.is_sidechain as i64,
            msg.entry_type,
            msg.subtype,
            msg.tool_name,
            msg.subagent_id,
            msg.model,
            msg.metadata,
        ],
    )?;

    Ok(())
}

/// Rebuild FTS index for all messages of a given session (batch operation).
pub fn rebuild_fts_for_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<(), AppError> {
    // Delete existing FTS entries for this session
    conn.execute(
        "DELETE FROM messages_fts WHERE rowid IN (SELECT rowid FROM messages WHERE session_id = ?1)",
        params![session_id],
    )?;
    // Batch insert FTS entries
    conn.execute(
        "INSERT INTO messages_fts (rowid, session_id, role, kind, content_preview)
         SELECT rowid, session_id, role, kind, content_preview FROM messages WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

pub fn get_session_messages(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<MessageRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, parent_id, role, kind, content_preview, timestamp, is_sidechain,
                entry_type, subtype, tool_name, subagent_id, model, metadata
         FROM messages WHERE session_id = ? ORDER BY timestamp ASC",
    )?;

    let messages = stmt
        .query_map(params![session_id], |row| {
            Ok(MessageRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                parent_id: row.get(2)?,
                role: row.get(3)?,
                kind: row.get(4)?,
                content_preview: row.get(5)?,
                timestamp: row.get(6)?,
                is_sidechain: row.get::<_, i64>(7)? != 0,
                entry_type: row.get(8)?,
                subtype: row.get(9)?,
                tool_name: row.get(10)?,
                subagent_id: row.get(11)?,
                model: row.get(12)?,
                metadata: row.get(13)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(messages)
}

pub fn get_preview_messages(
    conn: &rusqlite::Connection,
    session_id: &str,
    limit: i64,
) -> Result<Vec<MessagePreview>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT role, content_preview FROM messages
         WHERE session_id = ? AND role IN ('human', 'assistant')
         ORDER BY timestamp ASC LIMIT ?",
    )?;

    let previews = stmt
        .query_map(params![session_id, limit], |row| {
            Ok(MessagePreview {
                role: row.get(0)?,
                content_preview: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(previews)
}

// --- Transcript tree structures ---

#[derive(Debug, Serialize, Clone)]
pub struct SiblingInfo {
    pub message_id: String,
    pub label: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BranchPoint {
    pub parent_id: String,
    pub siblings: Vec<SiblingInfo>,
    pub active_index: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct TranscriptPayload {
    pub messages: Vec<MessageRecord>,
    pub main_path: Vec<String>,
    pub branches: Vec<BranchPoint>,
}

/// Extract a short label for a message to use in branch tabs.
fn branch_label(msg: &MessageRecord) -> String {
    if msg.kind == "tool_use" {
        let tool = msg.tool_name.as_deref().unwrap_or("Tool");
        // Take first line of preview as context
        let first_line = msg.content_preview.lines().next().unwrap_or("");
        let target = first_line.replace("Tool: ", "");
        if target.is_empty() {
            format!("Tool: {}", tool)
        } else {
            format!("{}: {}", tool, truncate_str(&target, 40))
        }
    } else {
        truncate_str(&msg.content_preview, 50).to_string()
    }
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..s.char_indices().take(max).last().map(|(i, _)| i).unwrap_or(s.len())] }
}

/// Build the conversation tree and extract main path + branches.
///
/// Strategy:
/// - Try tree-based main path extraction (backtrack from latest leaf to root)
/// - If the tree is fragmented (path covers < 50% of non-sidechain messages),
///   fall back to flat chronological list of all non-sidechain messages
/// - Branch points are identified from the tree where possible
pub fn get_session_transcript(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<TranscriptPayload, AppError> {
    let messages = get_session_messages(conn, session_id)?;

    if messages.is_empty() {
        return Ok(TranscriptPayload {
            messages,
            main_path: Vec::new(),
            branches: Vec::new(),
        });
    }

    let non_sidechain_count = messages.iter().filter(|m| !m.is_sidechain).count();
    if non_sidechain_count == 0 {
        return Ok(TranscriptPayload {
            messages,
            main_path: Vec::new(),
            branches: Vec::new(),
        });
    }

    // 1. Build id_map and children_map
    let mut id_map: HashMap<String, usize> = HashMap::new();
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();

    for (i, msg) in messages.iter().enumerate() {
        id_map.insert(msg.id.clone(), i);
        if let Some(ref parent_id) = msg.parent_id {
            children_map
                .entry(parent_id.clone())
                .or_default()
                .push(msg.id.clone());
        }
    }

    // 2. Try tree-based main path extraction
    let tree_path = extract_main_path(&messages, &id_map, &children_map);

    // 3. Decide: tree path or fallback to flat list
    let path = if tree_path.len() * 2 >= non_sidechain_count {
        // Tree path covers ≥ 50% — use it
        tree_path
    } else {
        // Tree is fragmented — fall back to flat chronological list
        messages
            .iter()
            .filter(|m| !m.is_sidechain)
            .map(|m| m.id.clone())
            .collect()
    };

    // 4. Identify branch points along the main path
    let path_set: std::collections::HashSet<&str> = path.iter().map(|s| s.as_str()).collect();
    let mut branches = Vec::new();

    for path_id in &path {
        if let Some(children) = children_map.get(path_id) {
            if children.len() > 1 {
                let mut siblings = Vec::new();
                let mut active_index = 0;

                for (i, child_id) in children.iter().enumerate() {
                    if let Some(&idx) = id_map.get(child_id) {
                        siblings.push(SiblingInfo {
                            message_id: child_id.clone(),
                            label: branch_label(&messages[idx]),
                        });
                    }
                    if path_set.contains(child_id.as_str()) {
                        active_index = i;
                    }
                }

                branches.push(BranchPoint {
                    parent_id: path_id.clone(),
                    siblings,
                    active_index,
                });
            }
        }
    }

    Ok(TranscriptPayload {
        messages,
        main_path: path,
        branches,
    })
}

/// Extract main path by backtracking from the latest leaf to root.
fn extract_main_path(
    messages: &[MessageRecord],
    id_map: &HashMap<String, usize>,
    children_map: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    // Find latest non-sidechain leaf
    let leaf_id = messages
        .iter()
        .filter(|m| !children_map.contains_key(&m.id))
        .filter(|m| !m.is_sidechain)
        .max_by(|a, b| {
            a.timestamp
                .as_deref()
                .unwrap_or("")
                .cmp(&b.timestamp.as_deref().unwrap_or(""))
        })
        .map(|m| m.id.clone());

    let Some(leaf_id) = leaf_id else {
        return Vec::new();
    };

    // Backtrack from leaf to root, stop when parent_id is dangling
    let mut path = Vec::new();
    let mut current = Some(leaf_id);
    while let Some(id) = current {
        path.push(id.clone());
        if let Some(&idx) = id_map.get(&id) {
            let parent = &messages[idx].parent_id;
            match parent {
                Some(pid) if id_map.contains_key(pid) => {
                    current = Some(pid.clone());
                }
                _ => break, // null parent or dangling — stop
            }
        } else {
            break;
        }
    }
    path.reverse();
    path
}
