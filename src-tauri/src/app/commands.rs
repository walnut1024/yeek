use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app::errors::AppError;
use crate::app::state::AppState;
use crate::domain::plugin;
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
pub fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatusPayload, AppError> {
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
pub fn browse_sessions(
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
pub fn search_sessions(
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
pub fn get_session_preview(
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
pub fn get_session_detail(
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
pub fn get_session_transcript(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<messages::TranscriptPayload, AppError> {
    let db = state.db()?;
    Ok(messages::get_session_transcript(&db, &session_id)?)
}

// --- Session Actions ---

/// Validate that a string matches UUID format (8-4-4-4-12 lowercase hex).
fn is_valid_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b[8] == b'-'
        && b[13] == b'-'
        && b[18] == b'-'
        && b[23] == b'-'
        && b[..8].iter().all(|c| c.is_ascii_hexdigit())
        && b[9..13].iter().all(|c| c.is_ascii_hexdigit())
        && b[14..18].iter().all(|c| c.is_ascii_hexdigit())
        && b[19..23].iter().all(|c| c.is_ascii_hexdigit())
        && b[24..].iter().all(|c| c.is_ascii_hexdigit())
}

/// Quote a string for safe use as a shell argument.
#[cfg(not(target_os = "windows"))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(target_os = "windows")]
fn shell_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

#[tauri::command]
pub fn resume_session(
    session_id: String,
    agent: String,
    cwd: Option<String>,
) -> Result<(), AppError> {
    // Validate inputs before constructing any shell command
    if !is_valid_uuid(&session_id) {
        return Err(AppError::Validation(format!(
            "Invalid session ID: {}",
            session_id
        )));
    }
    if let Some(ref d) = cwd {
        if !d.is_empty() && !std::path::Path::new(d).is_dir() {
            return Err(AppError::Validation(format!(
                "Invalid working directory: {}",
                d
            )));
        }
    }

    let sid = shell_quote(&session_id);
    let cmd = match agent.as_str() {
        "claude_code" | "claude_code_subagent" => format!("claude --resume {}", sid),
        "codex" => format!("codex resume {}", sid),
        _ => return Err(AppError::Internal(format!("Unknown agent: {}", agent))),
    };

    let cwd_ref = cwd.as_deref().filter(|s| !s.is_empty());
    launch_terminal(&cmd, cwd_ref).map_err(|e| AppError::Internal(e))
}

// ---------------------------------------------------------------------------
// Platform dispatch
// ---------------------------------------------------------------------------

fn launch_terminal(command: &str, cwd: Option<&str>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        launch_terminal_macos(command, cwd)
    }
    #[cfg(target_os = "linux")]
    {
        launch_terminal_linux(command, cwd)
    }
    #[cfg(target_os = "windows")]
    {
        launch_terminal_windows(command, cwd)
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn launch_terminal_macos(command: &str, cwd: Option<&str>) -> Result<(), String> {
    let terminals = ["Ghostty", "iTerm", "Warp", "WezTerm", "kitty", "Alacritty"];

    for &name in &terminals {
        if is_app_running(name) || app_exists(name) {
            return launch_with_open(command, name, cwd);
        }
    }

    launch_terminal_app(command, cwd)
}

#[cfg(target_os = "macos")]
fn is_app_running(bundle_id: &str) -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg(bundle_id)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn app_exists(name: &str) -> bool {
    std::path::Path::new(&format!("/Applications/{}.app", name)).exists()
        || std::path::Path::new(&format!(
            "{}/Applications/{}.app",
            std::env::var("HOME").unwrap_or_default(),
            name
        ))
        .exists()
}

#[cfg(target_os = "macos")]
fn launch_with_open(command: &str, app_name: &str, cwd: Option<&str>) -> Result<(), String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let full_cmd = cwd
        .map(|d| format!("cd {} && {}", shell_quote(d), command))
        .unwrap_or_else(|| command.to_string());

    match app_name {
        "Ghostty" => {
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
            let escaped = full_cmd.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                r#"tell application "iTerm"
    activate
    create window with default profile
    tell current session of current window
        write text "{escaped}"
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
            std::process::Command::new("open")
                .args(["-na", app_name, "--args", "-e", &shell, "-c", &full_cmd])
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {e}", app_name))?;
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn launch_terminal_app(command: &str, cwd: Option<&str>) -> Result<(), String> {
    let full_cmd = cwd
        .map(|d| format!("cd {} && {}", shell_quote(d), command))
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

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn launch_terminal_linux(command: &str, cwd: Option<&str>) -> Result<(), String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let full_cmd = cwd
        .map(|d| format!("cd {} && {}", shell_quote(d), command))
        .unwrap_or_else(|| command.to_string());

    let terminals = [
        ("ghostty", vec!["-e", &shell, "-c", &full_cmd]),
        ("wezterm", vec!["start", "--", &shell, "-c", &full_cmd]),
        ("kitty", vec!["-e", &shell, "-c", &full_cmd]),
        ("alacritty", vec!["-e", &shell, "-c", &full_cmd]),
        ("gnome-terminal", vec!["--", &shell, "-c", &full_cmd]),
        ("konsole", vec!["-e", &shell, "-c", &full_cmd]),
        ("xfce4-terminal", vec!["-e", &format!("{} -c {}", shell, shell_quote(&full_cmd))]),
    ];

    for (bin, args) in &terminals {
        if which_exists(bin) {
            return std::process::Command::new(bin)
                .args(args.iter().map(|s| s.as_str()))
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {e}", bin));
        }
    }

    // Fallback: xterm
    if which_exists("xterm") {
        return std::process::Command::new("xterm")
            .args(["-e", &shell, "-c", &full_cmd])
            .spawn()
            .map_err(|e| format!("Failed to launch xterm: {e}"));
    }

    Err("No terminal emulator found. Install ghostty, wezterm, kitty, alacritty, gnome-terminal, konsole, xfce4-terminal, or xterm.".to_string())
}

#[cfg(target_os = "linux")]
fn which_exists(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn launch_terminal_windows(command: &str, cwd: Option<&str>) -> Result<(), String> {
    let full_cmd = cwd
        .map(|d| format!("cd {}; {}", shell_quote(d), command))
        .unwrap_or_else(|| command.to_string());

    // Priority: PowerShell 7+, Windows PowerShell, Windows Terminal, cmd
    let candidates = [
        // pwsh (PowerShell 7+) — preferred
        ("pwsh.exe", vec!["-NoExit", "-Command", &full_cmd]),
        // Windows PowerShell (built-in)
        ("powershell.exe", vec!["-NoExit", "-Command", &full_cmd]),
    ];

    for (bin, args) in &candidates {
        if where_exists(bin) {
            // Use `start` via cmd to launch in a new window
            let mut start_args = vec!["/C", "start", bin];
            for a in args {
                start_args.push(a.as_str());
            }
            return std::process::Command::new("cmd")
                .args(&start_args)
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {e}", bin));
        }
    }

    // Fallback: Windows Terminal
    if where_exists("wt.exe") {
        let mut wt_args = vec!["-d"];
        if let Some(d) = cwd {
            wt_args.push(d);
        } else {
            wt_args.push(".");
        }
        wt_args.push("pwsh.exe");
        wt_args.push("-NoExit");
        wt_args.push("-Command");
        wt_args.push(&full_cmd);
        return std::process::Command::new("wt")
            .args(&wt_args)
            .spawn()
            .map_err(|e| format!("Failed to launch Windows Terminal: {e}"));
    }

    // Last resort: cmd
    std::process::Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", &full_cmd])
        .spawn()
        .map_err(|e| format!("Failed to launch cmd: {e}"))
}

#[cfg(target_os = "windows")]
fn where_exists(bin: &str) -> bool {
    std::process::Command::new("where")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[derive(Debug, Serialize)]
pub struct ActionResult {
    pub success: bool,
    pub affected_count: i64,
}

#[tauri::command]
pub fn soft_delete_sessions(
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
pub fn soft_delete_project(
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
pub fn get_subagent_messages(
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
pub fn get_action_log(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<ActionLogResponse, AppError> {
    let db = state.db()?;
    let actions = action_store::get_recent_actions(&db, limit.unwrap_or(50))?;
    Ok(ActionLogResponse { actions })
}

// --- Delete Planning ---

#[tauri::command]
pub fn get_delete_plan(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::service::delete_planner::DeletePlan, AppError> {
    let db = state.db()?;
    let plan = crate::service::delete_planner::resolve_delete_plan(&db, &session_id)?;
    Ok(plan)
}

#[tauri::command]
pub fn destructive_delete_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::service::delete_planner::DestructiveDeleteResult, AppError> {
    let db = state.db()?;
    let result = crate::service::delete_planner::execute_destructive_delete(&db, &session_id)?;
    Ok(result)
}

// --- Rescan ---

#[tauri::command]
pub fn rescan_sources(state: State<'_, AppState>) -> Result<ActionResult, AppError> {
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
pub fn release_and_resync(state: State<'_, AppState>) -> Result<ActionResult, AppError> {
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

// --- Plugin Helpers ---

fn read_json(path: &std::path::Path) -> Result<serde_json::Value, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("Failed to read {}: {}", path.display(), e)))?;
    serde_json::from_str(&content)
        .map_err(|e| AppError::ParseError(format!("Invalid JSON in {}: {}", path.display(), e)))
}

fn read_json_or_default(path: &std::path::Path) -> serde_json::Value {
    read_json(path).unwrap_or(serde_json::Value::Object(Default::default()))
}

fn scan_skills(plugin_path: &std::path::Path) -> Vec<plugin::SkillInfo> {
    let skills_dir = plugin_path.join("skills");
    if !skills_dir.exists() {
        return Vec::new();
    }
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let skill_md = entry.path().join("SKILL.md");
            if skill_md.exists() {
                if let Some(info) = parse_frontmatter(&skill_md, "skill") {
                    skills.push(info);
                }
            }
        }
    }
    skills
}

fn scan_agents(plugin_path: &std::path::Path) -> Vec<plugin::SkillInfo> {
    let agents_dir = plugin_path.join("agents");
    if !agents_dir.exists() {
        return Vec::new();
    }
    let mut agents = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(info) = parse_frontmatter(&path, "agent") {
                    agents.push(info);
                }
            }
        }
    }
    agents
}

fn has_hooks(plugin_path: &std::path::Path) -> bool {
    let hooks_file = plugin_path.join("hooks/hooks.json");
    hooks_file.exists() || plugin_path.join("hooks").join("session-start").exists()
}

fn parse_frontmatter(path: &std::path::Path, skill_type: &str) -> Option<plugin::SkillInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim_start();

    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let yaml_str = &rest[..end];

    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_str).ok()?;
    let name = yaml["name"].as_str().unwrap_or("").to_string();
    let description = yaml["description"].as_str().unwrap_or("").to_string();
    let tools = yaml["tools"].as_str().map(String::from);

    Some(plugin::SkillInfo {
        name,
        description,
        skill_type: skill_type.into(),
        tools,
        file_path: path.to_string_lossy().into_owned(),
        health: "ok".into(),
        health_detail: None,
    })
}

// --- Skills / Plugins ---

#[tauri::command]
pub fn list_plugins(
    state: State<'_, AppState>,
    scope: String,
) -> Result<plugin::SkillsOverview, AppError> {
    if scope == "project" {
        return list_project_plugins(&state);
    }
    list_global_plugins()
}

fn list_global_plugins() -> Result<plugin::SkillsOverview, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Read plugin registry
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let registry: serde_json::Value = read_json(&registry_path)?;

    // 2. Read enabled state
    let settings_path = claude_dir.join("settings.json");
    let settings: serde_json::Value = read_json_or_default(&settings_path);
    let enabled_map = settings.get("enabledPlugins").and_then(|v| v.as_object());

    // 3. Read marketplace metadata
    let marketplaces_path = claude_dir.join("plugins/known_marketplaces.json");
    let marketplaces: serde_json::Value = read_json_or_default(&marketplaces_path);

    let plugins_map = registry
        .get("plugins")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::ParseError("Invalid installed_plugins.json".into()))?;

    let mut plugins = Vec::new();
    let mut total_skills = 0usize;
    let mut total_agents = 0usize;
    let mut health_ok = 0usize;
    let mut health_partial = 0usize;
    let mut health_hook = 0usize;
    let mut health_broken = 0usize;

    for (key, entries) in plugins_map {
        let entries_arr = match entries.as_array() {
            Some(a) => a,
            None => continue,
        };
        let entry = match entries_arr.first() {
            Some(e) => e,
            None => continue,
        };

        let install_path = entry["installPath"].as_str().unwrap_or("").to_string();
        let version = entry["version"].as_str().unwrap_or("unknown").to_string();
        let installed_at = entry["installedAt"].as_str().map(String::from);
        let last_updated = entry["lastUpdated"].as_str().map(String::from);

        // Parse key: "plugin@marketplace"
        let parts: Vec<&str> = key.split('@').collect();
        let plugin_name = parts.first().map(|s| s.to_string()).unwrap_or_default();
        let market_name = parts.get(1).map(|s| s.to_string());

        // Enabled state
        let enabled = enabled_map
            .and_then(|m| m.get(key))
            .map(|v| v.as_bool().unwrap_or(true))
            .unwrap_or(true); // absent = enabled

        // Marketplace info
        let marketplace = market_name.as_ref().and_then(|mn| {
            let mkt = marketplaces.get(mn)?;
            let repo = mkt["source"]["repo"].as_str().unwrap_or("").to_string();
            let last_upd = mkt["lastUpdated"].as_str().map(String::from);
            Some(plugin::MarketplaceInfo {
                name: mn.clone(),
                repo,
                last_updated: last_upd,
            })
        });

        // Health check
        let path = std::path::Path::new(&install_path);
        let mut health_issues = Vec::new();

        let (skills, agents, health) = if !path.exists() {
            health_issues.push("Install path does not exist".into());
            (Vec::new(), Vec::new(), "broken")
        } else {
            let has_manifest = path.join(".claude-plugin/plugin.json").exists();
            let scanned_skills = scan_skills(path);
            let scanned_agents = scan_agents(path);

            if !has_manifest && scanned_skills.is_empty() && scanned_agents.is_empty() {
                if has_hooks(path) {
                    health_issues.push("Hook-only plugin, no skills or agents".into());
                    (scanned_skills, scanned_agents, "hook")
                } else {
                    health_issues.push("Missing plugin.json and no content".into());
                    (scanned_skills, scanned_agents, "broken")
                }
            } else if !has_manifest {
                health_issues.push("Missing plugin.json".into());
                (scanned_skills, scanned_agents, "partial")
            } else {
                (scanned_skills, scanned_agents, "ok")
            }
        };

        total_skills += skills.len();
        total_agents += agents.len();
        match health {
            "ok" => health_ok += 1,
            "partial" => health_partial += 1,
            "hook" => health_hook += 1,
            _ => health_broken += 1,
        }

        plugins.push(plugin::PluginInfo {
            key: key.clone(),
            name: plugin_name,
            version,
            scope: "global".into(),
            marketplace,
            install_path,
            enabled,
            health: health.into(),
            health_issues,
            skills,
            agents,
            installed_at,
            last_updated,
        });
    }

    Ok(plugin::SkillsOverview {
        total_plugins: plugins.len(),
        total_skills,
        total_agents,
        health_summary: plugin::HealthSummary {
            ok: health_ok,
            partial: health_partial,
            hook: health_hook,
            broken: health_broken,
        },
        plugins,
    })
}

fn list_project_plugins(state: &AppState) -> Result<plugin::SkillsOverview, AppError> {
    let db = state.db()?;
    let mut stmt = db.prepare("SELECT DISTINCT project_path FROM sessions WHERE project_path IS NOT NULL")
        .map_err(|e| AppError::DbError(e.to_string()))?;
    let paths: Vec<String> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| AppError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    let mut plugins = Vec::new();
    let mut total_skills = 0usize;
    let mut total_agents = 0usize;

    for project_path in &paths {
        let path = std::path::Path::new(project_path);
        let skills_dir = path.join(".claude/skills");
        let agents_dir = path.join(".claude/agents");

        let claude_dir = path.join(".claude");
        let skills = if skills_dir.exists() { scan_skills(&claude_dir) } else { Vec::new() };
        let agents = if agents_dir.exists() { scan_agents(&claude_dir) } else { Vec::new() };

        if skills.is_empty() && agents.is_empty() {
            continue;
        }

        total_skills += skills.len();
        total_agents += agents.len();

        let project_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        plugins.push(plugin::PluginInfo {
            key: project_path.clone(),
            name: project_name,
            version: String::new(),
            scope: "project".into(),
            marketplace: None,
            install_path: project_path.clone(),
            enabled: true,
            health: "ok".into(),
            health_issues: Vec::new(),
            skills,
            agents,
            installed_at: None,
            last_updated: None,
        });
    }

    Ok(plugin::SkillsOverview {
        total_plugins: plugins.len(),
        total_skills,
        total_agents,
        health_summary: plugin::HealthSummary {
            ok: plugins.len(),
            partial: 0,
            hook: 0,
            broken: 0,
        },
        plugins,
    })
}

// --- Plugin Toggle & Uninstall ---

#[tauri::command]
pub fn toggle_plugin(key: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let settings_path = home.join(".claude/settings.json");

    let mut settings: serde_json::Value = read_json(&settings_path)?;

    let enabled = settings
        .get_mut("enabledPlugins")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| AppError::Internal("No enabledPlugins in settings.json".into()))?;

    let current = enabled.get(&key).and_then(|v| v.as_bool()).unwrap_or(true);
    enabled.insert(key, serde_json::Value::Bool(!current));

    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| AppError::Internal(format!("Failed to serialize settings: {}", e)))?;
    std::fs::write(&settings_path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write settings: {}", e)))?;

    Ok(())
}

#[tauri::command]
pub fn uninstall_plugin(key: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Read registry, find install path, remove directory
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let mut registry: serde_json::Value = read_json(&registry_path)?;

    let install_path = registry
        .get("plugins")
        .and_then(|p| p.get(&key))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|e| e["installPath"].as_str())
        .ok_or_else(|| AppError::NotFound(format!("Plugin {} not found in registry", key)))?
        .to_string();

    let path = std::path::Path::new(&install_path);
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| AppError::DeleteFailed(format!("Failed to remove {}: {}", install_path, e)))?;
    }

    // 2. Remove from registry
    if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
        plugins.remove(&key);
    }
    let output = serde_json::to_string_pretty(&registry)
        .map_err(|e| AppError::Internal(format!("Failed to serialize registry: {}", e)))?;
    std::fs::write(&registry_path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write registry: {}", e)))?;

    // 3. Remove from enabledPlugins in settings.json
    let settings_path = claude_dir.join("settings.json");
    if let Ok(mut settings) = read_json(&settings_path) {
        if let Some(enabled) = settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut()) {
            enabled.remove(&key);
        }
        if let Ok(output) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&settings_path, output);
        }
    }

    Ok(())
}
