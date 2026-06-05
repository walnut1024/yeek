use serde::{Deserialize, Serialize};

use crate::app::errors::AppError;
use crate::app::state::AppState;
use crate::domain::plugin;
use crate::domain::session::SessionRecord;
use crate::store::actions as action_store;
use crate::store::messages;
use crate::store::sessions::{self, BrowseParams, SearchParams};

// --- System ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemStatusPayload {
    pub db_path: String,
    pub total_sessions: i64,
    pub total_sources: i64,
    pub total_projects: i64,
    pub total_messages: i64,
    pub active_sessions: i64,
    pub complete_sessions: i64,
    pub partial_sessions: i64,
    pub last_sync_at: Option<String>,
    pub status: String,
}

/// Get system health status including sync state, index size, and activity log.
///
/// Returns a [`SystemStatusPayload`] with pulse data for the System dashboard.
pub(crate) fn do_system_status(state: &AppState) -> Result<SystemStatusPayload, AppError> {
    let db = state.db()?;

    let total_sessions: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE parent_session_id IS NULL AND visibility = 'visible' AND delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_sources: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT ss.source_id) FROM session_sources ss JOIN sessions s ON ss.session_id = s.id WHERE s.visibility = 'visible' AND s.delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_projects: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT project_path) FROM sessions WHERE project_path IS NOT NULL AND visibility = 'visible' AND delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_messages: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(message_count), 0) FROM sessions WHERE parent_session_id IS NULL AND visibility = 'visible' AND delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let active_sessions: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE parent_session_id IS NULL AND status = 'active' AND visibility = 'visible' AND delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let complete_sessions: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE parent_session_id IS NULL AND status = 'complete' AND visibility = 'visible' AND delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let partial_sessions: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE parent_session_id IS NULL AND status = 'partial' AND visibility = 'visible' AND delete_mode = 'none'",
            [],
            |row| row.get(0),
        )
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
        total_projects,
        total_messages,
        active_sessions,
        complete_sessions,
        partial_sessions,
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
    pub agent: Option<String>,
}

