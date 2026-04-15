use rusqlite::params;

use crate::app::errors::AppError;
use crate::domain::source::{DeletePolicy, SourceDescriptor, SourceRef};

pub fn upsert_source(
    conn: &rusqlite::Connection,
    source: &SourceDescriptor,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sources (id, agent, source_type, path, fingerprint, last_modified, last_seen_at, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active')
         ON CONFLICT(id) DO UPDATE SET
           fingerprint=excluded.fingerprint,
           last_modified=excluded.last_modified,
           last_seen_at=excluded.last_seen_at,
           status='active'",
        params![
            source.fingerprint, // use fingerprint as id
            source.agent,
            source.source_type,
            source.path,
            source.fingerprint,
            source.last_modified,
            now,
        ],
    )?;
    Ok(())
}

pub fn link_session_source(
    conn: &rusqlite::Connection,
    session_id: &str,
    source_id: &str,
    source_type: &str,
    path: &str,
    delete_policy: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO session_sources (session_id, source_id, source_type, path, delete_policy)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(session_id, source_id) DO UPDATE SET
           path=excluded.path,
           delete_policy=excluded.delete_policy",
        params![session_id, source_id, source_type, path, delete_policy],
    )?;
    Ok(())
}

pub fn get_session_sources(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<SourceRef>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT source_id, source_type, path, delete_policy FROM session_sources WHERE session_id = ?",
    )?;

    let sources = stmt
        .query_map(params![session_id], |row| {
            let policy_str: String = row.get(3)?;
            Ok(SourceRef {
                source_id: row.get(0)?,
                source_type: row.get(1)?,
                path: row.get(2)?,
                delete_policy: match policy_str.as_str() {
                    "hide_only" => DeletePolicy::HideOnly,
                    "file_safe" => DeletePolicy::FileSafe,
                    "needs_review" => DeletePolicy::NeedsReview,
                    _ => DeletePolicy::NotAllowed,
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sources)
}
