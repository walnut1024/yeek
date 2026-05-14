use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::Value;

use crate::app::errors::AppError;
use crate::domain::session::{DeleteMode, SessionRecord, SessionStatus, VisibilityStatus};
use crate::domain::source::SourceDescriptor;
use crate::store::messages::MessageRecord;
use crate::store::sessions;

// ── Discovery ──

/// Resolve candidate OpenCode data directories.
fn opencode_data_dirs() -> Vec<PathBuf> {
    let mut dirs_list = Vec::new();

    // OPENCODE_DB override
    if let Ok(val) = std::env::var("OPENCODE_DB") {
        if Path::new(&val).is_absolute() {
            if let Some(parent) = Path::new(&val).parent() {
                dirs_list.push(parent.to_path_buf());
            }
        }
    }

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return dirs_list,
    };

    // macOS: ~/Library/Application Support/opencode
    dirs_list.push(home.join("Library/Application Support/opencode"));

    // XDG_DATA_HOME/opencode
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        dirs_list.push(PathBuf::from(xdg).join("opencode"));
    }

    // Fallback: ~/.local/share/opencode
    dirs_list.push(home.join(".local/share/opencode"));

    dirs_list
}

pub(crate) fn discover_sources() -> Result<Vec<SourceDescriptor>, AppError> {
    let mut sources = Vec::new();
    for data_dir in opencode_data_dirs() {
        if !data_dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(source) = source_descriptor_from_path(&path) {
                    sources.push(source);
                }
            }
        }
    }
    Ok(sources)
}

pub(crate) fn source_descriptor_from_path(path: &Path) -> Option<SourceDescriptor> {
    let file_name = path.file_name().and_then(|n| n.to_str())?;
    if file_name != "opencode.db"
        && !(file_name.starts_with("opencode-") && file_name.ends_with(".db"))
    {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }

    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            DateTime::from_timestamp(d.as_secs() as i64, 0)
                .unwrap_or_default()
                .to_rfc3339()
        })
        .unwrap_or_default();

    Some(SourceDescriptor {
        source_type: "opencode_db".to_string(),
        path: path.to_string_lossy().to_string(),
        agent: "opencode".to_string(),
        fingerprint: compute_fingerprint(&path),
        last_modified: modified,
    })
}

pub(crate) fn compute_fingerprint(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) => {
            let len = m.len();
            let modified = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("{}:{}", len, modified)
        }
        Err(_) => "unknown".to_string(),
    }
}

// ── Indexing ──

pub struct IndexResult {
    pub indexed: i64,
    pub updated: i64,
    pub errors: i64,
}

pub(crate) fn index_sources<F>(
    _conn: &rusqlite::Connection,
    sources: &[SourceDescriptor],
    _on_progress: F,
) -> Result<IndexResult, AppError>
where
    F: Fn(i64),
{
    // Placeholder — implemented in a later task
    Ok(IndexResult {
        indexed: 0,
        updated: 0,
        errors: sources.len() as i64,
    })
}

/// Open an OpenCode SQLite database in strict read-only mode.
fn open_opencode_db_readonly(
    path: &Path,
) -> Result<rusqlite::Connection, AppError> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| {
        AppError::DbError(format!(
            "Failed to open OpenCode DB at {}: {}",
            path.display(),
            e
        ))
    })?;

    conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|e| AppError::DbError(format!("Failed to set pragmas: {}", e)))?;

    Ok(conn)
}

/// Validate that the OpenCode DB has the minimum required schema.
fn validate_opencode_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
    let required_tables = ["session", "project", "message", "part"];
    let required_columns: &[(&str, &[&str])] = &[
        (
            "session",
            &[
                "id",
                "project_id",
                "parent_id",
                "directory",
                "title",
                "agent",
                "model",
                "time_created",
                "time_updated",
                "time_archived",
            ],
        ),
        ("project", &["id", "worktree"]),
        ("message", &["id", "session_id", "time_created", "data"]),
        ("part", &["id", "message_id", "session_id", "time_created", "data"]),
    ];

    for table in &required_tables {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                params![table],
                |row| row.get(0),
            )
            .map_err(|e| AppError::DbError(format!("Schema validation query failed: {}", e)))?;

        if count == 0 {
            return Err(AppError::Validation(format!(
                "OpenCode DB missing required table: {}",
                table
            )));
        }
    }

    for (table, cols) in required_columns {
        for col in *cols {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info(?) WHERE name=?",
                    params![table, col],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    AppError::DbError(format!("Column validation query failed: {}", e))
                })?;

            if count == 0 {
                return Err(AppError::Validation(format!(
                    "OpenCode DB table '{}' missing required column: {}",
                    table, col
                )));
            }
        }
    }

    Ok(())
}