/// Browse top-level sessions with sorting, pagination, and optional project filter.
///
/// Returns [`SessionListResponse`] with enriched session cards.
pub(crate) fn do_browse_sessions(
    state: &AppState,
    request: BrowseRequest,
) -> Result<SessionListResponse, AppError> {
    let db = state.db()?;
    let agent =
        request.agent.filter(|a| ["claude_code", "codex", "opencode"].contains(&a.as_str()));
    let params = BrowseParams {
        sort: request.sort.unwrap_or_else(|| "updated_at".to_string()),
        limit: request.limit.unwrap_or(50),
        offset: request.offset.unwrap_or(0),
        agent,
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
    pub agent: Option<String>,
}

/// Full-text search across all sessions.
///
/// Uses FTS5 to search titles, messages, and model names. Results include
/// highlighted preview snippets.
pub(crate) fn do_search_sessions(
    state: &AppState,
    request: SearchRequest,
) -> Result<SessionListResponse, AppError> {
    let db = state.db()?;
    let params = SearchParams {
        query: request.query,
        limit: request.limit.unwrap_or(50),
        offset: request.offset.unwrap_or(0),
        agent: request.agent.filter(|a| ["claude_code", "codex", "opencode"].contains(&a.as_str())),
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

pub(crate) fn do_session_preview(
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

    Ok(SessionPreviewPayload { record, preview_messages, source_count })
}

#[derive(Debug, Serialize)]
pub struct SessionDetailPayload {
    pub record: SessionRecord,
    pub messages: Vec<messages::MessageRecord>,
    pub sources: Vec<crate::domain::source::SourceRef>,
}

/// Get full session detail including message tree and source file references.
///
/// Returns [`SessionDetailResponse`] for the session inspector view.
pub(crate) fn do_session_detail(
    state: &AppState,
    session_id: String,
) -> Result<SessionDetailPayload, AppError> {
    let db = state.db()?;
    let record = sessions::get_session(&db, &session_id)?;
    let msgs = messages::get_session_messages(&db, &session_id)?;
    let sources = crate::store::sources::get_session_sources(&db, &session_id)?;

    Ok(SessionDetailPayload { record, messages: msgs, sources })
}

// --- Transcript (tree-aware) ---

/// Get the complete transcript of a session as a flat message list.
///
/// Returns [`TranscriptPayload`] suitable for export or copy.
pub(crate) fn do_session_transcript(
    state: &AppState,
    session_id: String,
) -> Result<messages::TranscriptPayload, AppError> {
    let db = state.db()?;
    messages::get_session_transcript(&db, &session_id)
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
///
/// On Unix: wraps in single quotes, escaping embedded single quotes.
/// On Windows: wraps in double quotes, escaping embedded double quotes.
///
/// Use this to sanitize any user-provided or externally-sourced values
/// before embedding them in shell commands passed to [`launch_terminal`].
#[cfg(not(target_os = "windows"))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(target_os = "windows")]
fn shell_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

/// Resume a session by launching the agent CLI in a terminal.
///
/// Validates the session ID (must be UUID) and opens the appropriate
/// terminal emulator with the agent resume command.
///
/// If `terminal` is provided, it will be tried first; if unavailable,
/// falls back to the platform-specific priority list.
pub(crate) fn do_resume_session(
    session_id: String,
    agent: String,
    cwd: Option<String>,
    terminal: Option<String>,
) -> Result<(), AppError> {
    // Validate inputs before constructing any shell command
    if !is_valid_uuid(&session_id) {
        return Err(AppError::Validation(format!("Invalid session ID: {}", session_id)));
    }
    if let Some(ref d) = cwd {
        if !d.is_empty() && !std::path::Path::new(d).is_dir() {
            return Err(AppError::Validation(format!("Invalid working directory: {}", d)));
        }
    }

    let sid = shell_quote(&session_id);
    let cmd = match agent.as_str() {
        "claude_code" | "claude_code_subagent" => format!("claude --resume {}", sid),
        "codex" => format!("codex resume {}", sid),
        _ => return Err(AppError::Internal(format!("Unknown agent: {}", agent))),
    };

    let cwd_ref = cwd.as_deref().filter(|s| !s.is_empty());
    launch_terminal(&cmd, cwd_ref, terminal.as_deref()).map_err(AppError::Internal)
}

// ---------------------------------------------------------------------------
// Platform dispatch
// ---------------------------------------------------------------------------

/// Launch a terminal emulator with the given shell command.
///
/// If `preferred` is provided and the named terminal is available, it is used
/// directly. Otherwise falls back to the platform-specific priority list.
///
/// # Safety
/// Callers MUST ensure `command` is free of shell injection. Arguments within
/// `command` should be individually quoted via [`shell_quote`] before assembly.
/// The `cwd` parameter is automatically quoted.
fn launch_terminal(
    command: &str,
    cwd: Option<&str>,
    preferred: Option<&str>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        launch_terminal_macos(command, cwd, preferred)
    }
    #[cfg(target_os = "linux")]
    {
        launch_terminal_linux(command, cwd, preferred)
    }
    #[cfg(target_os = "windows")]
    {
        launch_terminal_windows(command, cwd, preferred)
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn launch_terminal_macos(
    command: &str,
    cwd: Option<&str>,
    preferred: Option<&str>,
) -> Result<(), String> {
    // If user picked a specific terminal, try it first
    if let Some(name) = preferred {
        let app_name = macos_terminal_app_name(name);
        if !app_name.is_empty() && app_name != "Terminal.app" {
            if is_app_running(app_name) || app_exists(app_name) {
                return launch_with_open(command, app_name, cwd);
            }
        }
        if app_name == "Terminal.app" {
            return launch_terminal_app(command, cwd);
        }
    }

    // Fallback: priority-based discovery
    let terminals = [
        "Ghostty",
        "iTerm2",
        "Terminal.app",
        "cmux",
        "Warp",
        "WezTerm",
        "kitty",
        "Alacritty",
    ];

    for &name in &terminals {
        let app_name = macos_terminal_app_name(name);
        if app_name == "Terminal.app" {
            return launch_terminal_app(command, cwd);
        }
        if is_app_running(app_name) || app_exists(app_name) {
            return launch_with_open(command, app_name, cwd);
        }
    }

    launch_terminal_app(command, cwd)
}

#[cfg(target_os = "macos")]
fn macos_terminal_app_name(name: &str) -> &str {
    match name {
        "iTerm2" => "iTerm",
        other => other,
    }
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
        "Ghostty" | "cmux" => {
            std::process::Command::new("open")
                .args(macos_open_new_app_args(app_name))
                .args(ghostty_launch_args_macos(command, cwd))
                .spawn()
                .map_err(|e| format!("Failed to launch {app_name}: {e}"))?;
        },
        "iTerm" => {
            let script = iterm_resume_applescript(&full_cmd);
            run_osascript(&script, "iTerm")?;
        },
        _ => {
            let shell_args = unix_shell_command_args(&full_cmd, &shell);
            std::process::Command::new("open")
                .args(macos_open_app_args(app_name))
                .arg("-e")
                .args(shell_args)
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {e}", app_name))?;
        },
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_open_app_args(app_name: &str) -> Vec<&str> {
    vec!["-a", app_name, "--args"]
}

#[cfg(target_os = "macos")]
fn macos_open_new_app_args(app_name: &str) -> Vec<&str> {
    vec!["-n", "-a", app_name, "--args"]
}

#[cfg(target_os = "macos")]
fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str, app_name: &str) -> Result<(), String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Failed to launch {app_name}: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("Failed to launch {app_name}: osascript exited with {}", output.status))
    } else {
        Err(format!("Failed to launch {app_name}: {stderr}"))
    }
}

#[cfg(target_os = "macos")]
fn iterm_resume_applescript(full_cmd: &str) -> String {
    let escaped = applescript_escape(full_cmd);
    format!(
        r#"tell application "iTerm"
    activate
    if (count of windows) = 0 then
        create window with default profile
    else
        tell current window
            create tab with default profile
        end tell
    end if
    tell current session of current window
        write text "{escaped}"
    end tell
end tell"#
    )
}

#[cfg(target_os = "macos")]
fn ghostty_launch_args_macos(command: &str, cwd: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(dir) = cwd {
        args.push(format!("--working-directory={}", dir));
    }

    args.push(format!("--initial-command=shell:{}", command));
    args
}

#[cfg(target_os = "linux")]
fn ghostty_launch_args(command: &str, cwd: Option<&str>, shell: &str) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(dir) = cwd {
        args.push(format!("--working-directory={}", dir));
    }

    args.push("-e".to_string());
    args.extend(unix_shell_command_args(command, shell));
    args
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_shell_command_args(command: &str, shell: &str) -> Vec<String> {
    match std::path::Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
    {
        Some("bash" | "zsh") => vec![shell.to_string(), "-lic".to_string(), command.to_string()],
        Some("fish") => vec![
            shell.to_string(),
            "-l".to_string(),
            "-i".to_string(),
            "-c".to_string(),
            command.to_string(),
        ],
        _ => vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!(
                "export PATH=\"$PATH:/opt/homebrew/bin:/usr/local/bin:$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.npm-global/bin\"; exec {}",
                command
            ),
        ],
    }
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
fn launch_terminal_linux(
    command: &str,
    cwd: Option<&str>,
    preferred: Option<&str>,
) -> Result<(), String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let full_cmd = cwd
        .map(|d| format!("cd {} && {}", shell_quote(d), command))
        .unwrap_or_else(|| command.to_string());

    let shell_args = unix_shell_command_args(&full_cmd, &shell);
    let shell_command = shell_args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let terminals: Vec<(&str, Vec<String>)> = Vec::from([
        (
            "wezterm",
            Vec::from(["start".to_string(), "--".to_string()])
                .into_iter()
                .chain(shell_args.clone())
                .collect(),
        ),
        (
            "kitty",
            Vec::from(["-e".to_string()])
                .into_iter()
                .chain(shell_args.clone())
                .collect(),
        ),
        (
            "alacritty",
            Vec::from(["-e".to_string()])
                .into_iter()
                .chain(shell_args.clone())
                .collect(),
        ),
        (
            "gnome-terminal",
            Vec::from(["--".to_string()])
                .into_iter()
                .chain(shell_args.clone())
                .collect(),
        ),
        (
            "konsole",
            Vec::from(["-e".to_string()])
                .into_iter()
                .chain(shell_args.clone())
                .collect(),
        ),
        ("xfce4-terminal", vec!["-e".to_string(), shell_command]),
    ]);
    let xterm_args: Vec<String> = Vec::from(["-e".to_string()])
        .into_iter()
        .chain(shell_args.clone())
        .collect();

    // If user picked a specific terminal, try it first
    if let Some(name) = preferred {
        if !name.is_empty() {
            if name == "ghostty" && which_exists("ghostty") {
                return launch_ghostty_linux(command, cwd, &shell);
            }
            for (bin, args) in &terminals {
                if *bin == name && which_exists(bin) {
                    return std::process::Command::new(bin)
                        .args(args)
                        .spawn()
                        .map_err(|e| format!("Failed to launch {}: {e}", bin));
                }
            }
        }
    }

    // Fallback: priority-based discovery
    if which_exists("ghostty") {
        return launch_ghostty_linux(command, cwd, &shell);
    }

    for (bin, args) in &terminals {
        if which_exists(bin) {
            return std::process::Command::new(bin)
                .args(args)
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {e}", bin));
        }
    }

    // Fallback: xterm
    if which_exists("xterm") {
        return std::process::Command::new("xterm")
            .args(&xterm_args)
            .spawn()
            .map_err(|e| format!("Failed to launch xterm: {e}"));
    }

    Err("No terminal emulator found. Install ghostty, wezterm, kitty, alacritty, gnome-terminal, konsole, xfce4-terminal, or xterm.".to_string())
}

#[cfg(target_os = "linux")]
fn launch_ghostty_linux(command: &str, cwd: Option<&str>, shell: &str) -> Result<(), String> {
    std::process::Command::new("ghostty")
        .args(ghostty_launch_args(command, cwd, shell))
        .spawn()
        .map_err(|e| format!("Failed to launch ghostty: {e}"))?;

    Ok(())
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
fn launch_terminal_windows(
    command: &str,
    cwd: Option<&str>,
    preferred: Option<&str>,
) -> Result<(), String> {
    let powershell_cmd = powershell_resume_command(command, cwd);
    let cmd_cmd = cmd_resume_command(command, cwd);

    let candidates: &[(&str, &[&str])] = &[
        ("pwsh.exe", &["-NoExit", "-Command", &powershell_cmd]),
        ("powershell.exe", &["-NoExit", "-Command", &powershell_cmd]),
    ];

    // If user picked a specific shell, try it first
    if let Some(name) = preferred {
        if !name.is_empty() {
            if name == "wt.exe" && where_exists("wt.exe") {
                return launch_windows_terminal(&powershell_cmd, cwd);
            }
            if name == "cmd.exe" {
                return std::process::Command::new("cmd")
                    .args(["/C", "start", "", "cmd", "/K", &cmd_cmd])
                    .spawn()
                    .map_err(|e| format!("Failed to launch cmd: {e}"));
            }
            for (bin, args) in candidates {
                if *bin == name && where_exists(bin) {
                    let mut start_args = vec!["/C", "start", "", bin];
                    for a in args {
                        start_args.push(a.as_str());
                    }
                    return std::process::Command::new("cmd")
                        .args(&start_args)
                        .spawn()
                        .map_err(|e| format!("Failed to launch {}: {e}", bin));
                }
            }
        }
    }

    // Fallback: priority-based discovery
    for (bin, args) in candidates {
        if where_exists(bin) {
            let mut start_args = vec!["/C", "start", "", bin];
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
        return launch_windows_terminal(&powershell_cmd, cwd);
    }

    // Last resort: cmd
    std::process::Command::new("cmd")
        .args(["/C", "start", "", "cmd", "/K", &cmd_cmd])
        .spawn()
        .map_err(|e| format!("Failed to launch cmd: {e}"))
}

#[cfg(target_os = "windows")]
fn powershell_resume_command(command: &str, cwd: Option<&str>) -> String {
    cwd.map(|d| format!("Set-Location -LiteralPath {}; {}", powershell_quote(d), command))
        .unwrap_or_else(|| command.to_string())
}

#[cfg(target_os = "windows")]
fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn cmd_resume_command(command: &str, cwd: Option<&str>) -> String {
    cwd.map(|d| format!("cd /d {} && {}", shell_quote(d), command))
        .unwrap_or_else(|| command.to_string())
}

#[cfg(target_os = "windows")]
fn launch_windows_terminal(command: &str, cwd: Option<&str>) -> Result<(), String> {
    let mut wt_args = vec!["-d"];
    if let Some(d) = cwd {
        wt_args.push(d);
    } else {
        wt_args.push(".");
    }
    wt_args.push("pwsh.exe");
    wt_args.push("-NoExit");
    wt_args.push("-Command");
    wt_args.push(command);
    std::process::Command::new("wt")
        .args(&wt_args)
        .spawn()
        .map_err(|e| format!("Failed to launch Windows Terminal: {e}"))?;

    Ok(())
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

/// Soft-delete one or more sessions.
///
/// Marks sessions as deleted without removing data. Reversible via DB restore.
pub(crate) fn do_soft_delete_sessions(
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
    Ok(ActionResult { success: true, affected_count: ids.len() as i64 })
}

pub(crate) fn do_soft_delete_project(
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
    Ok(ActionResult { success: true, affected_count: count })
}

// --- Subagent Messages ---

pub(crate) fn do_subagent_messages(
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

pub(crate) fn do_action_log(
    state: &AppState,
    limit: Option<i64>,
) -> Result<ActionLogResponse, AppError> {
    let db = state.db()?;
    let actions = action_store::get_recent_actions(&db, limit.unwrap_or(50))?;
    Ok(ActionLogResponse { actions })
}

// --- Delete Planning ---

pub(crate) fn do_delete_plan(
    state: &AppState,
    session_id: String,
) -> Result<crate::service::delete_planner::DeletePlan, AppError> {
    let db = state.db()?;
    let plan = crate::service::delete_planner::resolve_delete_plan(&db, &session_id)?;
    Ok(plan)
}

pub(crate) fn do_destructive_delete(
    state: &AppState,
    session_id: String,
) -> Result<crate::service::delete_planner::DestructiveDeleteResult, AppError> {
    let db = state.db()?;
    let result = crate::service::delete_planner::execute_destructive_delete(&db, &session_id)?;
    Ok(result)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteJobPayload {
    pub job_id: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteJobStatus {
    pub job_id: String,
    pub processed: i64,
    pub total: i64,
    pub status: String,
}

pub(crate) fn do_get_delete_job(
    state: &AppState,
    job_id: &str,
) -> Result<DeleteJobStatus, AppError> {
    let db = state.db()?;
    let result: (i64, i64, String) = db
        .query_row(
            "SELECT current_index, total_count, status FROM delete_queue WHERE id = ?1",
            rusqlite::params![job_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| AppError::Internal(format!("Delete job {} not found", job_id)))?;
    Ok(DeleteJobStatus {
        job_id: job_id.to_string(),
        processed: result.0,
        total: result.1,
        status: result.2,
    })
}

pub(crate) fn do_destructive_delete_batch(
    state: &AppState,
    ids: Vec<String>,
) -> Result<DeleteJobPayload, AppError> {
    tracing::info!("do_destructive_delete_batch: {} ids", ids.len());
    let job_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let ids_json =
        serde_json::to_string(&ids).map_err(|e| AppError::Internal(format!("json: {}", e)))?;
    let total = ids.len() as i64;

    // Insert checkpoint record
    {
        let db = state.db()?;
        db.execute(
            "INSERT INTO delete_queue (id, session_ids, current_index, total_count, status, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, 'running', ?4, ?4)",
            rusqlite::params![job_id, ids_json, total, now],
        )?;
        action_store::record_action(
            &db,
            None,
            "destructive_delete_batch",
            Some(&format!("job {}: {} sessions queued", job_id, ids.len())),
        )?;
    }

    // Spawn background worker
    let emitter = state.event_emitter.clone();
    let db_path = state.db_path.clone();
    let jid = job_id.clone();

    std::thread::spawn(move || {
        tracing::info!("Delete job {}: starting background worker for {} sessions", jid, ids.len());
        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                emitter.emit_delete_progress(crate::app::events::DeleteProgressPayload {
                    processed: 0,
                    total,
                    current_session_id: String::new(),
                    status: "failed".into(),
                    deleted_files: 0,
                    failed_files: 0,
                });
                tracing::error!("Delete job {}: failed to open DB: {}", jid, e);
                return;
            },
        };
        let _ = crate::store::schema::configure_connection(&conn);

        let mut total_deleted = 0i64;
        let mut total_failed = 0i64;

        for (i, sid) in ids.iter().enumerate() {
            let processed = (i + 1) as i64;
            match crate::service::delete_planner::execute_destructive_delete(&conn, sid) {
                Ok(r) => {
                    total_deleted += r.deleted_files;
                    total_failed += r.failed_files;
                },
                Err(e) => {
                    total_failed += 1;
                    tracing::error!("Delete job {}: session {} failed: {}", jid, sid, e);
                },
            }

            // Update checkpoint
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let _ = conn.execute(
                "UPDATE delete_queue SET current_index = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![processed, now, jid],
            );

            // Emit progress
            tracing::info!(
                "Delete job {}: session {}/{} ({}) done, deleted={}, failed={}",
                jid,
                processed,
                total,
                sid,
                total_deleted,
                total_failed
            );
            emitter.emit_delete_progress(crate::app::events::DeleteProgressPayload {
                processed,
                total,
                current_session_id: sid.clone(),
                status: "running".into(),
                deleted_files: total_deleted,
                failed_files: total_failed,
            });
        }

        // Mark completed
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let _ = conn.execute(
            "UPDATE delete_queue SET status = 'completed', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, jid],
        );

        let _ = crate::store::actions::record_action(
            &conn,
            None,
            "destructive_delete_batch_completed",
            Some(&format!(
                "job {}: {} sessions, deleted={}, failed={}",
                jid,
                ids.len(),
                total_deleted,
                total_failed
            )),
        );

        emitter.emit_delete_progress(crate::app::events::DeleteProgressPayload {
            processed: total,
            total,
            current_session_id: String::new(),
            status: "completed".into(),
            deleted_files: total_deleted,
            failed_files: total_failed,
        });
        tracing::info!(
            "Delete job {}: completed, deleted={}, failed={}",
            jid,
            total_deleted,
            total_failed
        );
    });

    Ok(DeleteJobPayload { job_id })
}

/// Resume any incomplete delete jobs from a previous session.
pub(crate) fn resume_pending_delete_jobs(state: &AppState) {
    let emitter = state.event_emitter.clone();
    let db_path = state.db_path.clone();

    // Check for pending jobs
    let jobs: Vec<(String, String, i64, i64)> = {
        let db = match state.db() {
            Ok(d) => d,
            Err(_) => return,
        };
        let mut stmt = match db.prepare(
            "SELECT id, session_ids, current_index, total_count FROM delete_queue WHERE status = 'running'"
        ) { Ok(s) => s, Err(_) => return };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        });
        match rows {
            Ok(r) => r.filter_map(|r| r.ok()).collect(),
            Err(_) => return,
        }
    };

    for (job_id, ids_json, current_index, total_count) in jobs {
        let ids: Vec<String> = match serde_json::from_str(&ids_json) {
            Ok(ids) => ids,
            Err(_) => continue,
        };
        let remaining: Vec<String> = ids.into_iter().skip(current_index as usize).collect();
        if remaining.is_empty() {
            let _ = state.db().and_then(|db| {
                db.execute(
                    "UPDATE delete_queue SET status = 'completed', updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![
                        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        job_id
                    ],
                )
                .map_err(|e| AppError::DbError(e.to_string()))
            });
            continue;
        }

        let emitter = emitter.clone();
        let db_path = db_path.clone();
        let total = total_count;
        let start_index = current_index;

        tracing::info!("Resuming delete job {}: {} remaining", job_id, remaining.len());

        std::thread::spawn(move || {
            let conn = match rusqlite::Connection::open(&db_path) {
                Ok(c) => c,
                Err(_) => return,
            };
            let _ = crate::store::schema::configure_connection(&conn);

            let mut total_deleted = 0i64;
            let mut total_failed = 0i64;

            for (i, sid) in remaining.iter().enumerate() {
                let processed = start_index + (i + 1) as i64;
                match crate::service::delete_planner::execute_destructive_delete(&conn, sid) {
                    Ok(r) => {
                        total_deleted += r.deleted_files;
                        total_failed += r.failed_files;
                    },
                    Err(e) => {
                        total_failed += 1;
                        tracing::error!("Delete job {}: session {} failed: {}", job_id, sid, e);
                    },
                }

                let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                let _ = conn.execute(
                    "UPDATE delete_queue SET current_index = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![processed, now, job_id],
                );

                emitter.emit_delete_progress(crate::app::events::DeleteProgressPayload {
                    processed,
                    total,
                    current_session_id: sid.clone(),
                    status: "running".into(),
                    deleted_files: total_deleted,
                    failed_files: total_failed,
                });
            }

            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let _ = conn.execute(
                "UPDATE delete_queue SET status = 'completed', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, job_id],
            );

            emitter.emit_delete_progress(crate::app::events::DeleteProgressPayload {
                processed: total,
                total,
                current_session_id: String::new(),
                status: "completed".into(),
                deleted_files: total_deleted,
                failed_files: total_failed,
            });
        });
    }
}

// --- Rescan ---

pub(crate) fn do_rescan_sources(state: &AppState) -> Result<ActionResult, AppError> {
    let emitter = state.event_emitter.clone();
    let db_path = state.db_path.clone();
    let scan_guard = state.scan_guard.clone();

    let started = crate::sync::background::spawn_background_scan(db_path, emitter, scan_guard);

    if !started {
        return Err(AppError::Internal("Scan already in progress".to_string()));
    }

    Ok(ActionResult {
        success: true,
        affected_count: 0, // actual count arrives via sync-completed event
    })
}

// --- Release & Resync ---

pub(crate) fn do_release_and_resync(state: &AppState) -> Result<ActionResult, AppError> {
    // 1. Clear all indexed data (keep schema and action_log for audit)
    {
        let db = state.db()?;
        db.execute_batch(
            "DELETE FROM messages_fts;
             DELETE FROM messages;
             DELETE FROM session_sources;
             DELETE FROM sources;
             DELETE FROM sessions;
             DELETE FROM sqlite_sequence;
             DELETE FROM delete_queue;",
        )?;
        action_store::record_action(&db, None, "release", Some("Cleared all indexed data"))?;
    }

    // 2. Trigger full background rescan
    let emitter = state.event_emitter.clone();
    let db_path = state.db_path.clone();
    let scan_guard = state.scan_guard.clone();

    let started = crate::sync::background::spawn_background_scan(db_path, emitter, scan_guard);

    if !started {
        return Err(AppError::Internal("Scan already in progress".to_string()));
    }

    Ok(ActionResult { success: true, affected_count: 0 })
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

const MARKETPLACE_STALE_AFTER_HOURS: i64 = 2;

fn marketplace_sync_status(
    local_head: Option<&str>,
    remote_head: Option<&str>,
    last_checked_at: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    if last_checked_at.is_none() {
        return "never_checked".into();
    }

    if let Some(last_checked_at) = last_checked_at {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(last_checked_at) {
            let age = now.signed_duration_since(parsed.with_timezone(&chrono::Utc));
            if age > chrono::Duration::hours(MARKETPLACE_STALE_AFTER_HOURS) {
                return "stale".into();
            }
        }
    }

    match (local_head, remote_head) {
        (Some(local), Some(remote))
            if !local.is_empty() && !remote.is_empty() && local != remote =>
        {
            "update_available".into()
        },
        (Some(_), Some(_)) => "current".into(),
        (_, Some(_)) => "remote_known".into(),
        _ => "unknown".into(),
    }
}

fn installed_plugin_commit(entry: &serde_json::Value) -> Option<&str> {
    entry
        .get("gitCommitSha")
        .and_then(|v| v.as_str())
        .or_else(|| entry.get("version").and_then(|v| v.as_str()))
}

fn count_updates_available_for_marketplace(
    registry: &serde_json::Value,
    marketplace_name: &str,
    remote_head: Option<&str>,
) -> usize {
    let Some(remote_head) = remote_head.filter(|h| !h.is_empty()) else {
        return 0;
    };

    registry
        .get("plugins")
        .and_then(|v| v.as_object())
        .map(|plugins| {
            plugins
                .iter()
                .filter(|(key, _)| key.split('@').next_back() == Some(marketplace_name))
                .filter(|(_, entries)| {
                    entries
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(installed_plugin_commit)
                        .map(|commit| commit != remote_head)
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn marketplace_updates_available(
    registry: &serde_json::Value,
    marketplace_name: &str,
    remote_head: Option<&str>,
    sync_status: &str,
) -> usize {
    if sync_status != "update_available" {
        return 0;
    }
    count_updates_available_for_marketplace(registry, marketplace_name, remote_head)
}

fn git_output(args: &[&str], cwd: Option<&std::path::Path>) -> Result<String, AppError> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let out = cmd
        .output()
        .map_err(|e| AppError::Internal(format!("git {} failed: {}", args.join(" "), e)))?;
    if !out.status.success() {
        return Err(AppError::Internal(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_head(cwd: &std::path::Path, rev: &str) -> Option<String> {
    git_output(&["rev-parse", "--short", rev], Some(cwd)).ok().filter(|s| !s.is_empty())
}

fn marketplace_clone_url(repo: &str) -> String {
    let repo = repo.trim();
    if repo.starts_with("http://")
        || repo.starts_with("https://")
        || repo.starts_with("git@")
        || repo.starts_with("ssh://")
    {
        repo.to_string()
    } else {
        format!("https://github.com/{}.git", repo.trim_end_matches(".git"))
    }
}

fn short_git_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

fn git_remote_head_from_repo(repo: &str) -> Result<String, AppError> {
    let url = marketplace_clone_url(repo);
    let output = git_output(&["ls-remote", &url, "HEAD"], None)?;
    let sha = output.split_whitespace().next().ok_or_else(|| {
        AppError::Internal(format!("Remote marketplace {} did not return HEAD", repo))
    })?;
    Ok(short_git_sha(sha))
}

fn ensure_marketplace_clone(val: &serde_json::Value) -> Result<(), AppError> {
    let repo = val["source"]["repo"]
        .as_str()
        .ok_or_else(|| AppError::Validation("Marketplace source repo is missing".into()))?;
    let install_location = val["installLocation"]
        .as_str()
        .ok_or_else(|| AppError::Validation("Marketplace installLocation is missing".into()))?;
    let clone_dir = std::path::Path::new(install_location);

    if clone_dir.exists() && clone_dir.join(".git").exists() {
        return Ok(());
    }

    if clone_dir.exists() {
        std::fs::remove_dir_all(clone_dir).map_err(|e| {
            AppError::Internal(format!(
                "Failed to remove broken marketplace clone {}: {}",
                clone_dir.display(),
                e
            ))
        })?;
    }

    if let Some(parent) = clone_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::Internal(format!(
                "Failed to create marketplace parent {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    let clone_url = marketplace_clone_url(repo);
    let out = std::process::Command::new("git")
        .args(["clone", &clone_url, &clone_dir.to_string_lossy()])
        .output()
        .map_err(|e| AppError::Internal(format!("git clone failed: {}", e)))?;
    if !out.status.success() {
        return Err(AppError::Internal(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    Ok(())
}

fn git_remote_head(cwd: &std::path::Path) -> Option<String> {
    if let Ok(remote_ref) =
        git_output(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], Some(cwd))
    {
        if let Some(head) = git_head(cwd, &remote_ref) {
            return Some(head);
        }
    }
    git_head(cwd, "origin/main").or_else(|| git_head(cwd, "origin/master"))
}

fn count_available_marketplace_plugins(clone_path: &std::path::Path) -> usize {
    let plugins_dir = clone_path.join("plugins");
    let skills_dir = clone_path.join("skills");

    if plugins_dir.is_dir() {
        return std::fs::read_dir(plugins_dir)
            .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
            .unwrap_or(0);
    }

    if skills_dir.is_dir() {
        return std::fs::read_dir(skills_dir)
            .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
            .unwrap_or(0);
    }

    std::fs::read_dir(clone_path)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    let path = e.path();
                    path.is_dir() && path.join("SKILL.md").exists()
                })
                .count()
        })
        .unwrap_or(0)
}

fn scan_skills(plugin_path: &std::path::Path) -> Vec<plugin::SkillInfo> {
    let mut skills = Vec::new();
    let root_skill = plugin_path.join("SKILL.md");
    if root_skill.exists() {
        if let Some(info) = parse_frontmatter(&root_skill, "skill") {
            skills.push(info);
        }
    }

    let skills_dir = plugin_path.join("skills");
    if !skills_dir.exists() {
        return skills;
    }
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
    let hooks_dir = plugin_path.join("hooks/session-start");
    hooks_file.exists() || hooks_dir.is_dir()
}

struct PluginContentInventory {
    skills: Vec<plugin::SkillInfo>,
    agents: Vec<plugin::SkillInfo>,
    health: &'static str,
    issues: Vec<String>,
}

fn has_plugin_manifest(plugin_path: &std::path::Path) -> bool {
    plugin_path.join(".claude-plugin/plugin.json").exists()
        || plugin_path.join(".codex-plugin/plugin.json").exists()
}

fn inspect_plugin_content(plugin_path: &std::path::Path) -> PluginContentInventory {
    let has_manifest = has_plugin_manifest(plugin_path);
    let skills = scan_skills(plugin_path);
    let agents = scan_agents(plugin_path);
    let has_hooks = has_hooks(plugin_path);
    let mut issues = Vec::new();

    let health = if has_manifest {
        "ok"
    } else if !skills.is_empty() || !agents.is_empty() {
        issues.push("Missing plugin.json".into());
        "partial"
    } else if has_hooks {
        issues.push("Hook-only plugin, no skills or agents".into());
        "hook"
    } else {
        issues.push("No plugin, skill, agent, or hook content found".into());
        "unsupported"
    };

    PluginContentInventory { skills, agents, health, issues }
}

/// Detect which agents a plugin directory targets based on its manifests and content.
fn detect_agent_targets(path: &std::path::Path) -> Vec<String> {
    let mut targets = Vec::new();
    if path.join(".claude-plugin/plugin.json").exists()
        || path.join("skills").is_dir()
        || path.join("SKILL.md").exists()
        || path.join("agents").is_dir()
    {
        targets.push("claude_code".into());
    }
    if path.join(".codex-plugin/plugin.json").exists() {
        targets.push("codex".into());
    }
    // SKILL.md is cross-compatible, so if there's a skill, all agents can use it
    if path.join("SKILL.md").exists() || (path.join("skills").is_dir() && !targets.is_empty()) {
        if !targets.contains(&"codex".into()) {
            targets.push("codex".into());
        }
        targets.push("opencode".into());
    }
    targets
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

/// List all installed plugins with health status.
///
/// Scans global and project-local plugin directories, reports broken/missing
/// installations and orphaned registry entries.
pub(crate) fn do_list_plugins(
    state: &AppState,
    scope: String,
) -> Result<plugin::SkillsOverview, AppError> {
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
            Some(plugin::MarketplaceInfo { name: mn.clone(), repo, last_updated: last_upd })
        });

        // Health check
        let path = std::path::Path::new(&install_path);
        let mut health_issues = Vec::new();

        let (skills, agents, health) = if !path.exists() {
            health_issues.push("Install path does not exist".into());
            (Vec::new(), Vec::new(), "broken")
        } else {
            let inventory = inspect_plugin_content(path);
            health_issues.extend(inventory.issues);
            (inventory.skills, inventory.agents, inventory.health)
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
            agent: "claude_code".into(),
        });
    }

    // Scan Codex plugins from ~/.codex/plugins/cache/
    let codex_dir = home.join(".codex/plugins/cache");
    if codex_dir.exists() {
        if let Ok(mkt_entries) = std::fs::read_dir(&codex_dir) {
            for mkt_entry in mkt_entries.flatten() {
                let mkt_path = mkt_entry.path();
                if !mkt_path.is_dir() {
                    continue;
                }
                let mkt_name = mkt_entry.file_name().to_string_lossy().into_owned();
                if let Ok(plugin_entries) = std::fs::read_dir(&mkt_path) {
                    for pl_entry in plugin_entries.flatten() {
                        let pl_path = pl_entry.path();
                        if !pl_path.is_dir() {
                            continue;
                        }
                        let pl_name = pl_entry.file_name().to_string_lossy().into_owned();
                        // Find the version directory (first subdirectory)
                        if let Ok(ver_entries) = std::fs::read_dir(&pl_path) {
                            if let Some(ver_entry) =
                                ver_entries.flatten().find(|e| e.path().is_dir())
                            {
                                let install_dir = ver_entry.path();
                                let inventory = inspect_plugin_content(&install_dir);
                                let health = inventory.health;
                                match health {
                                    "ok" => health_ok += 1,
                                    "partial" => health_partial += 1,
                                    "hook" => health_hook += 1,
                                    _ => health_broken += 1,
                                }
                                total_skills += inventory.skills.len();
                                total_agents += inventory.agents.len();
                                plugins.push(plugin::PluginInfo {
                                    key: format!("{}@{}", pl_name, mkt_name),
                                    name: pl_name,
                                    version: ver_entry.file_name().to_string_lossy().into_owned(),
                                    scope: "global".into(),
                                    marketplace: None,
                                    install_path: install_dir.to_string_lossy().into_owned(),
                                    enabled: true,
                                    health: health.into(),
                                    health_issues: inventory.issues,
                                    skills: inventory.skills,
                                    agents: inventory.agents,
                                    installed_at: None,
                                    last_updated: None,
                                    agent: "codex".into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Scan OpenCode skills from ~/.config/opencode/skills/
    let opencode_skills_dir =
        dirs::data_local_dir().unwrap_or_else(|| home.join(".local/share")).join("opencode/skills");
    if opencode_skills_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&opencode_skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let skill_md = path.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }
                let desc = parse_frontmatter(&skill_md, "skill")
                    .map(|s| s.description)
                    .unwrap_or_else(|| name.clone());
                total_skills += 1;
                health_ok += 1;
                plugins.push(plugin::PluginInfo {
                    key: format!("{}@opencode", name),
                    name: name.clone(),
                    version: String::new(),
                    scope: "global".into(),
                    marketplace: None,
                    install_path: path.to_string_lossy().into_owned(),
                    enabled: true,
                    health: "ok".into(),
                    health_issues: Vec::new(),
                    skills: vec![plugin::SkillInfo {
                        name,
                        description: desc,
                        skill_type: "skill".into(),
                        tools: None,
                        file_path: skill_md.to_string_lossy().into_owned(),
                        health: "ok".into(),
                        health_detail: None,
                    }],
                    agents: Vec::new(),
                    installed_at: None,
                    last_updated: None,
                    agent: "opencode".into(),
                });
            }
        }
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
    let mut stmt = db
        .prepare("SELECT DISTINCT project_path FROM sessions WHERE project_path IS NOT NULL")
        .map_err(|e| AppError::DbError(e.to_string()))?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))
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

        let project_name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

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
            agent: "claude_code".into(),
        });
    }

    Ok(plugin::SkillsOverview {
        total_plugins: plugins.len(),
        total_skills,
        total_agents,
        health_summary: plugin::HealthSummary { ok: plugins.len(), partial: 0, hook: 0, broken: 0 },
        plugins,
    })
}

// --- Plugin Toggle & Uninstall ---

pub(crate) fn do_toggle_plugin(key: String) -> Result<(), AppError> {
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

pub(crate) fn do_uninstall_plugin(key: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // Parse key: "pluginName@marketplace"
    let parts: Vec<&str> = key.splitn(2, '@').collect();
    let plugin_name = parts.first().map(|s| *s).unwrap_or("");

    // 1. Uninstall from Claude Code registry
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    if registry_path.exists() {
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

        if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
            plugins.remove(&key);
        }
        let output = serde_json::to_string_pretty(&registry)
            .map_err(|e| AppError::Internal(format!("Failed to serialize registry: {}", e)))?;
        std::fs::write(&registry_path, output)
            .map_err(|e| AppError::Internal(format!("Failed to write registry: {}", e)))?;

        let settings_path = claude_dir.join("settings.json");
        if let Ok(mut settings) = read_json(&settings_path) {
            if let Some(enabled) =
                settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut())
            {
                enabled.remove(&key);
            }
            if let Ok(output) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(&settings_path, output);
            }
        }
    }

    // 2. Uninstall from Codex (remove from plugins cache and config.toml)
    if let Some(market_name) = parts.get(1).map(|s| *s) {
        let codex_cache =
            home.join(format!(".codex/plugins/cache/{}/{}", market_name, plugin_name));
        if codex_cache.exists() {
            let _ = std::fs::remove_dir_all(&codex_cache);
        }
        // Remove from config.toml
        let config_path = home.join(".codex/config.toml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                let plugin_header = format!("[plugins.\"{}\"]", key);
                let updated = remove_toml_section(&content, &plugin_header);
                if updated.len() != content.len() {
                    let _ = std::fs::write(&config_path, updated);
                }
            }
        }
    }

    // 3. Uninstall from OpenCode skills
    let opencode_skill = dirs::data_local_dir()
        .unwrap_or_else(|| home.join(".local/share"))
        .join(format!("opencode/skills/{}", plugin_name));
    if opencode_skill.exists() {
        let _ = std::fs::remove_dir_all(&opencode_skill);
    }

    Ok(())
}

/// Remove a TOML section (header line + following key=value lines until next section).
fn remove_toml_section(content: &str, section_header: &str) -> String {
    let lines: Vec<String> = content.lines().map(String::from).collect();
    let mut result = Vec::new();
    let mut skipping = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = false;
        }
        if trimmed == section_header {
            skipping = true;
            continue;
        }
        if !skipping {
            result.push(line.clone());
        }
    }

    result.join("\n")
}

// --- Plugin Fix: Clean & Reinstall ---

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dst).map_err(|e| {
        AppError::Internal(format!("Failed to create dir {}: {}", dst.display(), e))
    })?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| AppError::Internal(format!("Failed to read dir {}: {}", src.display(), e)))?
    {
        let entry = entry.map_err(|e| AppError::Internal(format!("Dir entry error: {}", e)))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        // Preserve symlinks
        if src_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            if let Ok(link_target) = std::fs::read_link(&src_path) {
                let _ = std::os::unix::fs::symlink(&link_target, &dst_path);
            }
            continue;
        }
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| {
                AppError::Internal(format!("Failed to copy {}: {}", src_path.display(), e))
            })?;
        }
    }
    Ok(())
}

