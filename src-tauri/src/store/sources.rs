use std::collections::HashSet;

use rusqlite::params;

use crate::app::errors::AppError;
use crate::domain::source::{DeletePolicy, SourceDescriptor, SourceRef};

/// Insert or update a source file reference.
///
/// Sources are files referenced by session messages (e.g., code files).
pub(crate) fn upsert_source(
    conn: &rusqlite::Connection,
    source: &SourceDescriptor,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    // Remove old source rows for the same path but different fingerprint (stale entries)
    conn.execute(
        "DELETE FROM sources WHERE path = ? AND id != ?",
        params![source.path, source.fingerprint],
    )?;

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

/// Link a source file to a session.
///
/// Creates a many-to-many relationship between sessions and their referenced sources.
pub(crate) fn link_session_source(
    conn: &rusqlite::Connection,
    session_id: &str,
    source_id: &str,
    source_type: &str,
    path: &str,
    delete_policy: &str,
) -> Result<(), AppError> {
    // Remove stale links for the same session + path with old source_id (old fingerprint)
    conn.execute(
        "DELETE FROM session_sources WHERE session_id = ? AND path = ? AND source_id != ?",
        params![session_id, path, source_id],
    )?;

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

/// Retrieve all source files referenced by a session.
///
/// Returns a list of source paths associated with the given session ID.
pub(crate) fn get_session_sources(
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

pub(crate) fn mark_source_deleted(
    conn: &rusqlite::Connection,
    path: &str,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO deleted_sources (path, deleted_at) VALUES (?, ?)",
        params![path, now],
    )?;
    Ok(())
}

pub(crate) fn get_deleted_source_paths(
    conn: &rusqlite::Connection,
) -> Result<HashSet<String>, AppError> {
    let mut stmt = conn.prepare("SELECT path FROM deleted_sources")?;
    let paths: HashSet<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(paths)
}
