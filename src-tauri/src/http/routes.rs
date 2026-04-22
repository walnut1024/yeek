use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, post};
use axum::Json;
use axum::Router;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::app::commands::*;
use crate::app::errors::AppError;
use crate::http::app_state::HttpRuntimeState;
use crate::http::dto::*;

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn build_router(state: HttpRuntimeState) -> Router {
    let api = Router::new()
        // System
        .route("/system/status", get(system_status))
        .route("/system/rescan", post(rescan_sources))
        .route("/system/release-and-resync", post(release_and_resync))
        .route("/system/action-log", get(action_log))
        // Sessions
        .route("/sessions", get(browse_sessions))
        .route("/sessions/search", get(search_sessions))
        .route("/sessions/{id}/preview", get(session_preview))
        .route("/sessions/{id}/detail", get(session_detail))
        .route("/sessions/{id}/transcript", get(session_transcript))
        .route("/sessions/{id}/delete-plan", get(delete_plan))
        .route("/sessions/{id}/destructive-delete", post(destructive_delete))
        .route("/sessions/soft-delete", post(soft_delete))
        .route("/sessions/soft-delete-project", post(soft_delete_project))
        .route("/sessions/{session_id}/subagents/{subagent_id}", get(subagent_messages))
        .route("/sessions/resume", post(resume_session))
        // Plugins
        .route("/plugins", get(list_plugins))
        .route("/plugins/toggle", post(toggle_plugin))
        .route("/plugins/uninstall", post(uninstall_plugin))
        .route("/plugins/clean", post(clean_plugin))
        .route("/plugins/reinstall", post(reinstall_plugin))
        // Marketplaces
        .route("/marketplaces", get(list_marketplaces).post(add_marketplace))
        .route("/marketplaces/{name}/update", post(update_marketplace))
        .route("/marketplaces/{name}", delete(remove_marketplace))
        .route("/marketplaces/{name}/plugins", get(list_marketplace_plugins))
        .route("/marketplaces/install-plugin", post(install_marketplace_plugin))
        // SSE
        .route("/events", get(sse_handler))
        .with_state(state);

    Router::new()
        .nest("/api", api)
        .layer(CorsLayer::permissive())
}

// ---------------------------------------------------------------------------
// System handlers
// ---------------------------------------------------------------------------

async fn system_status(
    State(state): State<HttpRuntimeState>,
) -> Result<Json<SystemStatusPayload>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_system_status(&state.app_state)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn rescan_sources(
    State(state): State<HttpRuntimeState>,
) -> Result<Json<ActionResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_rescan_sources(&state.app_state)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn release_and_resync(
    State(state): State<HttpRuntimeState>,
) -> Result<Json<ActionResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_release_and_resync(&state.app_state)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn action_log(
    State(state): State<HttpRuntimeState>,
    Query(query): Query<ActionLogQuery>,
) -> Result<Json<ActionLogResponse>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_action_log(&state.app_state, query.limit)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// Session handlers
// ---------------------------------------------------------------------------

