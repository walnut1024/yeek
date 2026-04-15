use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::app::errors::AppError;
use crate::app::state::AppState;
use crate::domain::session::SessionRecord;
use crate::store::sessions::{self, BrowseParams, SearchParams};
use crate::store::messages;
use crate::store::actions as action_store;

// --- System ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemStatusPayload {
    pub db_path: String,
    pub total_sessions: i64,
    pub total_sources: i64,
    pub last_sync_at: Option<String>,
    pub status: String,
}

#[tauri::command]
pub async fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatusPayload, AppError> {
    let db = state.db()?;

    let total_sessions: i64 = db
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .unwrap_or(0);

    let total_sources: i64 = db
        .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))
        .unwrap_or(0);

    let last_sync_at: Option<String> = db
        .query_row(
            "SELECT MAX(created_at) FROM action_log WHERE action = 'sync_completed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(None);

    Ok(SystemStatusPayload {
        db_path: "local".to_string(),
        total_sessions,
        total_sources,
        last_sync_at,
        status: "idle".to_string(),
    })
}

// --- Sessions Browse & Search ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionRecord>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct BrowseRequest {
    pub sort: Option<String>,
    pub group: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub visibility: Option<String>,
    pub agent: Option<String>,
    pub project_path: Option<String>,
    pub pinned_only: Option<bool>,
}

#[tauri::command]
pub async fn browse_sessions(
    state: State<'_, AppState>,
    request: BrowseRequest,
) -> Result<SessionListResponse, AppError> {
    let db = state.db()?;
    let params = BrowseParams {
        sort: request.sort.unwrap_or_else(|| "updated_at".to_string()),
        group: request.group,
        limit: request.limit.unwrap_or(50),
        offset: request.offset.unwrap_or(0),
        visibility: request.visibility.or_else(|| Some("visible".to_string())),
        agent: request.agent,
        project_path: request.project_path,
        pinned_only: request.pinned_only.unwrap_or(false),
    };

    let result = sessions::browse_sessions(&db, &params)?;
    Ok(SessionListResponse {
        sessions: result.sessions,
        total: result.total,
        has_more: result.has_more,
    })
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub visibility: Option<String>,
    pub agent: Option<String>,
}

#[tauri::command]
pub async fn search_sessions(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<SessionListResponse, AppError> {
    let db = state.db()?;
    let params = SearchParams {
        query: request.query,
        limit: request.limit.unwrap_or(50),
        offset: request.offset.unwrap_or(0),
        visibility: request.visibility.or_else(|| Some("visible".to_string())),
        agent: request.agent,
    };

    let result = sessions::search_sessions(&db, &params)?;
    Ok(SessionListResponse {
        sessions: result.sessions,
        total: result.total,
        has_more: result.has_more,
    })
}

// --- Session Detail ---

#[derive(Debug, Serialize)]
pub struct SessionPreviewPayload {
    pub record: SessionRecord,
    pub preview_messages: Vec<messages::MessagePreview>,
    pub source_count: i64,
}

#[tauri::command]
pub async fn get_session_preview(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionPreviewPayload, AppError> {
    let db = state.db()?;
    let record = sessions::get_session(&db, &session_id)?;
    let preview_messages = messages::get_preview_messages(&db, &session_id, 10)?;

    let source_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM session_sources WHERE session_id = ?",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(SessionPreviewPayload {
        record,
        preview_messages,
        source_count,
    })
}

#[derive(Debug, Serialize)]
pub struct SessionDetailPayload {
    pub record: SessionRecord,
    pub messages: Vec<messages::MessageRecord>,
    pub sources: Vec<crate::domain::source::SourceRef>,
}