/// Remove orphaned registry entry for a broken plugin (install dir missing/empty).
pub(crate) fn do_clean_plugin(key: String) -> Result<plugin::FixPluginResult, AppError> {
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
pub(crate) fn do_reinstall_plugin(key: String) -> Result<plugin::FixPluginResult, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Parse key: "pluginName@marketplace"
    let parts: Vec<&str> = key.splitn(2, '@').collect();
    let plugin_name =
        parts.first().ok_or_else(|| AppError::Validation("Invalid plugin key".into()))?;
    let market_name = parts.get(1).ok_or_else(|| {
        AppError::Validation(format!("Plugin key '{}' missing marketplace suffix", key))
    })?;

    // 2. Read marketplace metadata
    let marketplaces_path = claude_dir.join("plugins/known_marketplaces.json");
    let marketplaces: serde_json::Value = read_json(&marketplaces_path)?;
    let mkt = marketplaces
        .get(market_name)
        .ok_or_else(|| AppError::NotFound(format!("Marketplace '{}' not found", market_name)))?;
    let repo = mkt["source"]["repo"]
        .as_str()
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
    let install_path_str = entry["installPath"]
        .as_str()
        .ok_or_else(|| AppError::NotFound(format!("No installPath for '{}'", key)))?;
    let git_sha = entry["gitCommitSha"].as_str().unwrap_or("");

    // 4. Ensure marketplace clone is available and up-to-date
    let clone_dir_exists = clone_path.exists();
    let has_git = clone_path.join(".git").exists();
    if clone_dir_exists && has_git {
        // Fetch + pull latest so we get the newest version
        let _ = std::process::Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(clone_path)
            .output();
        let _ = std::process::Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(clone_path)
            .output();
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
                "git clone failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
    }

    // 5. Get latest HEAD SHA
    let new_sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(clone_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // 5. Find plugin source directory in marketplace
    let clone_path_buf = clone_path.to_path_buf();
    let candidates = [
        clone_path.join(format!("plugins/{}", plugin_name)),
        clone_path.join(format!("skills/{}", plugin_name)),
        clone_path.join(format!("agents/{}", plugin_name)),
    ];
    let source_dir = candidates
        .iter()
        .find(|p| p.exists() && p.is_dir())
        .or_else(|| {
            // Single-plugin repo: use clone root if it has plugin-like content
            let has_skill =
                clone_path.join("skills").is_dir() || clone_path.join("SKILL.md").exists();
            let has_plugin_json = clone_path.join(".claude-plugin/plugin.json").exists();
            if has_skill || has_plugin_json {
                Some(&clone_path_buf)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Could not find plugin '{}' in marketplace '{}'",
                plugin_name, market_name
            ))
        })?;

    // 7. Detect agent targets and reinstall to each
    let targets = detect_agent_targets(source_dir);

    // Claude Code: reinstall with updated registry
    if targets.contains(&"claude_code".to_string()) {
        let old_install_path = std::path::Path::new(install_path_str);
        if old_install_path.exists() {
            let _ = std::fs::remove_dir_all(old_install_path);
        }
        let new_install_path =
            claude_dir.join(format!("plugins/cache/{}/{}/{}", market_name, plugin_name, new_sha));
        if let Some(parent) = new_install_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("Failed to create cache dir: {}", e)))?;
        }
        copy_dir_recursive(source_dir, &new_install_path)?;

        let mut registry: serde_json::Value = read_json(&registry_path)?;
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
            plugins.insert(
                key.clone(),
                serde_json::json!([{
                    "scope": "user",
                    "installPath": new_install_path.to_string_lossy(),
                    "version": new_sha,
                    "installedAt": entry["installedAt"].clone(),
                    "lastUpdated": now,
                    "gitCommitSha": new_sha,
                }]),
            );
        }
        let output = serde_json::to_string_pretty(&registry)
            .map_err(|e| AppError::Internal(format!("Serialize: {}", e)))?;
        std::fs::write(&registry_path, output)
            .map_err(|e| AppError::Internal(format!("Write: {}", e)))?;
    }

    // Codex: reinstall to codex cache
    if targets.contains(&"codex".to_string()) {
        if let Err(e) = install_to_codex(&home, source_dir, market_name, plugin_name, &new_sha) {
            tracing::warn!("Codex reinstall failed for {}: {}", key, e);
        }
    }

    // OpenCode: reinstall to opencode skills
    if targets.contains(&"opencode".to_string()) {
        if let Err(e) = install_to_opencode(&home, source_dir, plugin_name) {
            tracing::warn!("OpenCode reinstall failed for {}: {}", key, e);
        }
    }

    Ok(plugin::FixPluginResult {
        action: "reinstall".into(),
        message: format!(
            "Reinstalled {} from {} ({} → {}) [{}]",
            key,
            repo,
            git_sha,
            new_sha,
            targets.join(",")
        ),
    })
}

