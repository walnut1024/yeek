use serde::{Deserialize, Serialize};

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

pub fn do_system_status(state: &AppState) -> Result<SystemStatusPayload, AppError> {
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

pub fn do_browse_sessions(
    state: &AppState,
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

pub fn do_search_sessions(
    state: &AppState,
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

pub fn do_session_preview(
    state: &AppState,
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

pub fn do_session_detail(
    state: &AppState,
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

pub fn do_session_transcript(
    state: &AppState,
    session_id: String,
) -> Result<messages::TranscriptPayload, AppError> {
    let db = state.db()?;
    Ok(messages::get_session_transcript(&db, &session_id)?)
}

// --- Session Actions ---

/// Validate that a string match UUID format (8-4-4-4-12 lowercase hex).
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

pub fn do_resume_session(
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

pub fn do_soft_delete_sessions(
    state: &AppState,
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

pub fn do_soft_delete_project(
    state: &AppState,
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

pub fn do_subagent_messages(
    state: &AppState,
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

pub fn do_action_log(
    state: &AppState,
    limit: Option<i64>,
) -> Result<ActionLogResponse, AppError> {
    let db = state.db()?;
    let actions = action_store::get_recent_actions(&db, limit.unwrap_or(50))?;
    Ok(ActionLogResponse { actions })
}

// --- Delete Planning ---

pub fn do_delete_plan(
    state: &AppState,
    session_id: String,
) -> Result<crate::service::delete_planner::DeletePlan, AppError> {
    let db = state.db()?;
    let plan = crate::service::delete_planner::resolve_delete_plan(&db, &session_id)?;
    Ok(plan)
}

pub fn do_destructive_delete(
    state: &AppState,
    session_id: String,
) -> Result<crate::service::delete_planner::DestructiveDeleteResult, AppError> {
    let db = state.db()?;
    let result = crate::service::delete_planner::execute_destructive_delete(&db, &session_id)?;
    Ok(result)
}

// --- Rescan ---

pub fn do_rescan_sources(state: &AppState) -> Result<ActionResult, AppError> {
    let emitter = state.event_emitter.clone();
    let db_path = state.db_path.clone();
    let scan_guard = state.scan_guard.clone();

    let started = crate::sync::background::spawn_background_scan(
        db_path,
        emitter,
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

pub fn do_release_and_resync(state: &AppState) -> Result<ActionResult, AppError> {
    // 1. Clear all indexed data (keep schema and action_log for audit)
    {
        let db = state.db()?;
        db.execute_batch(
            "DELETE FROM messages_fts;
             DELETE FROM messages;
             DELETE FROM session_sources;
             DELETE FROM sources;
             DELETE FROM sessions;
             DELETE FROM sqlite_sequence;"
        )?;
        action_store::record_action(&db, None, "release", Some("Cleared all indexed data"))?;
    }

    // 2. Trigger full background rescan
    let emitter = state.event_emitter.clone();
    let db_path = state.db_path.clone();
    let scan_guard = state.scan_guard.clone();

    let started = crate::sync::background::spawn_background_scan(
        db_path,
        emitter,
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

pub fn do_list_plugins(state: &AppState, scope: String) -> Result<plugin::SkillsOverview, AppError> {
    if scope == "project" {
        return list_project_plugins(state);
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

pub fn do_toggle_plugin(key: String) -> Result<(), AppError> {
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

pub fn do_uninstall_plugin(key: String) -> Result<(), AppError> {
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

// --- Plugin Fix: Clean & Reinstall ---

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dst)
        .map_err(|e| AppError::Internal(format!("Failed to create dir {}: {}", dst.display(), e)))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| AppError::Internal(format!("Failed to read dir {}: {}", src.display(), e)))?
    {
        let entry = entry.map_err(|e| AppError::Internal(format!("Dir entry error: {}", e)))?;
        // Skip symlinks
        if entry.path().symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| AppError::Internal(format!("Failed to copy {}: {}", src_path.display(), e)))?;
        }
    }
    Ok(())
}

/// Remove orphaned registry entry for a broken plugin (install dir missing/empty).
pub fn do_clean_plugin(key: String) -> Result<plugin::FixPluginResult, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Read registry, remove install dir if present
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let mut registry: serde_json::Value = read_json(&registry_path)?;

    if let Some(install_path) = registry
        .get("plugins")
        .and_then(|p| p.get(&key))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|e| e["installPath"].as_str())
    {
        let path = std::path::Path::new(install_path);
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    // 2. Remove from registry
    if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
        plugins.remove(&key);
    }
    let output = serde_json::to_string_pretty(&registry)
        .map_err(|e| AppError::Internal(format!("Failed to serialize registry: {}", e)))?;
    std::fs::write(&registry_path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write registry: {}", e)))?;

    // 3. Remove from enabledPlugins
    let settings_path = claude_dir.join("settings.json");
    if let Ok(mut settings) = read_json(&settings_path) {
        if let Some(enabled) = settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut()) {
            enabled.remove(&key);
        }
        if let Ok(output) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&settings_path, output);
        }
    }

    Ok(plugin::FixPluginResult {
        action: "clean".into(),
        message: format!("Cleaned orphaned entry for {}", key),
    })
}

/// Re-download a broken plugin from its marketplace (experimental).
pub fn do_reinstall_plugin(key: String) -> Result<plugin::FixPluginResult, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Parse key: "pluginName@marketplace"
    let parts: Vec<&str> = key.splitn(2, '@').collect();
    let plugin_name = parts.first().ok_or_else(|| AppError::Validation("Invalid plugin key".into()))?;
    let market_name = parts.get(1).ok_or_else(|| {
        AppError::Validation(format!("Plugin key '{}' missing marketplace suffix", key))
    })?;

    // 2. Read marketplace metadata
    let marketplaces_path = claude_dir.join("plugins/known_marketplaces.json");
    let marketplaces: serde_json::Value = read_json(&marketplaces_path)?;
    let mkt = marketplaces.get(market_name)
        .ok_or_else(|| AppError::NotFound(format!("Marketplace '{}' not found", market_name)))?;
    let repo = mkt["source"]["repo"].as_str()
        .ok_or_else(|| AppError::NotFound(format!("No repo for marketplace '{}'", market_name)))?;
    let clone_path_str = mkt["installLocation"].as_str().unwrap_or("");
    let clone_path = std::path::Path::new(clone_path_str);

    // 3. Read registry for install path and version
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let registry: serde_json::Value = read_json(&registry_path)?;
    let entry = registry
        .get("plugins")
        .and_then(|p| p.get(&key))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| AppError::NotFound(format!("Plugin '{}' not found in registry", key)))?;
    let install_path_str = entry["installPath"].as_str()
        .ok_or_else(|| AppError::NotFound(format!("No installPath for '{}'", key)))?;
    let git_sha = entry["gitCommitSha"].as_str().unwrap_or("");

    // 4. Ensure marketplace clone is available
    let clone_dir_exists = clone_path.exists();
    let has_git = clone_path.join(".git").exists();
    if clone_dir_exists && has_git {
        // Fetch latest
        let _ = std::process::Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(clone_path)
            .output();
        if !git_sha.is_empty() {
            let out = std::process::Command::new("git")
                .args(["checkout", git_sha])
                .current_dir(clone_path)
                .output()
                .map_err(|e| AppError::Internal(format!("git checkout failed: {}", e)))?;
            if !out.status.success() {
                return Err(AppError::Internal(format!(
                    "git checkout {} failed: {}", git_sha,
                    String::from_utf8_lossy(&out.stderr)
                )));
            }
        } else {
            let _ = std::process::Command::new("git")
                .args(["pull", "--ff-only"])
                .current_dir(clone_path)
                .output();
        }
    } else {
        // Directory exists but no .git (broken clone) — remove it first
        if clone_dir_exists {
            let _ = std::fs::remove_dir_all(clone_path);
        }
        // Clone the repo
        let clone_url = format!("https://github.com/{}.git", repo);
        let out = std::process::Command::new("git")
            .args(["clone", &clone_url, &clone_path.to_string_lossy()])
            .output()
            .map_err(|e| AppError::Internal(format!("git clone failed: {}", e)))?;
        if !out.status.success() {
            return Err(AppError::Internal(format!(
                "git clone failed: {}", String::from_utf8_lossy(&out.stderr)
            )));
        }
        if !git_sha.is_empty() {
            let _ = std::process::Command::new("git")
                .args(["checkout", git_sha])
                .current_dir(clone_path)
                .output();
        }
    }

    // 5. Find plugin source directory in marketplace
    let clone_path_buf = clone_path.to_path_buf();
    let candidates = [
        clone_path.join(format!("plugins/{}", plugin_name)),
        clone_path.join(format!("skills/{}", plugin_name)),
        clone_path.join(format!("agents/{}", plugin_name)),
    ];
    let source_dir = candidates.iter().find(|p| p.exists() && p.is_dir())
        .or_else(|| {
            // Single-plugin repo: use clone root if it has plugin-like content
            let has_skill = clone_path.join("skills").is_dir() || clone_path.join("SKILL.md").exists();
            let has_plugin_json = clone_path.join(".claude-plugin/plugin.json").exists();
            if has_skill || has_plugin_json {
                Some(&clone_path_buf)
            } else {
                None
            }
        })
        .ok_or_else(|| AppError::NotFound(format!(
            "Could not find plugin '{}' in marketplace '{}'", plugin_name, market_name
        )))?;

    // 6. Remove old install dir, copy fresh source
    let install_path = std::path::Path::new(install_path_str);
    if install_path.exists() {
        let _ = std::fs::remove_dir_all(install_path);
    }
    if let Some(parent) = install_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("Failed to create cache dir: {}", e)))?;
    }
    copy_dir_recursive(source_dir, install_path)?;

    // 7. Return success
    Ok(plugin::FixPluginResult {
        action: "reinstall".into(),
        message: format!("Reinstalled {} from {}", key, repo),
    })
}

// --- Marketplace Management ---

pub fn do_list_marketplaces() -> Result<plugin::MarketplaceListResult, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let marketplaces: serde_json::Value =
        read_json_or_default(&claude_dir.join("plugins/known_marketplaces.json"));
    let registry: serde_json::Value =
        read_json_or_default(&claude_dir.join("plugins/installed_plugins.json"));

    // Count plugins per marketplace from registry
    let mut plugin_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    if let Some(plugins) = registry.get("plugins").and_then(|v| v.as_object()) {
        for key in plugins.keys() {
            let market_name = key.split('@').last().unwrap_or("");
            if !market_name.is_empty() {
                *plugin_counts.entry(market_name.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut entries = Vec::new();
    if let Some(obj) = marketplaces.as_object() {
        for (name, val) in obj {
            let repo = val["source"]["repo"].as_str().unwrap_or("").to_string();
            let install_location = val["installLocation"].as_str().unwrap_or("").to_string();
            let last_updated = val["lastUpdated"].as_str().map(String::from);
            let plugin_count = *plugin_counts.get(name).unwrap_or(&0);

            entries.push(plugin::MarketplaceEntry {
                name: name.clone(),
                repo,
                install_location,
                last_updated,
                plugin_count,
            });
        }
    }

    Ok(plugin::MarketplaceListResult { marketplaces: entries })
}

pub fn do_add_marketplace(name: String, repo: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");
    let marketplaces_dir = claude_dir.join("plugins/marketplaces");
    std::fs::create_dir_all(&marketplaces_dir)
        .map_err(|e| AppError::Internal(format!("Failed to create marketplaces dir: {}", e)))?;

    // Clone repo
    let clone_url = format!("https://github.com/{}.git", repo);
    let dest = marketplaces_dir.join(&name);
    let out = std::process::Command::new("git")
        .args(["clone", &clone_url, &dest.to_string_lossy()])
        .output()
        .map_err(|e| AppError::Internal(format!("git clone failed: {}", e)))?;
    if !out.status.success() {
        return Err(AppError::Internal(format!(
            "git clone failed: {}", String::from_utf8_lossy(&out.stderr)
        )));
    }

    // Update known_marketplaces.json
    let path = claude_dir.join("plugins/known_marketplaces.json");
    let mut marketplaces: serde_json::Value = read_json_or_default(&path);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    marketplaces[&name] = serde_json::json!({
        "source": { "source": "github", "repo": repo },
        "installLocation": dest.to_string_lossy(),
        "lastUpdated": now
    });
    let output = serde_json::to_string_pretty(&marketplaces)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;
    std::fs::write(&path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write: {}", e)))?;

    Ok(())
}

pub fn do_update_marketplace(name: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let path = claude_dir.join("plugins/known_marketplaces.json");
    let mut marketplaces: serde_json::Value = read_json(&path)?;
    let mkt = marketplaces.get_mut(&name)
        .ok_or_else(|| AppError::NotFound(format!("Marketplace '{}' not found", name)))?;
    let clone_path = mkt["installLocation"].as_str().unwrap_or("");

    // git pull
    let out = std::process::Command::new("git")
        .args(["pull", "--ff-only"])
        .current_dir(clone_path)
        .output()
        .map_err(|e| AppError::Internal(format!("git pull failed: {}", e)))?;
    if !out.status.success() {
        return Err(AppError::Internal(format!(
            "git pull failed: {}", String::from_utf8_lossy(&out.stderr)
        )));
    }

    // Update lastUpdated
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    mkt["lastUpdated"] = serde_json::Value::String(now);
    let output = serde_json::to_string_pretty(&marketplaces)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;
    std::fs::write(&path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write: {}", e)))?;

    Ok(())
}

pub fn do_remove_marketplace(name: String, remove_plugins: bool) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let path = claude_dir.join("plugins/known_marketplaces.json");
    let mut marketplaces: serde_json::Value = read_json(&path)?;
    let install_location = marketplaces.get(&name)
        .and_then(|v| v["installLocation"].as_str())
        .unwrap_or("")
        .to_string();

    // Delete local clone
    if !install_location.is_empty() {
        let p = std::path::Path::new(&install_location);
        if p.exists() {
            let _ = std::fs::remove_dir_all(p);
        }
    }

    // Optionally remove all plugins belonging to this marketplace
    if remove_plugins {
        let registry_path = claude_dir.join("plugins/installed_plugins.json");
        if let Ok(mut registry) = read_json(&registry_path) {
            let keys_to_remove: Vec<String> = registry
                .get("plugins")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.keys()
                        .filter(|k| k.split('@').last() == Some(&name))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
                for key in &keys_to_remove {
                    // Remove install dir
                    if let Some(entry) = plugins.get(key)
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|e| e["installPath"].as_str())
                    {
                        let _ = std::fs::remove_dir_all(std::path::Path::new(entry));
                    }
                    plugins.remove(key);
                }
            }
            if let Ok(output) = serde_json::to_string_pretty(&registry) {
                let _ = std::fs::write(&registry_path, output);
            }
        }

        // Clean enabledPlugins
        let settings_path = claude_dir.join("settings.json");
        if let Ok(mut settings) = read_json(&settings_path) {
            if let Some(enabled) = settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut()) {
                let keys: Vec<String> = enabled.keys()
                    .filter(|k| k.split('@').last() == Some(&name))
                    .cloned()
                    .collect();
                for key in keys {
                    enabled.remove(&key);
                }
            }
            if let Ok(output) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(&settings_path, output);
            }
        }
    }

    // Remove from known_marketplaces.json
    if let Some(obj) = marketplaces.as_object_mut() {
        obj.remove(&name);
    }
    let output = serde_json::to_string_pretty(&marketplaces)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;
    std::fs::write(&path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write: {}", e)))?;

    Ok(())
}

// --- Marketplace Plugin Browser & Install ---

pub fn do_list_marketplace_plugins(marketplace_name: String) -> Result<Vec<plugin::MarketplacePlugin>, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let marketplaces: serde_json::Value = read_json(&claude_dir.join("plugins/known_marketplaces.json"))?;
    let clone_path_str = marketplaces[&marketplace_name]["installLocation"]
        .as_str().unwrap_or("");
    let clone_path = std::path::Path::new(clone_path_str);
    if !clone_path.exists() {
        return Ok(Vec::new());
    }

    let registry: serde_json::Value = read_json_or_default(&claude_dir.join("plugins/installed_plugins.json"));
    let installed_names: std::collections::HashSet<String> = registry
        .get("plugins").and_then(|v| v.as_object())
        .map(|obj| obj.keys()
            .filter(|k| k.split('@').last() == Some(marketplace_name.as_str()))
            .filter_map(|k| k.split('@').next())
            .map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let mut result = Vec::new();

    let plugins_dir = clone_path.join("plugins");
    let skills_dir = clone_path.join("skills");

    // Pattern 1: plugins/<name>/ (e.g., claude-plugins-official)
    if plugins_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let name = entry.file_name().to_string_lossy().into_owned();
                let skills = scan_skills(&path);
                let agents = scan_agents(&path);
                let desc = skills.iter().chain(agents.iter())
                    .next().map(|s| s.description.clone()).unwrap_or_else(|| name.clone());
                result.push(plugin::MarketplacePlugin {
                    installed: installed_names.contains(&name),
                    name, description: desc,
                    skill_count: skills.len(), agent_count: agents.len(),
                    has_hooks: has_hooks(&path),
                });
            }
        }
    }
    // Pattern 2: skills/<name>/ (e.g., anthropic-agent-skills)
    else if skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let name = entry.file_name().to_string_lossy().into_owned();
                let desc = path.join("SKILL.md").exists()
                    .then(|| parse_frontmatter(&path.join("SKILL.md"), "skill"))
                    .flatten().map(|s| s.description).unwrap_or_else(|| name.clone());
                result.push(plugin::MarketplacePlugin {
                    installed: installed_names.contains(&name),
                    name, description: desc,
                    skill_count: 1, agent_count: 0, has_hooks: false,
                });
            }
        }
    }
    // Pattern 3: root subdirectories with SKILL.md (e.g., axton-obsidian-visual-skills)
    else if let Ok(entries) = std::fs::read_dir(clone_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let name = entry.file_name().to_string_lossy().into_owned();
            // Skip common non-plugin directories
            if ["node_modules", ".git", "dist", "src", "test", "tests", "__tests__"].contains(&name.as_str()) { continue; }
            if !path.join("SKILL.md").exists() { continue; }
            let desc = parse_frontmatter(&path.join("SKILL.md"), "skill")
                .map(|s| s.description).unwrap_or_else(|| name.clone());
            result.push(plugin::MarketplacePlugin {
                installed: installed_names.contains(&name),
                name, description: desc,
                skill_count: 1, agent_count: 0, has_hooks: false,
            });
        }
    }

    Ok(result)
}

