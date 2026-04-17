use serde::{Deserialize, Serialize};
use tauri::State;

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
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE parent_session_id IS NULL",
            [],
            |row| row.get(0),
        )
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
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[tauri::command]
pub async fn browse_sessions(
    state: State<'_, AppState>,
    request: BrowseRequest,
) -> Result<SessionListResponse, AppError> {
    let db = state.db()?;
    let params = BrowseParams {
        sort: request.sort.unwrap_or_else(|| "updated_at".to_string()),
        limit: request.limit.unwrap_or(50),
        offset: request.offset.unwrap_or(0),
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

// --- Transcript (tree-aware) ---

#[tauri::command]
pub async fn get_session_transcript(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<messages::TranscriptPayload, AppError> {
    let db = state.db()?;
    Ok(messages::get_session_transcript(&db, &session_id)?)
}

// --- Session Actions ---

#[tauri::command]
pub async fn resume_session(
    session_id: String,
    agent: String,
    cwd: Option<String>,
) -> Result<(), AppError> {
    let cmd = match agent.as_str() {
        "claude_code" | "claude_code_subagent" => format!("claude --resume {}", session_id),
        "codex" => format!("codex resume {}", session_id),
        _ => return Err(AppError::Internal(format!("Unknown agent: {}", agent))),
    };

    let cwd_ref = cwd.as_deref().filter(|s| !s.is_empty());
    launch_terminal(&cmd, cwd_ref).map_err(|e| AppError::Internal(e))
}

/// Detect the running terminal and launch command in it.
/// Falls back to macOS default Terminal.app via osascript.
fn launch_terminal(command: &str, cwd: Option<&str>) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Terminal resume is only supported on macOS".to_string());
    }

    let terminals = ["Ghostty", "iTerm", "Warp", "WezTerm", "kitty", "Alacritty"];

    // Try running terminal first, then installed
    for &name in &terminals {
        if is_app_running(name) || app_exists(name) {
            return launch_with_open(command, name, cwd);
        }
    }

    // Fallback: Terminal.app via osascript
    launch_terminal_app(command, cwd)
}

fn is_app_running(bundle_id: &str) -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg(bundle_id)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn app_exists(name: &str) -> bool {
    std::path::Path::new(&format!("/Applications/{}.app", name)).exists()
        || std::path::Path::new(&format!(
            "{}/Applications/{}.app",
            std::env::var("HOME").unwrap_or_default(),
            name
        ))
        .exists()
}

fn launch_with_open(command: &str, app_name: &str, cwd: Option<&str>) -> Result<(), String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    match app_name {
        "Ghostty" => {
            let full_cmd = cwd
                .map(|d| format!("cd \"{}\" && {}", d, command))
                .unwrap_or_else(|| command.to_string());
            std::process::Command::new("open")
                .args(["-na", "Ghostty", "--args"])
                .arg("-e")
                .arg(&shell)
                .arg("-c")
                .arg(&full_cmd)
                .spawn()
                .map_err(|e| format!("Failed to launch Ghostty: {e}"))?;
        }
        "iTerm" => {
            let escaped = command.replace('\\', "\\\\").replace('"', "\\\"");
            let cd_cmd = cwd
                .map(|d| format!("cd \"{}\" && ", d.replace('"', "\\\"")))
                .unwrap_or_default();
            let script = format!(
                r#"tell application "iTerm"
    activate
    create window with default profile
    tell current session of current window
        write text "{cd_cmd}{escaped}"
    end tell
end tell"#
            );
            std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .spawn()
                .map_err(|e| format!("Failed to launch iTerm: {e}"))?;
        }
        _ => {
            // Generic: prepend cd if cwd given
            let full_cmd = cwd
                .map(|d| format!("cd \"{}\" && {}", d, command))
                .unwrap_or_else(|| command.to_string());
            std::process::Command::new("open")
                .args(["-na", app_name, "--args", "-e", &shell, "-c", &full_cmd])
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {e}", app_name))?;
        }
    }

    Ok(())
}

fn launch_terminal_app(command: &str, cwd: Option<&str>) -> Result<(), String> {
    let full_cmd = cwd
        .map(|d| format!("cd \"{}\" && {}", d, command))
        .unwrap_or_else(|| command.to_string());
    let escaped = full_cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"tell application "Terminal"
    activate
    do script "{escaped}"
end tell"#
    );
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .spawn()
        .map_err(|e| format!("Failed to launch Terminal: {e}"))?;

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ActionResult {
    pub success: bool,
    pub affected_count: i64,
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

#[tauri::command]
pub async fn soft_delete_project(
    state: State<'_, AppState>,
    project_path: String,
) -> Result<ActionResult, AppError> {
    let db = state.db()?;
    let count = sessions::soft_delete_by_project(&db, &project_path)?;
    action_store::record_action(
        &db,
        None,
        "soft_delete_project",
        Some(&format!("{} sessions in {}", count, project_path)),
    )?;
    Ok(ActionResult {
        success: true,
        affected_count: count,
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
    let app_handle = state
        .app_handle()
        .ok_or_else(|| AppError::Internal("No app handle".to_string()))?
        .clone();
    let db_path = state.db_path.clone();
    let scan_guard = state.scan_guard.clone();

    let started = crate::sync::background::spawn_background_scan(
        db_path,
        app_handle,
        scan_guard,
    );

    if !started {
        return Err(AppError::Internal("Scan already in progress".to_string()));
    }

    Ok(ActionResult {
        success: true,
        affected_count: 0, // actual count arrives via sync-completed event
    })
}

// --- Release & Resync ---

#[tauri::command]
pub async fn release_and_resync(state: State<'_, AppState>) -> Result<ActionResult, AppError> {
    // 1. Clear all indexed data (keep schema and action_log for audit)
    {
        let db = state.db()?;
        db.execute_batch(
            "DELETE FROM messages_fts;
             DELETE FROM messages;
             DELETE FROM session_sources;
             DELETE FROM sources;
             DELETE FROM sessions;"
        )?;
        action_store::record_action(&db, None, "release", Some("Cleared all indexed data"))?;
    }

    // 2. Trigger full background rescan
    let app_handle = state
        .app_handle()
        .ok_or_else(|| AppError::Internal("No app handle".to_string()))?
        .clone();
    let db_path = state.db_path.clone();
    let scan_guard = state.scan_guard.clone();

    let started = crate::sync::background::spawn_background_scan(
        db_path,
        app_handle,
        scan_guard,
    );

    if !started {
        return Err(AppError::Internal("Scan already in progress".to_string()));
    }

    Ok(ActionResult {
        success: true,
        affected_count: 0,
    })
}