async fn browse_sessions(
    State(state): State<HttpRuntimeState>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<SessionListResponse>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_browse_sessions(&state.app_state, BrowseRequest {
            sort: query.sort,
            limit: query.limit,
            offset: query.offset,
        })
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn search_sessions(
    State(state): State<HttpRuntimeState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SessionListResponse>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_search_sessions(&state.app_state, SearchRequest {
            query: query.q,
            limit: query.limit,
            offset: query.offset,
        })
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn session_preview(
    State(state): State<HttpRuntimeState>,
    Path(id): Path<String>,
) -> Result<Json<SessionPreviewPayload>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_session_preview(&state.app_state, id)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn session_detail(
    State(state): State<HttpRuntimeState>,
    Path(id): Path<String>,
) -> Result<Json<SessionDetailPayload>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_session_detail(&state.app_state, id)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn session_transcript(
    State(state): State<HttpRuntimeState>,
    Path(id): Path<String>,
) -> Result<Json<crate::store::messages::TranscriptPayload>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_session_transcript(&state.app_state, id)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn delete_plan(
    State(state): State<HttpRuntimeState>,
    Path(id): Path<String>,
) -> Result<Json<crate::service::delete_planner::DeletePlan>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_delete_plan(&state.app_state, id)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn destructive_delete(
    State(state): State<HttpRuntimeState>,
    Path(id): Path<String>,
) -> Result<Json<crate::service::delete_planner::DestructiveDeleteResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_destructive_delete(&state.app_state, id)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn soft_delete(
    State(state): State<HttpRuntimeState>,
    Json(body): Json<SoftDeleteRequest>,
) -> Result<Json<ActionResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_soft_delete_sessions(&state.app_state, body.ids)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn soft_delete_project(
    State(state): State<HttpRuntimeState>,
    Json(body): Json<SoftDeleteProjectRequest>,
) -> Result<Json<ActionResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_soft_delete_project(&state.app_state, body.project_path)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn subagent_messages(
    State(state): State<HttpRuntimeState>,
    Path((session_id, subagent_id)): Path<(String, String)>,
) -> Result<Json<Vec<crate::store::messages::MessageRecord>>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_subagent_messages(&state.app_state, session_id, subagent_id)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn resume_session(
    Json(body): Json<ResumeRequest>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_resume_session(body.session_id, body.agent, body.cwd)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

// ---------------------------------------------------------------------------
// Plugin handlers
// ---------------------------------------------------------------------------

async fn list_plugins(
    State(state): State<HttpRuntimeState>,
    Query(query): Query<PluginScopeQuery>,
) -> Result<Json<crate::domain::plugin::SkillsOverview>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_list_plugins(&state.app_state, query.scope)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn toggle_plugin(
    Json(body): Json<PluginActionRequest>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_toggle_plugin(body.key)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

async fn uninstall_plugin(
    Json(body): Json<PluginActionRequest>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_uninstall_plugin(body.key)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

async fn clean_plugin(
    Json(body): Json<PluginActionRequest>,
) -> Result<Json<crate::domain::plugin::FixPluginResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_clean_plugin(body.key)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn reinstall_plugin(
    Json(body): Json<PluginActionRequest>,
) -> Result<Json<crate::domain::plugin::FixPluginResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_reinstall_plugin(body.key)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// Marketplace handlers
// ---------------------------------------------------------------------------

async fn list_marketplaces(
) -> Result<Json<crate::domain::plugin::MarketplaceListResult>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_list_marketplaces()
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn add_marketplace(
    Json(body): Json<AddMarketplaceRequest>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_add_marketplace(body.name, body.repo)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

async fn update_marketplace(
    Path(name): Path<String>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_update_marketplace(name)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

async fn remove_marketplace(
    Path(name): Path<String>,
    Query(query): Query<RemoveMarketplaceRequest>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_remove_marketplace(name, query.remove_plugins.unwrap_or(false))
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

async fn list_marketplace_plugins(
    Path(name): Path<String>,
) -> Result<Json<Vec<crate::domain::plugin::MarketplacePlugin>>, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        do_list_marketplace_plugins(name)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(result))
}

async fn install_marketplace_plugin(
    Json(body): Json<InstallMarketplacePluginRequest>,
) -> Result<Json<()>, AppError> {
    tokio::task::spawn_blocking(move || {
        do_install_marketplace_plugin(body.marketplace_name, body.plugin_name)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))?;
    Ok(Json(()))
}

// ---------------------------------------------------------------------------
// SSE handler
// ---------------------------------------------------------------------------

async fn sse_handler(
    State(state): State<HttpRuntimeState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|msg| {
        match msg {
            Ok(data) => Some(Ok(Event::default().data(data))),
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new().interval(Duration::from_secs(30)),
    )
}