pub fn do_install_marketplace_plugin(marketplace_name: String, plugin_name: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let marketplaces: serde_json::Value = read_json(&claude_dir.join("plugins/known_marketplaces.json"))?;
    let clone_path_str = marketplaces[&marketplace_name]["installLocation"]
        .as_str().unwrap_or("");
    let clone_path = std::path::Path::new(clone_path_str);

    let source = clone_path.join(format!("plugins/{}", plugin_name));
    let source = if source.is_dir() { source } else { clone_path.join(format!("skills/{}", plugin_name)) };
    if !source.is_dir() {
        return Err(AppError::NotFound(format!("Plugin '{}' not found in '{}'", plugin_name, marketplace_name)));
    }

    let git_sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(clone_path)
        .output().ok()
        .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
        .unwrap_or_else(|| "unknown".to_string());

    let install_path = claude_dir.join(format!("plugins/cache/{}/{}/{}", marketplace_name, plugin_name, git_sha));
    std::fs::create_dir_all(&install_path)
        .map_err(|e| AppError::Internal(format!("Failed to create dir: {}", e)))?;
    copy_dir_recursive(&source, &install_path)?;

    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let mut registry: serde_json::Value = read_json(&registry_path)?;
    let key = format!("{}@{}", plugin_name, marketplace_name);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let plugins = registry.get_mut("plugins").and_then(|v| v.as_object_mut())
        .ok_or_else(|| AppError::Internal("Invalid registry".into()))?;
    plugins.insert(key.clone(), serde_json::json!([{
        "scope": "user",
        "installPath": install_path.to_string_lossy(),
        "version": git_sha,
        "installedAt": now,
        "lastUpdated": now,
        "gitCommitSha": git_sha,
    }]));

    let output = serde_json::to_string_pretty(&registry)
        .map_err(|e| AppError::Internal(format!("Serialize: {}", e)))?;
    std::fs::write(&registry_path, output)
        .map_err(|e| AppError::Internal(format!("Write: {}", e)))?;

    let settings_path = claude_dir.join("settings.json");
    if let Ok(mut settings) = read_json(&settings_path) {
        if let Some(enabled) = settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut()) {
            enabled.insert(key, serde_json::Value::Bool(true));
        }
        if let Ok(output) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&settings_path, output);
        }
    }

    Ok(())
}