// --- Marketplace Management ---

/// List configured plugin marketplaces.
///
/// Returns marketplace metadata including name, URL, and last update timestamp.
pub(crate) fn do_list_marketplaces() -> Result<plugin::MarketplaceListResult, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let marketplaces: serde_json::Value =
        read_json_or_default(&claude_dir.join("plugins/known_marketplaces.json"));
    let registry: serde_json::Value =
        read_json_or_default(&claude_dir.join("plugins/installed_plugins.json"));

    let mut entries = Vec::new();
    let now = chrono::Utc::now();
    if let Some(obj) = marketplaces.as_object() {
        for (name, val) in obj {
            let repo = val["source"]["repo"].as_str().unwrap_or("").to_string();
            let install_location = val["installLocation"].as_str().unwrap_or("").to_string();
            let last_updated = val["lastUpdated"].as_str().map(String::from);
            let last_checked_at = val["lastCheckedAt"].as_str().map(String::from);
            let local_head = val["localHead"]
                .as_str()
                .map(String::from)
                .or_else(|| git_head(std::path::Path::new(&install_location), "HEAD"));
            let remote_head = val["remoteHead"].as_str().map(String::from);
            let sync_status = val["syncStatus"].as_str().map(String::from).unwrap_or_else(|| {
                marketplace_sync_status(
                    local_head.as_deref(),
                    remote_head.as_deref(),
                    last_checked_at.as_deref(),
                    now,
                )
            });
            let check_error = val["lastCheckError"].as_str().map(String::from);
            let plugin_count =
                count_available_marketplace_plugins(std::path::Path::new(&install_location));
            let updates_available = marketplace_updates_available(
                &registry,
                name,
                remote_head.as_deref(),
                &sync_status,
            );

            entries.push(plugin::MarketplaceEntry {
                name: name.clone(),
                repo,
                install_location,
                last_updated,
                plugin_count,
                last_checked_at,
                local_head,
                remote_head,
                sync_status,
                check_error,
                updates_available,
            });
        }
    }

    Ok(plugin::MarketplaceListResult { marketplaces: entries })
}

