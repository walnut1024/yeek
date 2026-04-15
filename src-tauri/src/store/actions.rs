use chrono::Utc;
use rusqlite::params;
use serde::Serialize;

use crate::app::errors::AppError;

#[derive(Debug, Serialize, Clone)]
pub struct ActionLogEntry {
    pub id: i64,
    pub session_id: Option<String>,
    pub action: String,
    pub detail: Option<String>,
    pub created_at: String,
}

pub fn record_action(
    conn: &rusqlite::Connection,
    session_id: Option<&str>,
    action: &str,
    detail: Option<&str>,
) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO action_log (session_id, action, detail, created_at) VALUES (?, ?, ?, ?)",
        params![session_id, action, detail, now],
    )?;
    Ok(())
}

pub fn get_recent_actions(
    conn: &rusqlite::Connection,
    limit: i64,
) -> Result<Vec<ActionLogEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, action, detail, created_at FROM action_log \
         ORDER BY created_at DESC LIMIT ?",
    )?;

    let entries = stmt
        .query_map(params![limit], |row| {
            Ok(ActionLogEntry {
                id: row.get("id")?,
                session_id: row.get("session_id")?,
                action: row.get("action")?,
                detail: row.get("detail")?,
                created_at: row.get("created_at")?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(entries)
}
