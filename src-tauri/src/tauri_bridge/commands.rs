use tauri::State;

use crate::app::commands::*;
use crate::app::errors::AppError;
use crate::app::state::AppState;

#[tauri::command]
pub fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatusPayload, AppError> {
    do_system_status(&state)
}

#[tauri::command]
pub fn browse_sessions(
    state: State<'_, AppState>,
    request: BrowseRequest,
) -> Result<SessionListResponse, AppError> {
    do_browse_sessions(&state, request)
}

#[tauri::command]
pub fn search_sessions(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<SessionListResponse, AppError> {
    do_search_sessions(&state, request)
}

#[tauri::command]
pub fn get_session_preview(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionPreviewPayload, AppError> {
    do_session_preview(&state, session_id)
}

#[tauri::command]
pub fn get_session_detail(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionDetailPayload, AppError> {
    do_session_detail(&state, session_id)
}

#[tauri::command]
pub fn get_session_transcript(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::store::messages::TranscriptPayload, AppError> {
    do_session_transcript(&state, session_id)
}

#[tauri::command]
pub fn get_subagent_messages(
    state: State<'_, AppState>,
    session_id: String,
    subagent_id: String,
) -> Result<Vec<crate::store::messages::MessageRecord>, AppError> {
    do_subagent_messages(&state, session_id, subagent_id)
}

#[tauri::command]
pub fn soft_delete_sessions(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<ActionResult, AppError> {
    do_soft_delete_sessions(&state, ids)
}

#[tauri::command]
pub fn soft_delete_project(
    state: State<'_, AppState>,
    project_path: String,
) -> Result<ActionResult, AppError> {
    do_soft_delete_project(&state, project_path)
}

#[tauri::command]
pub fn get_action_log(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<ActionLogResponse, AppError> {
    do_action_log(&state, limit)
}

#[tauri::command]
pub fn rescan_sources(state: State<'_, AppState>) -> Result<ActionResult, AppError> {
    do_rescan_sources(&state)
}

#[tauri::command]
pub fn release_and_resync(state: State<'_, AppState>) -> Result<ActionResult, AppError> {
    do_release_and_resync(&state)
}

#[tauri::command]
pub fn resume_session(
    session_id: String,
    agent: String,
    cwd: Option<String>,
) -> Result<(), AppError> {
    do_resume_session(session_id, agent, cwd)
}

#[tauri::command]
pub fn get_delete_plan(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::service::delete_planner::DeletePlan, AppError> {
    do_delete_plan(&state, session_id)
}

#[tauri::command]
pub fn destructive_delete_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::service::delete_planner::DestructiveDeleteResult, AppError> {
    do_destructive_delete(&state, session_id)
}

#[tauri::command]
pub fn list_plugins(
    state: State<'_, AppState>,
    scope: String,
) -> Result<crate::domain::plugin::SkillsOverview, AppError> {
    do_list_plugins(&state, scope)
}

#[tauri::command]
pub fn toggle_plugin(key: String) -> Result<(), AppError> {
    do_toggle_plugin(key)
}

#[tauri::command]
pub fn uninstall_plugin(key: String) -> Result<(), AppError> {
    do_uninstall_plugin(key)
}

#[tauri::command]
pub fn clean_plugin(key: String) -> Result<crate::domain::plugin::FixPluginResult, AppError> {
    do_clean_plugin(key)
}

#[tauri::command]
pub fn reinstall_plugin(key: String) -> Result<crate::domain::plugin::FixPluginResult, AppError> {
    do_reinstall_plugin(key)
}

#[tauri::command]
pub fn list_marketplaces(
) -> Result<crate::domain::plugin::MarketplaceListResult, AppError> {
    do_list_marketplaces()
}

#[tauri::command]
pub fn add_marketplace(name: String, repo: String) -> Result<(), AppError> {
    do_add_marketplace(name, repo)
}

#[tauri::command]
pub fn update_marketplace(name: String) -> Result<(), AppError> {
    do_update_marketplace(name)
}

#[tauri::command]
pub fn remove_marketplace(name: String, remove_plugins: bool) -> Result<(), AppError> {
    do_remove_marketplace(name, remove_plugins)
}

#[tauri::command]
pub fn list_marketplace_plugins(
    marketplace_name: String,
) -> Result<Vec<crate::domain::plugin::MarketplacePlugin>, AppError> {
    do_list_marketplace_plugins(marketplace_name)
}

#[tauri::command]
pub fn install_marketplace_plugin(
    marketplace_name: String,
    plugin_name: String,
) -> Result<(), AppError> {
    do_install_marketplace_plugin(marketplace_name, plugin_name)
}