fn refresh_marketplace_upstream_state(
    val: &mut serde_json::Value,
    checked_at: &str,
    now: chrono::DateTime<chrono::Utc>,
) {
    let repo = val["source"]["repo"].as_str().unwrap_or("").to_string();
    let install_location = val["installLocation"].as_str().unwrap_or("").to_string();
    let clone_dir = std::path::Path::new(&install_location);
    let remote_result = git_remote_head_from_repo(&repo);

    let fetch_result = if clone_dir.exists() && clone_dir.join(".git").exists() {
        std::process::Command::new("git")
            .args(["fetch", "origin", "--prune"])
            .current_dir(clone_dir)
            .output()
            .map_err(|e| AppError::Internal(format!("git fetch failed: {}", e)))
            .and_then(|out| {
                if out.status.success() {
                    Ok(())
                } else {
                    Err(AppError::Internal(format!(
                        "git fetch failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    )))
                }
            })
    } else {
        Ok(())
    };

    match (remote_result, fetch_result) {
        (Ok(remote_head_from_upstream), Ok(())) => {
            let has_local_clone = clone_dir.exists() && clone_dir.join(".git").exists();
            let local_head = has_local_clone.then(|| git_head(clone_dir, "HEAD")).flatten();
            let remote_head = git_remote_head(clone_dir).unwrap_or(remote_head_from_upstream);
            val["lastCheckedAt"] = serde_json::Value::String(checked_at.to_string());
            val["localHead"] =
                local_head.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null);
            val["remoteHead"] = serde_json::Value::String(remote_head.clone());
            val["syncStatus"] = serde_json::Value::String(if has_local_clone {
                marketplace_sync_status(
                    local_head.as_deref(),
                    Some(&remote_head),
                    Some(checked_at),
                    now,
                )
            } else {
                "clone_missing".into()
            });
            val["lastCheckError"] = if has_local_clone {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(
                    "Remote upstream is reachable, but local marketplace clone is missing".into(),
                )
            };
        },
        (Err(e), _) => {
            val["lastCheckedAt"] = serde_json::Value::String(checked_at.to_string());
            val["syncStatus"] = serde_json::Value::String("check_failed".into());
            val["lastCheckError"] = serde_json::Value::String(e.to_string());
        },
        (Ok(remote_head), Err(e)) => {
            val["lastCheckedAt"] = serde_json::Value::String(checked_at.to_string());
            val["remoteHead"] = serde_json::Value::String(remote_head);
            val["syncStatus"] = serde_json::Value::String("fetch_failed".into());
            val["lastCheckError"] = serde_json::Value::String(e.to_string());
        },
    }
}

pub(crate) fn do_add_marketplace(name: String, repo: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");
    let marketplaces_dir = claude_dir.join("plugins/marketplaces");
    std::fs::create_dir_all(&marketplaces_dir)
        .map_err(|e| AppError::Internal(format!("Failed to create marketplaces dir: {}", e)))?;

    // Clone repo
    let clone_url = marketplace_clone_url(&repo);
    let dest = marketplaces_dir.join(&name);
    let out = std::process::Command::new("git")
        .args(["clone", &clone_url, &dest.to_string_lossy()])
        .output()
        .map_err(|e| AppError::Internal(format!("git clone failed: {}", e)))?;
    if !out.status.success() {
        return Err(AppError::Internal(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    // Update known_marketplaces.json
    let path = claude_dir.join("plugins/known_marketplaces.json");
    let mut marketplaces: serde_json::Value = read_json_or_default(&path);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    marketplaces[&name] = serde_json::json!({
        "source": { "source": "github", "repo": repo },
        "installLocation": dest.to_string_lossy(),
        "lastUpdated": now,
        "lastCheckedAt": now,
        "localHead": git_head(&dest, "HEAD"),
        "remoteHead": git_remote_head(&dest),
        "syncStatus": "current",
        "lastCheckError": null
    });
    let output = serde_json::to_string_pretty(&marketplaces)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;
    std::fs::write(&path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write: {}", e)))?;

    Ok(())
}

pub(crate) fn do_update_marketplace(name: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let path = claude_dir.join("plugins/known_marketplaces.json");
    let mut marketplaces: serde_json::Value = read_json(&path)?;
    let mkt = marketplaces
        .get_mut(&name)
        .ok_or_else(|| AppError::NotFound(format!("Marketplace '{}' not found", name)))?;

    let now = chrono::Utc::now();
    let checked_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    mkt["lastUpdated"] = serde_json::Value::String(checked_at.clone());
    mkt["lastCheckedAt"] = serde_json::Value::String(checked_at.clone());
    match ensure_marketplace_clone(mkt) {
        Ok(()) => refresh_marketplace_upstream_state(mkt, &checked_at, now),
        Err(e) => {
            mkt["syncStatus"] = serde_json::Value::String("clone_failed".into());
            mkt["lastCheckError"] = serde_json::Value::String(e.to_string());
        },
    }

    let output = serde_json::to_string_pretty(&marketplaces)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {}", e)))?;
    std::fs::write(&path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write: {}", e)))?;

    Ok(())
}

pub(crate) fn do_remove_marketplace(name: String, remove_plugins: bool) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let path = claude_dir.join("plugins/known_marketplaces.json");
    let mut marketplaces: serde_json::Value = read_json(&path)?;
    let install_location = marketplaces
        .get(&name)
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
                        .filter(|k| k.split('@').next_back() == Some(&name))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
                for key in &keys_to_remove {
                    // Remove install dir
                    if let Some(entry) = plugins
                        .get(key)
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
            if let Some(enabled) =
                settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut())
            {
                let keys: Vec<String> = enabled
                    .keys()
                    .filter(|k| k.split('@').next_back() == Some(&name))
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

pub(crate) fn do_list_marketplace_plugins(
    marketplace_name: String,
) -> Result<Vec<plugin::MarketplacePlugin>, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let marketplaces: serde_json::Value =
        read_json(&claude_dir.join("plugins/known_marketplaces.json"))?;
    let clone_path_str = marketplaces[&marketplace_name]["installLocation"].as_str().unwrap_or("");
    let clone_path = std::path::Path::new(clone_path_str);
    if !clone_path.exists() {
        return Ok(Vec::new());
    }

    let registry: serde_json::Value =
        read_json_or_default(&claude_dir.join("plugins/installed_plugins.json"));
    let installed_names: std::collections::HashSet<String> = registry
        .get("plugins")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.keys()
                .filter(|k| k.split('@').next_back() == Some(marketplace_name.as_str()))
                .filter_map(|k| k.split('@').next())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut result = Vec::new();

    let plugins_dir = clone_path.join("plugins");
    let skills_dir = clone_path.join("skills");

    // Pattern 1: plugins/<name>/ (e.g., claude-plugins-official)
    if plugins_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let targets = detect_agent_targets(&path);
                if targets.is_empty() {
                    continue;
                }
                let skills = scan_skills(&path);
                let agents = scan_agents(&path);
                let desc = skills
                    .iter()
                    .chain(agents.iter())
                    .next()
                    .map(|s| s.description.clone())
                    .unwrap_or_else(|| name.clone());
                result.push(plugin::MarketplacePlugin {
                    installed: installed_names.contains(&name),
                    name,
                    description: desc,
                    skill_count: skills.len(),
                    agent_count: agents.len(),
                    has_hooks: has_hooks(&path),
                    agent_targets: targets,
                });
            }
        }
    }
    // Pattern 2: skills/<name>/ (e.g., anthropic-agent-skills)
    else if skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let targets = detect_agent_targets(&path);
                if targets.is_empty() {
                    continue;
                }
                let desc = path
                    .join("SKILL.md")
                    .exists()
                    .then(|| parse_frontmatter(&path.join("SKILL.md"), "skill"))
                    .flatten()
                    .map(|s| s.description)
                    .unwrap_or_else(|| name.clone());
                result.push(plugin::MarketplacePlugin {
                    installed: installed_names.contains(&name),
                    name,
                    description: desc,
                    skill_count: 1,
                    agent_count: 0,
                    has_hooks: false,
                    agent_targets: targets,
                });
            }
        }
    }
    // Pattern 3: root subdirectories with SKILL.md (e.g., axton-obsidian-visual-skills)
    else if let Ok(entries) = std::fs::read_dir(clone_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // Skip common non-plugin directories
            if ["node_modules", ".git", "dist", "src", "test", "tests", "__tests__"]
                .contains(&name.as_str())
            {
                continue;
            }
            if !path.join("SKILL.md").exists() {
                continue;
            }
            let targets = detect_agent_targets(&path);
            if targets.is_empty() {
                continue;
            }
            let desc = parse_frontmatter(&path.join("SKILL.md"), "skill")
                .map(|s| s.description)
                .unwrap_or_else(|| name.clone());
            result.push(plugin::MarketplacePlugin {
                installed: installed_names.contains(&name),
                name,
                description: desc,
                skill_count: 1,
                agent_count: 0,
                has_hooks: false,
                agent_targets: targets,
            });
        }
    }

    Ok(result)
}