// ── Session & Message Mapping ──

/// Convert Unix milliseconds to RFC3339 UTC string.
fn ms_to_rfc3339(ms: i64) -> Option<String> {
    DateTime::from_timestamp_millis(ms).map(|dt| dt.to_rfc3339())
}

/// Query all sessions from an OpenCode SQLite database.
pub(crate) fn query_sessions(
    oc_conn: &rusqlite::Connection,
) -> Result<Vec<SessionRecord>, AppError> {
    let mut stmt = oc_conn.prepare(
        "SELECT s.id, s.parent_id, s.directory, s.title, s.agent, s.model,
                s.time_created, s.time_updated, s.time_archived,
                p.worktree,
                (SELECT COUNT(*) FROM message m WHERE m.session_id = s.id) AS msg_count
         FROM session s
         LEFT JOIN project p ON s.project_id = p.id
         ORDER BY s.time_created ASC",
    )?;

    let sessions = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let parent_id: Option<String> = row.get(1)?;
            let directory: String = row.get(2)?;
            let title: Option<String> = row.get(3)?;
            let _agent: Option<String> = row.get(4)?;
            let model_json: Option<String> = row.get(5)?;
            let time_created: i64 = row.get(6)?;
            let time_updated: i64 = row.get(7)?;
            let time_archived: Option<i64> = row.get(8)?;
            let worktree: Option<String> = row.get(9)?;
            let msg_count: i64 = row.get(10)?;

            let project_path = worktree.or_else(|| {
                if directory.is_empty() {
                    None
                } else {
                    Some(directory)
                }
            });

            let model = model_json.and_then(|json| {
                let v: Value = serde_json::from_str(&json).ok()?;
                if let Some(id) = v.get("id").and_then(|v| v.as_str()) {
                    Some(id.to_string())
                } else {
                    let provider = v.get("providerID").and_then(|v| v.as_str()).unwrap_or("");
                    let model_id = v.get("modelID").and_then(|v| v.as_str()).unwrap_or("");
                    if provider.is_empty() && model_id.is_empty() {
                        None
                    } else if provider.is_empty() {
                        Some(model_id.to_string())
                    } else {
                        Some(format!("{}/{}", provider, model_id))
                    }
                }
            });

            let status = if time_archived.is_some() {
                SessionStatus::Complete
            } else {
                SessionStatus::Active
            };

            Ok(SessionRecord {
                id: format!("opencode:{}", id),
                agent: "opencode".to_string(),
                project_path,
                title,
                model,
                git_branch: None,
                started_at: ms_to_rfc3339(time_created),
                ended_at: ms_to_rfc3339(time_updated),
                status,
                visibility: VisibilityStatus::Visible,
                pinned: false,
                archived_at: None,
                deleted_at: None,
                delete_mode: DeleteMode::None,
                message_count: msg_count,
                updated_at: ms_to_rfc3339(time_updated).unwrap_or_default(),
                parent_session_id: parent_id.map(|pid| format!("opencode:{}", pid)),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sessions)
}

/// Safe UTF-8 character-boundary truncation.
fn truncate_to_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        s
    } else {
        let end = s
            .char_indices()
            .take(max)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        &s[..end]
    }
}