#[tauri::command]
pub async fn get_session_detail(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionDetailPayload, AppError> {
    let db = state.db()?;
    let record = sessions::get_session(&db, &session_id)?;
    let msgs = messages::get_session_messages(&db, &session_id)?;
    let sources = crate::store::sources::get_session_sources(&db, &session_id)?;

    Ok(SessionDetailPayload {
        record,
        messages: msgs,
        sources,
    })
}

// --- Session Actions ---

#[derive(Debug, Serialize)]
pub struct ActionResult {
    pub success: bool,
    pub affected_count: i64,
}

#[tauri::command]
pub async fn set_pinned(
    state: State<'_, AppState>,
    ids: Vec<String>,
    value: bool,
) -> Result<ActionResult, AppError> {
    let db = state.db()?;
    let val = if value { "1" } else { "0" };
    sessions::set_session_field(&db, &ids, "pinned", val)?;
    action_store::record_action(
        &db,
        None,
        if value { "pin" } else { "unpin" },
        Some(&format!("{} sessions", ids.len())),
    )?;
    Ok(ActionResult {
        success: true,
        affected_count: ids.len() as i64,
    })
}

#[tauri::command]
pub async fn set_archived(
    state: State<'_, AppState>,
    ids: Vec<String>,
    value: bool,
) -> Result<ActionResult, AppError> {
    let db = state.db()?;
    let vis = if value {
        "archived"
    } else {
        "visible"
    };
    sessions::set_session_field(&db, &ids, "visibility", vis)?;
    action_store::record_action(
        &db,
        None,
        if value { "archive" } else { "unarchive" },
        Some(&format!("{} sessions", ids.len())),
    )?;
    Ok(ActionResult {
        success: true,
        affected_count: ids.len() as i64,
    })
}

#[tauri::command]
pub async fn set_hidden(
    state: State<'_, AppState>,
    ids: Vec<String>,
    value: bool,
) -> Result<ActionResult, AppError> {
    let db = state.db()?;
    let vis = if value {
        "hidden"
    } else {
        "visible"
    };
    sessions::set_session_field(&db, &ids, "visibility", vis)?;
    action_store::record_action(
        &db,
        None,
        if value { "hide" } else { "unhide" },
        Some(&format!("{} sessions", ids.len())),
    )?;
    Ok(ActionResult {
        success: true,
        affected_count: ids.len() as i64,
    })
}

#[tauri::command]
pub async fn soft_delete_sessions(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<ActionResult, AppError> {
    let db = state.db()?;
    sessions::soft_delete_sessions(&db, &ids)?;
    action_store::record_action(
        &db,
        None,
        "soft_delete",
        Some(&format!("{} sessions", ids.len())),
    )?;
    Ok(ActionResult {
        success: true,
        affected_count: ids.len() as i64,
    })
}

// --- Subagent Messages ---

#[tauri::command]
pub async fn get_subagent_messages(
    state: State<'_, AppState>,
    session_id: String,
    subagent_id: String,
) -> Result<Vec<messages::MessageRecord>, AppError> {
    let db = state.db()?;
    // Subagent session id is "{parent_session_id}:{agentId}"
    let sub_session_id = format!("{}:{}", session_id, subagent_id);
    let msgs = messages::get_session_messages(&db, &sub_session_id)?;
    Ok(msgs)
}

// --- Action Log ---

#[derive(Debug, Serialize)]
pub struct ActionLogResponse {
    pub actions: Vec<action_store::ActionLogEntry>,
}

#[tauri::command]
pub async fn get_action_log(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<ActionLogResponse, AppError> {
    let db = state.db()?;
    let actions = action_store::get_recent_actions(&db, limit.unwrap_or(50))?;
    Ok(ActionLogResponse { actions })
}

// --- Delete Planning ---

#[tauri::command]
pub async fn get_delete_plan(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::service::delete_planner::DeletePlan, AppError> {
    let db = state.db()?;
    let plan = crate::service::delete_planner::resolve_delete_plan(&db, &session_id)?;
    Ok(plan)
}

#[tauri::command]
pub async fn destructive_delete_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::service::delete_planner::DestructiveDeleteResult, AppError> {
    let db = state.db()?;
    let result = crate::service::delete_planner::execute_destructive_delete(&db, &session_id)?;
    Ok(result)
}

// --- Rescan ---

#[tauri::command]
pub async fn rescan_sources(state: State<'_, AppState>) -> Result<ActionResult, AppError> {
    // Emit sync started event
    if let Some(handle) = state.app_handle() {
        let _ = handle.emit("sync-started", crate::app::events::SyncStartedPayload {
            source_count: 0,
        });
    }

    let db = state.db()?;
    let result = crate::adapter::claudecode::index_all(&db)?;

    // Emit sync completed event
    if let Some(handle) = state.app_handle() {
        let _ = handle.emit("sync-completed", crate::app::events::SyncCompletedPayload {
            sessions_indexed: result.indexed,
            sessions_updated: result.updated,
            errors: result.errors,
        });
    }

    Ok(ActionResult {
        success: true,
        affected_count: result.indexed + result.updated,
    })
}