pub(crate) fn do_install_marketplace_plugin(
    marketplace_name: String,
    plugin_name: String,
) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    let marketplaces: serde_json::Value =
        read_json(&claude_dir.join("plugins/known_marketplaces.json"))?;
    let clone_path_str = marketplaces[&marketplace_name]["installLocation"].as_str().unwrap_or("");
    let clone_path = std::path::Path::new(clone_path_str);

    let source = clone_path.join(format!("plugins/{}", plugin_name));
    let source = if source.is_dir() {
        source
    } else if clone_path.join(format!("skills/{}", plugin_name)).is_dir() {
        clone_path.join(format!("skills/{}", plugin_name))
    } else if clone_path.join(format!("agents/{}", plugin_name)).is_dir() {
        clone_path.join(format!("agents/{}", plugin_name))
    } else {
        clone_path.join(format!("plugins/{}", plugin_name))
    };
    if !source.is_dir() {
        return Err(AppError::NotFound(format!(
            "Plugin '{}' not found in '{}'",
            plugin_name, marketplace_name
        )));
    }

    let git_sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(clone_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    let targets = detect_agent_targets(&source);
    if targets.is_empty() {
        return Err(AppError::Validation(format!(
            "'{}' is not an installable plugin or skill",
            plugin_name
        )));
    }
    let mut errors = Vec::new();

    // Install to Claude Code
    if targets.contains(&"claude_code".to_string()) {
        if let Err(e) = install_to_claude(&home, &source, &marketplace_name, &plugin_name, &git_sha)
        {
            errors.push(format!("claude_code: {}", e));
        }
    }

    // Install to Codex
    if targets.contains(&"codex".to_string()) {
        if let Err(e) = install_to_codex(&home, &source, &marketplace_name, &plugin_name, &git_sha)
        {
            errors.push(format!("codex: {}", e));
        }
    }

    // Install to OpenCode
    if targets.contains(&"opencode".to_string()) {
        if let Err(e) = install_to_opencode(&home, &source, &plugin_name) {
            errors.push(format!("opencode: {}", e));
        }
    }

    if !errors.is_empty() {
        return Err(AppError::Internal(format!("Install partially failed: {}", errors.join("; "))));
    }

    Ok(())
}

