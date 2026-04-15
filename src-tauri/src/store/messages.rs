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

    // Update FTS index
    conn.execute(
        "DELETE FROM messages_fts WHERE message_id = ?",
        params![msg.id],
    )?;
    conn.execute(
        "INSERT INTO messages_fts (session_id, message_id, role, kind, content_preview)
         VALUES (?, ?, ?, ?, ?)",
        params![msg.session_id, msg.id, msg.role, msg.kind, msg.content_preview],
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
