use chrono::Utc;
use rusqlite::params;

use crate::app::errors::AppError;
use crate::domain::session::{
    DeleteMode, SessionRecord, SessionStatus, VisibilityStatus,
};

pub struct SessionListResult {
    pub sessions: Vec<SessionRecord>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct BrowseParams {
    pub sort: String,
    pub limit: i64,
    pub offset: i64,
}

impl Default for BrowseParams {
    fn default() -> Self {
        Self {
            sort: "updated_at".to_string(),
            limit: 50,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub limit: i64,
    pub offset: i64,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 50,
            offset: 0,
        }
    }
}

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        id: row.get("id")?,
        agent: row.get("agent")?,
        project_path: row.get("project_path")?,
        title: row.get("title")?,
        model: row.get("model")?,
        git_branch: row.get("git_branch")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        status: match row.get::<_, String>("status")?.as_str() {
            "active" => SessionStatus::Active,
            "complete" => SessionStatus::Complete,
            _ => SessionStatus::Partial,
        },
        visibility: match row.get::<_, String>("visibility")?.as_str() {
            "hidden" => VisibilityStatus::Hidden,
            "archived" => VisibilityStatus::Archived,
            _ => VisibilityStatus::Visible,
        },
        pinned: row.get::<_, i64>("pinned")? != 0,
        archived_at: row.get("archived_at")?,
        deleted_at: row.get("deleted_at")?,
        delete_mode: match row.get::<_, String>("delete_mode")?.as_str() {
            "soft_deleted" => DeleteMode::SoftDeleted,
            "source_deleted" => DeleteMode::SourceDeleted,
            _ => DeleteMode::None,
        },
        message_count: row.get("message_count")?,
        updated_at: row.get("updated_at")?,
        parent_session_id: row.get::<_, Option<String>>("parent_session_id").unwrap_or(None),
    })
}

pub fn browse_sessions(
    conn: &rusqlite::Connection,
    params: &BrowseParams,
) -> Result<SessionListResult, AppError> {
    let order_by = match params.sort.as_str() {
        "updated_at_asc" => "updated_at ASC",
        "started_at" => "started_at DESC",
        "started_at_asc" => "started_at ASC",
        _ => "updated_at DESC",
    };

    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE parent_session_id IS NULL",
        [],
        |row| row.get(0),
    )?;

    let query_sql = format!(
        "SELECT * FROM sessions WHERE parent_session_id IS NULL ORDER BY {} LIMIT ? OFFSET ?",
        order_by
    );

    let mut stmt = conn.prepare(&query_sql)?;
    let sessions = stmt
        .query_map(params![params.limit, params.offset], row_to_session)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(SessionListResult {
        sessions,
        total,
        has_more: (params.offset + params.limit) < total,
    })
}

pub fn search_sessions(
    conn: &rusqlite::Connection,
    params: &SearchParams,
) -> Result<SessionListResult, AppError> {
    let like_pattern = format!("%{}%", params.query);
    let fts_query = params.query
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");

    let where_sql = "(s.title LIKE ?1 OR s.project_path LIKE ?1 OR s.id IN (SELECT fts.session_id FROM messages_fts fts WHERE messages_fts MATCH ?2)) AND s.parent_session_id IS NULL";

    let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
        Box::new(like_pattern),
        Box::new(fts_query),
    ];

    let count_sql = format!("SELECT COUNT(DISTINCT s.id) FROM sessions s WHERE {}", where_sql);
    let total: i64 = conn
        .query_row(
            &count_sql,
            rusqlite::params_from_iter(param_values.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )
        .unwrap_or(0);

    let query_sql = format!(
        "SELECT DISTINCT s.* FROM sessions s WHERE {} ORDER BY s.updated_at DESC LIMIT ?3 OFFSET ?4",
        where_sql
    );
    let mut all_params = param_values;
    all_params.push(Box::new(params.limit));
    all_params.push(Box::new(params.offset));

    let mut stmt = conn.prepare(&query_sql)?;
    let sessions: Vec<SessionRecord> = stmt
        .query_map(
            rusqlite::params_from_iter(all_params.iter().map(|p| p.as_ref())),
            row_to_session,
        )?
        .filter_map(|r| r.ok())
        .collect();

    Ok(SessionListResult {
        sessions,
        total,
        has_more: (params.offset + params.limit) < total,
    })
}

pub fn get_session(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<SessionRecord, AppError> {
    conn.query_row(
        "SELECT * FROM sessions WHERE id = ?",
        params![id],
        row_to_session,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(id.to_string()),
        e => AppError::DbError(e.to_string()),
    })
}

pub fn upsert_session(
    conn: &rusqlite::Connection,
    session: &SessionRecord,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO sessions (id, agent, project_path, title, model, git_branch,
         started_at, ended_at, status, visibility, pinned, archived_at, deleted_at,
         delete_mode, message_count, updated_at, parent_session_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           project_path=excluded.project_path,
           title=excluded.title,
           model=excluded.model,
           git_branch=excluded.git_branch,
           started_at=excluded.started_at,
           ended_at=excluded.ended_at,
           status=excluded.status,
           message_count=excluded.message_count,
           updated_at=excluded.updated_at",
        params![
            session.id,
            session.agent,
            session.project_path,
            session.title,
            session.model,
            session.git_branch,
            session.started_at,
            session.ended_at,
            session.status.as_str(),
            session.visibility.as_str(),
            session.pinned as i64,
            session.archived_at,
            session.deleted_at,
            session.delete_mode.as_str(),
            session.message_count,
            session.updated_at,
            session.parent_session_id,
        ],
    )?;
    Ok(())
}

pub fn set_session_field(
    conn: &rusqlite::Connection,
    ids: &[String],
    field: &str,
    value: &str,
) -> Result<(), AppError> {
    let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
    let sql = format!(
        "UPDATE sessions SET {} = ?1, updated_at = ?0 WHERE id IN ({})",
        field,
        placeholders.join(", ")
    );

    let now = Utc::now().to_rfc3339();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(now), Box::new(value.to_string())];
    for id in ids {
        params_vec.push(Box::new(id.clone()));
    }

    conn.execute(
        &sql,
        rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
    )?;
    Ok(())
}

pub fn soft_delete_sessions(
    conn: &rusqlite::Connection,
    ids: &[String],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!(
        "UPDATE sessions SET visibility = 'hidden', delete_mode = 'soft_deleted', \
         deleted_at = ?, updated_at = ? WHERE id IN ({})",
        placeholders.join(", ")
    );

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(now.clone()), Box::new(now)];
    for id in ids {
        params.push(Box::new(id.clone()));
    }

    conn.execute(
        &sql,
        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
    )?;
    Ok(())
}

pub fn soft_delete_by_project(
    conn: &rusqlite::Connection,
    project_path: &str,
) -> Result<i64, AppError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE sessions SET visibility = 'hidden', delete_mode = 'soft_deleted', \
         deleted_at = ?, updated_at = ? WHERE project_path = ?",
        params![now, now, project_path],
    )?;
    Ok(conn.changes() as i64)
}