fn install_to_claude(
    home: &std::path::Path,
    source: &std::path::Path,
    marketplace_name: &str,
    plugin_name: &str,
    git_sha: &str,
) -> Result<(), AppError> {
    let claude_dir = home.join(".claude");
    let install_path =
        claude_dir.join(format!("plugins/cache/{}/{}/{}", marketplace_name, plugin_name, git_sha));

    // Remove old install if exists
    if install_path.exists() {
        let _ = std::fs::remove_dir_all(&install_path);
    }
    std::fs::create_dir_all(&install_path)
        .map_err(|e| AppError::Internal(format!("Failed to create dir: {}", e)))?;
    copy_dir_recursive(source, &install_path)?;

    // Update registry
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let mut registry: serde_json::Value = read_json(&registry_path)?;
    let key = format!("{}@{}", plugin_name, marketplace_name);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let plugins = registry
        .get_mut("plugins")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| AppError::Internal("Invalid registry".into()))?;
    plugins.insert(
        key.clone(),
        serde_json::json!([{
            "scope": "user",
            "installPath": install_path.to_string_lossy(),
            "version": git_sha,
            "installedAt": now,
            "lastUpdated": now,
            "gitCommitSha": git_sha,
        }]),
    );

    let output = serde_json::to_string_pretty(&registry)
        .map_err(|e| AppError::Internal(format!("Serialize: {}", e)))?;
    std::fs::write(&registry_path, output)
        .map_err(|e| AppError::Internal(format!("Write: {}", e)))?;

    // Enable in settings.json
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

fn install_to_codex(
    home: &std::path::Path,
    source: &std::path::Path,
    marketplace_name: &str,
    plugin_name: &str,
    git_sha: &str,
) -> Result<(), AppError> {
    let codex_dir = home.join(".codex");
    let install_path =
        codex_dir.join(format!("plugins/cache/{}/{}/{}", marketplace_name, plugin_name, git_sha));

    // Remove old install if exists
    if install_path.exists() {
        let _ = std::fs::remove_dir_all(&install_path);
    }
    std::fs::create_dir_all(&install_path)
        .map_err(|e| AppError::Internal(format!("Failed to create dir: {}", e)))?;
    copy_dir_recursive(source, &install_path)?;

    // Register in config.toml: [plugins."plugin@marketplace"] enabled = true
    let config_path = codex_dir.join("config.toml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| AppError::Internal(format!("Read config.toml: {}", e)))?;
        let key = format!("{}@{}", plugin_name, marketplace_name);
        let plugin_header = format!("[plugins.\"{}\"]", key);

        if !content.contains(&plugin_header) {
            let mut updated = content;
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&format!("{}\nenabled = true\n", plugin_header));
            std::fs::write(&config_path, updated)
                .map_err(|e| AppError::Internal(format!("Write config.toml: {}", e)))?;
        }
    }

    Ok(())
}

fn install_to_opencode(
    home: &std::path::Path,
    source: &std::path::Path,
    plugin_name: &str,
) -> Result<(), AppError> {
    let opencode_skills_dir =
        dirs::data_local_dir().unwrap_or_else(|| home.join(".local/share")).join("opencode/skills");
    let install_path = opencode_skills_dir.join(plugin_name);

    // Remove old install if exists
    if install_path.exists() {
        let _ = std::fs::remove_dir_all(&install_path);
    }
    std::fs::create_dir_all(&install_path)
        .map_err(|e| AppError::Internal(format!("Failed to create dir: {}", e)))?;
    copy_dir_recursive(source, &install_path)?;

    Ok(())
}

// --- Proxy ---

use crate::app::proxy::{ProxyConfig, ProxyStatus};

pub(crate) fn do_proxy_status(state: &AppState) -> Result<ProxyStatus, AppError> {
    Ok(state.proxy_manager.status())
}