/// Query messages and parts for a session from an OpenCode SQLite database.
pub(crate) fn query_messages(
    oc_conn: &rusqlite::Connection,
    session_id: &str,
    is_sidechain: bool,
) -> Result<Vec<MessageRecord>, AppError> {
    // Load messages
    let mut msg_stmt = oc_conn.prepare(
        "SELECT id, time_created, data FROM message WHERE session_id = ? ORDER BY time_created ASC",
    )?;

    struct RawMessage {
        id: String,
        time_created: i64,
        data: Value,
    }

    let raw_messages: Vec<RawMessage> = msg_stmt
        .query_map(params![session_id], |row| {
            let id: String = row.get(0)?;
            let time_created: i64 = row.get(1)?;
            let data_str: String = row.get(2)?;
            let data: Value = serde_json::from_str(&data_str).unwrap_or(Value::Null);
            Ok(RawMessage {
                id,
                time_created,
                data,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Load parts
    let mut part_stmt = oc_conn.prepare(
        "SELECT id, message_id, time_created, data FROM part WHERE session_id = ? ORDER BY time_created ASC",
    )?;

    struct RawPart {
        id: String,
        message_id: String,
        time_created: i64,
        data: Value,
    }

    let raw_parts: Vec<RawPart> = part_stmt
        .query_map(params![session_id], |row| {
            let id: String = row.get(0)?;
            let message_id: String = row.get(1)?;
            let time_created: i64 = row.get(2)?;
            let data_str: String = row.get(3)?;
            let data: Value = serde_json::from_str(&data_str).unwrap_or(Value::Null);
            Ok(RawPart {
                id,
                message_id,
                time_created,
                data,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Group parts by message_id
    let mut parts_by_msg: std::collections::HashMap<&str, Vec<&RawPart>> =
        std::collections::HashMap::new();
    for part in &raw_parts {
        parts_by_msg
            .entry(&part.message_id)
            .or_default()
            .push(part);
    }

    let prefixed_session = format!("opencode:{}", session_id);
    let mut records = Vec::new();

    for msg in &raw_messages {
        let role_raw = msg
            .data
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let role = if role_raw == "user" {
            "human".to_string()
        } else {
            role_raw.to_string()
        };

        let parent_id = msg
            .data
            .get("parentID")
            .and_then(|v| v.as_str())
            .map(|pid| format!("opencode:{}", pid));

        let model = msg
            .data
            .get("modelID")
            .and_then(|v| v.as_str())
            .map(|m| m.to_string());

        let parts = parts_by_msg.get(msg.id.as_str()).cloned().unwrap_or_default();

        // Collect text parts for preview
        let text_parts: Vec<&str> = parts
            .iter()
            .filter(|p| {
                p.data
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|t| t == "text")
                    .unwrap_or(false)
            })
            .filter_map(|p| p.data.get("text").and_then(|v| v.as_str()))
            .collect();
        let preview = text_parts.join("\n");

        // Primary message record
        records.push(MessageRecord {
            id: format!("opencode:{}", msg.id),
            session_id: prefixed_session.clone(),
            parent_id,
            role: role.clone(),
            kind: "message".to_string(),
            content_preview: truncate_to_chars(&preview, 500).to_string(),
            timestamp: ms_to_rfc3339(msg.time_created),
            is_sidechain,
            entry_type: String::new(),
            subtype: None,
            tool_name: None,
            subagent_id: None,
            model,
            metadata: None,
        });

        // Extra records for special parts
        for part in &parts {
            let part_type = part
                .data
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let part_id = format!("opencode:{}:part:{}", msg.id, part.id);

            match part_type {
                "reasoning" => {
                    let thinking = part
                        .data
                        .get("thinking")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    records.push(MessageRecord {
                        id: part_id,
                        session_id: prefixed_session.clone(),
                        parent_id: None,
                        role: role.clone(),
                        kind: "thinking".to_string(),
                        content_preview: truncate_to_chars(thinking, 500).to_string(),
                        timestamp: ms_to_rfc3339(part.time_created),
                        is_sidechain,
                        entry_type: "reasoning".to_string(),
                        subtype: None,
                        tool_name: None,
                        subagent_id: None,
                        model: None,
                        metadata: None,
                    });
                }
                "tool" => {
                    let tool_name = part
                        .data
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let status_val = part
                        .data
                        .get("state")
                        .and_then(|s| s.get("status"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let (kind, preview_text) = match status_val {
                        "completed" => {
                            let output = part
                                .data
                                .get("state")
                                .and_then(|s| s.get("output"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            (
                                "tool_result".to_string(),
                                truncate_to_chars(output, 500).to_string(),
                            )
                        }
                        "error" => {
                            let error = part
                                .data
                                .get("state")
                                .and_then(|s| s.get("error"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("Tool error");
                            (
                                "tool_result".to_string(),
                                truncate_to_chars(error, 500).to_string(),
                            )
                        }
                        _ => {
                            let name = tool_name.as_deref().unwrap_or("unknown");
                            (
                                "tool_use".to_string(),
                                format!("[tool: {}]", name),
                            )
                        }
                    };

                    records.push(MessageRecord {
                        id: part_id,
                        session_id: prefixed_session.clone(),
                        parent_id: None,
                        role: role.clone(),
                        kind,
                        content_preview: preview_text,
                        timestamp: ms_to_rfc3339(part.time_created),
                        is_sidechain,
                        entry_type: String::new(),
                        subtype: None,
                        tool_name,
                        subagent_id: None,
                        model: None,
                        metadata: None,
                    });
                }
                _ => {
                    // Skip other part types (fold into metadata later if needed)
                }
            }
        }
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_descriptor_accepts_default_db_name() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode.db");
        std::fs::write(&db_path, b"fake sqlite").unwrap();
        let result = source_descriptor_from_path(&db_path);
        assert!(result.is_some());
        let desc = result.unwrap();
        assert_eq!(desc.source_type, "opencode_db");
        assert_eq!(desc.agent, "opencode");
    }

    #[test]
    fn source_descriptor_accepts_channel_db_name() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode-beta.db");
        std::fs::write(&db_path, b"fake sqlite").unwrap();
        let result = source_descriptor_from_path(&db_path);
        assert!(result.is_some());
    }

    #[test]
    fn source_descriptor_rejects_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("other.db");
        std::fs::write(&db_path, b"not opencode").unwrap();
        assert!(source_descriptor_from_path(&db_path).is_none());

        let jsonl_path = dir.path().join("session.jsonl");
        std::fs::write(&jsonl_path, b"{}").unwrap();
        assert!(source_descriptor_from_path(&jsonl_path).is_none());
    }

    #[test]
    fn source_descriptor_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("opencode.db");
        std::fs::create_dir(&subdir).unwrap();
        assert!(source_descriptor_from_path(&subdir).is_none());
    }

    #[test]
    fn compute_fingerprint_returns_size_colon_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::fs::write(&path, b"hello world").unwrap();
        let fp = compute_fingerprint(&path);
        assert!(fp.starts_with("11:"));
    }

    #[test]
    fn validate_schema_accepts_valid_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_id TEXT,
                directory TEXT NOT NULL, title TEXT NOT NULL, agent TEXT, model TEXT,
                time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, time_archived INTEGER
            );
            CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT);
            CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, data TEXT NOT NULL);
            CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, data TEXT NOT NULL);"
        ).unwrap();
        drop(conn);

        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        assert!(validate_opencode_schema(&oc_conn).is_ok());
    }

    #[test]
    fn validate_schema_rejects_missing_table() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE session (id TEXT PRIMARY KEY);")
            .unwrap();
        drop(conn);

        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        assert!(validate_opencode_schema(&oc_conn).is_err());
    }

    // ── Helpers ──

    fn create_test_opencode_db(dir: &tempfile::TempDir) -> PathBuf {
        let db_path = dir.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY, project_id TEXT NOT NULL, workspace_id TEXT, parent_id TEXT,
                slug TEXT NOT NULL DEFAULT '', directory TEXT NOT NULL, path TEXT, title TEXT NOT NULL,
                version TEXT NOT NULL DEFAULT '', share_url TEXT, summary_additions INTEGER, summary_deletions INTEGER,
                summary_files INTEGER, summary_diffs TEXT, revert TEXT, permission TEXT,
                agent TEXT, model TEXT, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
                time_compacting INTEGER, time_archived INTEGER
            );
            CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT);
            CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);
            CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);"
        ).unwrap();
        conn.execute(
            "INSERT INTO project (id, worktree) VALUES (?, ?)",
            params!["proj1", "/Users/test/myproject"],
        ).unwrap();
        conn.execute(
            "INSERT INTO session (id, project_id, directory, title, agent, model, time_created, time_updated, time_archived) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                "sess1", "proj1", "/Users/test/myproject", "Test Session", "code",
                r#"{"id":"claude-4-sonnet","providerID":"anthropic"}"#,
                1716200000000i64, 1716200060000i64, std::option::Option::<i64>::None,
            ],
        ).unwrap();
        conn.execute(
            "INSERT INTO session (id, project_id, parent_id, directory, title, agent, model, time_created, time_updated, time_archived) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                "child1", "proj1", "sess1", "/Users/test/myproject", "Child Session", "code",
                r#"{"id":"claude-4-sonnet","providerID":"anthropic"}"#,
                1716200010000i64, 1716200050000i64, std::option::Option::<i64>::None,
            ],
        ).unwrap();
        drop(conn);
        db_path
    }

    fn seed_messages(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
            params!["msg1", "sess1", 1716200010000i64, 1716200010000i64, r#"{"role":"user","time":{"created":1716200010000}}"#],
        ).unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            params!["part1", "msg1", "sess1", 1716200010000i64, 1716200010000i64, r#"{"type":"text","text":"Hello, help me with this."}"#],
        ).unwrap();

        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
            params!["msg2", "sess1", 1716200020000i64, 1716200020000i64, r#"{"role":"assistant","parentID":"msg1","agent":"code","modelID":"claude-4-sonnet","providerID":"anthropic","time":{"created":1716200020000}}"#],
        ).unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            params!["part2", "msg2", "sess1", 1716200020000i64, 1716200020000i64, r#"{"type":"reasoning","thinking":"Let me think about this..."}"#],
        ).unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            params!["part3", "msg2", "sess1", 1716200021000i64, 1716200021000i64, r#"{"type":"text","text":"I'll check the code."}"#],
        ).unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            params!["part4", "msg2", "sess1", 1716200022000i64, 1716200022000i64, r#"{"type":"tool","callID":"tc1","tool":"bash","state":{"status":"completed","input":"ls -la","output":"file1.txt\nfile2.txt","title":"List files"}}"#],
        ).unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            params!["part5", "msg2", "sess1", 1716200023000i64, 1716200023000i64, r#"{"type":"tool","callID":"tc2","tool":"bash","state":{"status":"running","input":"grep -r TODO","title":"Search TODOs"}}"#],
        ).unwrap();
    }

    // ── Session mapping tests ──

    #[test]
    fn parses_root_session_from_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let sessions = query_sessions(&oc_conn).unwrap();
        assert_eq!(sessions.len(), 2);
        let root = sessions.iter().find(|s| s.parent_session_id.is_none()).unwrap();
        assert_eq!(root.id, "opencode:sess1");
        assert_eq!(root.agent, "opencode");
        assert_eq!(root.project_path.as_deref(), Some("/Users/test/myproject"));
        assert_eq!(root.title.as_deref(), Some("Test Session"));
        assert_eq!(root.model.as_deref(), Some("claude-4-sonnet"));
        assert!(matches!(root.status, SessionStatus::Active));
    }

    #[test]
    fn parses_child_session_with_parent_id() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let sessions = query_sessions(&oc_conn).unwrap();
        let child = sessions.iter().find(|s| s.parent_session_id.is_some()).unwrap();
        assert_eq!(child.id, "opencode:child1");
        assert_eq!(child.parent_session_id.as_deref(), Some("opencode:sess1"));
    }

    #[test]
    fn marks_archived_session_as_complete() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        { let conn = rusqlite::Connection::open(&db_path).unwrap(); conn.execute("UPDATE session SET time_archived = 1716209999000 WHERE id = 'sess1'", []).unwrap(); }
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let sessions = query_sessions(&oc_conn).unwrap();
        let root = sessions.iter().find(|s| s.id == "opencode:sess1").unwrap();
        assert!(matches!(root.status, SessionStatus::Complete));
    }

    // ── Message mapping tests ──

    #[test]
    fn maps_text_messages_from_parts() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        { let conn = rusqlite::Connection::open(&db_path).unwrap(); seed_messages(&conn); }
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let messages = query_messages(&oc_conn, "sess1", false).unwrap();
        let user_msg = messages.iter().find(|m| m.role == "human").unwrap();
        assert_eq!(user_msg.content_preview, "Hello, help me with this.");
        assert_eq!(user_msg.session_id, "opencode:sess1");
        assert!(user_msg.parent_id.is_none());
    }

    #[test]
    fn maps_tool_parts_to_tool_use_and_result() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        { let conn = rusqlite::Connection::open(&db_path).unwrap(); seed_messages(&conn); }
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let messages = query_messages(&oc_conn, "sess1", false).unwrap();
        let tool_use = messages.iter().find(|m| m.kind == "tool_use").unwrap();
        assert_eq!(tool_use.tool_name.as_deref(), Some("bash"));
        assert!(tool_use.id.contains(":part:"));
        let tool_result = messages.iter().find(|m| m.kind == "tool_result").unwrap();
        assert_eq!(tool_result.tool_name.as_deref(), Some("bash"));
    }

    #[test]
    fn maps_reasoning_parts_to_thinking() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        { let conn = rusqlite::Connection::open(&db_path).unwrap(); seed_messages(&conn); }
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let messages = query_messages(&oc_conn, "sess1", false).unwrap();
        let thinking = messages.iter().find(|m| m.kind == "thinking").unwrap();
        assert_eq!(thinking.content_preview, "Let me think about this...");
        assert_eq!(thinking.entry_type, "reasoning");
    }

    #[test]
    fn marks_child_session_messages_as_sidechain() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_test_opencode_db(&dir);
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
                params!["cmsg1", "child1", 1716200030000i64, 1716200030000i64, r#"{"role":"user","time":{"created":1716200030000}}"#],
            ).unwrap();
            conn.execute(
                "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
                params!["cpart1", "cmsg1", "child1", 1716200030000i64, 1716200030000i64, r#"{"type":"text","text":"sub task"}"#],
            ).unwrap();
        }
        let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
        let messages = query_messages(&oc_conn, "child1", true).unwrap();
        assert!(messages[0].is_sidechain);
    }
}