pub(crate) fn do_start_proxy(state: &AppState) -> Result<(), AppError> {
    state.proxy_manager.start()
}

pub(crate) fn do_stop_proxy(state: &AppState) -> Result<(), AppError> {
    state.proxy_manager.stop()
}

pub(crate) fn do_restart_proxy(state: &AppState) -> Result<(), AppError> {
    state.proxy_manager.restart()
}

pub(crate) fn do_get_proxy_config(state: &AppState) -> Result<ProxyConfig, AppError> {
    state.proxy_manager.read_config()
}

pub(crate) fn do_update_proxy_config(
    state: &AppState,
    config: ProxyConfig,
) -> Result<(), AppError> {
    state.proxy_manager.write_config(&config)
}

pub(crate) fn do_get_proxy_logs(state: &AppState, lines: usize) -> Result<String, AppError> {
    state.proxy_manager.get_logs(lines)
}

pub(crate) fn do_get_proxy_metrics(
    state: &AppState,
) -> Result<crate::app::proxy::ProxyMetrics, AppError> {
    state.proxy_manager.get_metrics()
}

pub(crate) fn do_get_proxy_error_events(
    state: &AppState,
) -> Result<Vec<crate::app::proxy::ProxyErrorEvent>, AppError> {
    state.proxy_manager.get_error_events()
}

#[cfg(test)]
mod plugin_freshness_tests {
    use super::*;

    #[test]
    fn marketplace_status_marks_stale_after_two_hours_without_successful_check() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let last_checked = Some("2026-05-18T09:59:59.000Z");

        assert_eq!(
            marketplace_sync_status(Some("abc123"), Some("abc123"), last_checked, now),
            "stale"
        );
    }

    #[test]
    fn marketplace_status_reports_remote_update_when_heads_differ() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let last_checked = Some("2026-05-18T11:30:00.000Z");

        assert_eq!(
            marketplace_sync_status(Some("local1"), Some("remote2"), last_checked, now),
            "update_available"
        );
    }

    #[test]
    fn counts_installed_plugins_behind_marketplace_head() {
        let registry = serde_json::json!({
            "plugins": {
                "fresh@official": [{ "gitCommitSha": "head123", "version": "head123" }],
                "stale@official": [{ "gitCommitSha": "old456", "version": "old456" }],
                "other@community": [{ "gitCommitSha": "old456", "version": "old456" }]
            }
        });

        assert_eq!(
            count_updates_available_for_marketplace(&registry, "official", Some("head123")),
            1
        );
    }

    #[test]
    fn clone_missing_marketplace_does_not_report_update_count() {
        let registry = serde_json::json!({
            "plugins": {
                "stale@official": [{ "gitCommitSha": "old456", "version": "old456" }]
            }
        });

        assert_eq!(
            marketplace_updates_available(&registry, "official", Some("head123"), "clone_missing"),
            0
        );
    }

    #[test]
    fn marketplace_clone_url_targets_github_for_owner_repo_sources() {
        assert_eq!(
            marketplace_clone_url("anthropics/claude-plugins-official"),
            "https://github.com/anthropics/claude-plugins-official.git"
        );
    }

    #[test]
    fn marketplace_clone_url_preserves_absolute_urls() {
        assert_eq!(
            marketplace_clone_url("https://github.com/openai/codex-plugin-cc.git"),
            "https://github.com/openai/codex-plugin-cc.git"
        );
    }

    #[test]
    fn readme_only_directory_has_no_agent_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "docs only").unwrap();

        assert!(detect_agent_targets(dir.path()).is_empty());
    }

    #[test]
    fn readme_only_directory_is_classified_as_unsupported_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "docs only").unwrap();

        let inventory = inspect_plugin_content(dir.path());

        assert_eq!(inventory.health, "unsupported");
        assert_eq!(inventory.skills.len(), 0);
        assert_eq!(inventory.agents.len(), 0);
        assert_eq!(inventory.issues, vec!["No plugin, skill, agent, or hook content found"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ghostty_macos_uses_shell_initial_command_with_working_directory() {
        let args = ghostty_launch_args_macos(
            "claude --resume 'a340bf99-5508-48be-b116-80783accc430'",
            Some("/Users/hipnusleo/Documents/Projects/apps/yeek"),
        );

        assert_eq!(
            args[0],
            "--working-directory=/Users/hipnusleo/Documents/Projects/apps/yeek"
        );
        assert_eq!(
            args[1],
            "--initial-command=shell:claude --resume 'a340bf99-5508-48be-b116-80783accc430'"
        );
        assert!(args.iter().all(|arg| !arg.starts_with("--command=")));
        assert!(args.iter().all(|arg| arg != "-e"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ghostty_macos_initial_command_does_not_wrap_exec() {
        let args = ghostty_launch_args_macos(
            "claude --resume 'a340bf99-5508-48be-b116-80783accc430'",
            Some("/Users/hipnusleo/Documents/Projects/apps/yeek test"),
        );

        assert_eq!(
            args[0],
            "--working-directory=/Users/hipnusleo/Documents/Projects/apps/yeek test"
        );
        assert_eq!(
            args[1],
            "--initial-command=shell:claude --resume 'a340bf99-5508-48be-b116-80783accc430'"
        );
        assert!(!args[1].contains("exec"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ghostty_linux_uses_fish_supported_flags() {
        let args = ghostty_launch_args(
            "codex resume 'a340bf99-5508-48be-b116-80783accc430'",
            None,
            "/opt/homebrew/bin/fish",
        );

        assert_eq!(
            args,
            vec![
                "-e",
                "/opt/homebrew/bin/fish",
                "-l",
                "-i",
                "-c",
                "codex resume 'a340bf99-5508-48be-b116-80783accc430'"
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ghostty_linux_falls_back_for_unknown_shells() {
        let args = ghostty_launch_args(
            "claude --resume 'a340bf99-5508-48be-b116-80783accc430'",
            None,
            "/bin/tcsh",
        );

        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "-e");
        assert_eq!(args[1], "/bin/sh");
        assert_eq!(args[2], "-c");
        assert!(args[3].contains("export PATH=\"$PATH:/opt/homebrew/bin:/usr/local/bin:"));
        assert!(args[3].contains("exec claude --resume"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_open_args_reuse_running_app_instance_for_standard_apps() {
        assert_eq!(macos_open_app_args("Warp"), vec!["-a", "Warp", "--args"]);
        assert!(!macos_open_app_args("Warp").contains(&"-n"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_open_new_app_args_force_new_instance_for_ghostty_argument_delivery() {
        assert_eq!(
            macos_open_new_app_args("Ghostty"),
            vec!["-n", "-a", "Ghostty", "--args"]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn iterm_applescript_reuses_running_app_with_new_tab() {
        let script = iterm_resume_applescript(
            "cd '/Users/hipnusleo/Documents/Projects/apps/yeek' && codex resume 'a340bf99-5508-48be-b116-80783accc430'",
        );

        assert!(script.contains(r#"tell application "iTerm""#));
        assert!(script.contains("if (count of windows) = 0 then"));
        assert!(script.contains("create window with default profile"));
        assert!(script.contains("create tab with default profile"));
        assert!(script.contains(r#"write text "cd '/Users/hipnusleo/Documents/Projects/apps/yeek' && codex resume 'a340bf99-5508-48be-b116-80783accc430'""#));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn non_ghostty_unix_terminals_share_shell_argument_selection() {
        let zsh_args = unix_shell_command_args("claude --resume 'session-id'", "/bin/zsh");
        assert_eq!(
            zsh_args,
            vec!["/bin/zsh", "-lic", "claude --resume 'session-id'"]
        );

        let fish_args = unix_shell_command_args("codex resume 'session-id'", "/opt/homebrew/bin/fish");
        assert_eq!(
            fish_args,
            vec!["/opt/homebrew/bin/fish", "-l", "-i", "-c", "codex resume 'session-id'"]
        );

        let fallback_args = unix_shell_command_args("claude --resume 'session-id'", "/bin/tcsh");
        assert_eq!(fallback_args[0], "/bin/sh");
        assert_eq!(fallback_args[1], "-c");
        assert!(fallback_args[2].contains("exec claude --resume"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_terminal_display_names_map_to_app_names() {
        assert_eq!(macos_terminal_app_name("iTerm2"), "iTerm");
        assert_eq!(macos_terminal_app_name("cmux"), "cmux");
        assert_eq!(macos_terminal_app_name("Terminal.app"), "Terminal.app");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_powershell_resume_command_uses_literal_path() {
        let command = r#"claude --resume "a340bf99-5508-48be-b116-80783accc430""#;
        let full_cmd = powershell_resume_command(command, Some(r#"C:\Users\leo's app\yeek"#));

        assert_eq!(
            full_cmd,
            r#"Set-Location -LiteralPath 'C:\Users\leo''s app\yeek'; claude --resume "a340bf99-5508-48be-b116-80783accc430""#
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_cmd_resume_command_uses_cmd_separator() {
        let command = r#"codex resume "a340bf99-5508-48be-b116-80783accc430""#;
        let full_cmd = cmd_resume_command(command, Some(r#"C:\Users\leo\yeek"#));

        assert_eq!(
            full_cmd,
            r#"cd /d "C:\Users\leo\yeek" && codex resume "a340bf99-5508-48be-b116-80783accc430""#
        );
    }
}
