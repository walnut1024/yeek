# OpenCode Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an OpenCode source adapter that reads OpenCode's SQLite database and ingests sessions/messages into Yeek's existing data model.

**Architecture:** New `adapter/opencode/mod.rs` follows the same convention as `adapter/codex/mod.rs` — three public functions (`discover_sources`, `source_descriptor_from_path`, `index_sources`). Opens OpenCode's DB read-only via rusqlite, queries sessions/messages/parts, and upserts into Yeek's SQLite.

**Tech Stack:** Rust, rusqlite (bundled), serde_json, chrono, tempfile (tests)

---

### Task 1: Adapter skeleton — discovery and fingerprint

**Files:**
- Create: `src-tauri/src/adapter/opencode/mod.rs`

- [ ] **Step 1: Write the adapter module with discover_sources, source_descriptor_from_path, and compute_fingerprint**

```rust
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::app::errors::AppError;
use crate::domain::session::{DeleteMode, SessionRecord, SessionStatus, VisibilityStatus};
use crate::domain::source::SourceDescriptor;
use crate::store::messages::MessageRecord;
use crate::store::sessions;

// ── Discovery ──

/// Resolve the OpenCode data directory.
/// Checks OPENCODE_DB env var, then platform-specific XDG paths.
fn opencode_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // OPENCODE_DB override
    if let Ok(val) = std::env::var("OPENCODE_DB") {
        if Path::new(&val).is_absolute() {
            // Use parent directory as the search dir
            if let Some(parent) = Path::new(&val).parent() {
                dirs.push(parent.to_path_buf());
            }
        }
    }

    // macOS: ~/Library/Application Support/opencode
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return dirs,
    };
    dirs.push(home.join("Library/Application Support/opencode"));

    // XDG_DATA_HOME/opencode
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(xdg).join("opencode"));
    }

    // Fallback: ~/.local/share/opencode
    dirs.push(home.join(".local/share/opencode"));

    dirs
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
    if file_name != "opencode.db" && !(file_name.starts_with("opencode-") && file_name.ends_with(".db")) {
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
            DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default().to_rfc3339()
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
        },
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
    // Placeholder — implemented in Task 3
    Ok(IndexResult {
        indexed: 0,
        updated: 0,
        errors: sources.len() as i64,
    })
}
```

- [ ] **Step 2: Register the module in adapter/mod.rs**

Add to `src-tauri/src/adapter/mod.rs`:

```rust
pub mod claudecode;
pub mod codex;
pub mod opencode;
```

- [ ] **Step 3: Run cargo check to verify compilation**

Run: `cargo check -p yeek 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Write failing tests for discovery and source_descriptor**

Add at the bottom of `src-tauri/src/adapter/opencode/mod.rs`:

```rust
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
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p yeek opencode -- --nocapture 2>&1 | tail -15`
Expected: 5 passed

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/adapter/opencode/mod.rs src-tauri/src/adapter/mod.rs
git commit -m "feat(opencode): adapter skeleton with discovery and fingerprint"
```

---

### Task 2: Schema validation helper

**Files:**
- Modify: `src-tauri/src/adapter/opencode/mod.rs`

- [ ] **Step 1: Write the failing test for schema validation**

Add inside `#[cfg(test)] mod tests`:

```rust
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
    conn.execute_batch("CREATE TABLE session (id TEXT PRIMARY KEY);").unwrap();
    drop(conn);

    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    assert!(validate_opencode_schema(&oc_conn).is_err());
}
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cargo test -p yeek validate_schema -- --nocapture 2>&1 | tail -10`
Expected: compile errors — `open_opencode_db_readonly` and `validate_opencode_schema` not defined

- [ ] **Step 3: Implement open_opencode_db_readonly and validate_opencode_schema**

Add above the `#[cfg(test)]` block:

```rust
/// Open an OpenCode SQLite database in strict read-only mode.
fn open_opencode_db_readonly(
    path: &Path,
) -> Result<rusqlite::Connection, AppError> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| AppError::DbError(format!("Failed to open OpenCode DB at {}: {}", path.display(), e)))?;

    conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|e| AppError::DbError(format!("Failed to set pragmas: {}", e)))?;

    Ok(conn)
}

/// Validate that the OpenCode DB has the minimum required schema.
fn validate_opencode_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
    let required_tables = ["session", "project", "message", "part"];
    let required_columns: &[(&str, &[&str])] = &[
        ("session", &["id", "project_id", "parent_id", "directory", "title", "agent", "model", "time_created", "time_updated", "time_archived"]),
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
                .map_err(|e| AppError::DbError(format!("Column validation query failed: {}", e)))?;

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p yeek validate_schema -- --nocapture 2>&1 | tail -10`
Expected: 2 passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/adapter/opencode/mod.rs
git commit -m "feat(opencode): add schema validation and read-only DB open"
```

---

### Task 3: Session mapping — query and convert

**Files:**
- Modify: `src-tauri/src/adapter/opencode/mod.rs`

- [ ] **Step 1: Write the failing test for session parsing from SQLite**

Add inside `#[cfg(test)] mod tests`:

```rust
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

    // Insert test project
    conn.execute(
        "INSERT INTO project (id, worktree) VALUES (?, ?)",
        params!["proj1", "/Users/test/myproject"],
    ).unwrap();

    // Insert root session
    conn.execute(
        "INSERT INTO session (id, project_id, directory, title, agent, model, time_created, time_updated, time_archived) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            "sess1",
            "proj1",
            "/Users/test/myproject",
            "Test Session",
            "code",
            r#"{"id":"claude-4-sonnet","providerID":"anthropic"}"#,
            1716200000000i64,
            1716200060000i64,
            std::option::Option::<i64>::None,
        ],
    ).unwrap();

    // Insert child session
    conn.execute(
        "INSERT INTO session (id, project_id, parent_id, directory, title, agent, model, time_created, time_updated, time_archived) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            "child1",
            "proj1",
            "sess1",
            "/Users/test/myproject",
            "Child Session",
            "code",
            r#"{"id":"claude-4-sonnet","providerID":"anthropic"}"#,
            1716200010000i64,
            1716200050000i64,
            std::option::Option::<i64>::None,
        ],
    ).unwrap();

    drop(conn);
    db_path
}

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
    assert!(root.parent_session_id.is_none());
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

    // Update the root session to be archived
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("UPDATE session SET time_archived = 1716209999000 WHERE id = 'sess1'", []).unwrap();
    }

    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    let sessions = query_sessions(&oc_conn).unwrap();

    let root = sessions.iter().find(|s| s.id == "opencode:sess1").unwrap();
    assert!(matches!(root.status, SessionStatus::Complete));
}
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cargo test -p yeek parses_root_session -- --nocapture 2>&1 | tail -10`
Expected: compile error — `query_sessions` not defined

- [ ] **Step 3: Implement query_sessions with session mapping**

Add above the `#[cfg(test)]` block:

```rust
/// Query all sessions from the OpenCode DB and convert to Yeek SessionRecords.
fn query_sessions(
    oc_conn: &rusqlite::Connection,
) -> Result<Vec<SessionRecord>, AppError> {
    let mut stmt = oc_conn.prepare(
        "SELECT s.id, s.parent_id, s.directory, s.title, s.agent, s.model,
                s.time_created, s.time_updated, s.time_archived,
                p.worktree,
                (SELECT COUNT(*) FROM message m WHERE m.session_id = s.id) AS msg_count
         FROM session s
         LEFT JOIN project p ON s.project_id = p.id
         ORDER BY s.time_created ASC"
    ).map_err(|e| AppError::DbError(format!("Failed to prepare session query: {}", e)))?;

    let sessions = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let parent_id: Option<String> = row.get(1)?;
            let directory: String = row.get(2)?;
            let title: String = row.get(3)?;
            let _agent: Option<String> = row.get(4)?;
            let model_json: Option<String> = row.get(5)?;
            let time_created: i64 = row.get(6)?;
            let time_updated: i64 = row.get(7)?;
            let time_archived: Option<i64> = row.get(8)?;
            let worktree: Option<String> = row.get(9)?;
            let msg_count: i64 = row.get(10)?;

            let status = if time_archived.is_some() {
                SessionStatus::Complete
            } else {
                SessionStatus::Active
            };

            let model = model_json
                .as_deref()
                .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                .and_then(|v| {
                    v.get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            let pid = v.get("providerID").and_then(|v| v.as_str()).unwrap_or("");
                            let mid = v.get("modelID").and_then(|v| v.as_str()).unwrap_or("");
                            if mid.is_empty() { None } else { Some(format!("{}/{}", pid, mid)) }
                        })
                });

            let project_path = worktree.or_else(|| {
                if directory.is_empty() { None } else { Some(directory.clone()) }
            });

            let title = if title.is_empty() { None } else { Some(title) };

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
                updated_at: ms_to_rfc3339(time_updated).unwrap_or_else(|| Utc::now().to_rfc3339()),
                parent_session_id: parent_id.map(|pid| format!("opencode:{}", pid)),
            })
        })
        .map_err(|e| AppError::DbError(format!("Session query failed: {}", e)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sessions)
}

/// Convert Unix milliseconds to RFC3339 UTC string.
fn ms_to_rfc3339(ms: i64) -> Option<String> {
    DateTime::from_timestamp_millis(ms).map(|dt| dt.to_rfc3339())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p yeek parses_root_session parses_child_session marks_archived -- --nocapture 2>&1 | tail -10`
Expected: 3 passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/adapter/opencode/mod.rs
git commit -m "feat(opencode): session mapping from SQLite to Yeek SessionRecord"
```

---

### Task 4: Message and part mapping

**Files:**
- Modify: `src-tauri/src/adapter/opencode/mod.rs`

- [ ] **Step 1: Write the failing tests for message and part parsing**

Add inside `#[cfg(test)] mod tests`:

```rust
fn seed_messages(oc_conn: &rusqlite::Connection) {
    // User message with text part
    oc_conn.execute(
        "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
        params![
            "msg1",
            "sess1",
            1716200010000i64,
            1716200010000i64,
            r#"{"role":"user","time":{"created":1716200010000}}"#,
        ],
    ).unwrap();
    oc_conn.execute(
        "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            "part1",
            "msg1",
            "sess1",
            1716200010000i64,
            1716200010000i64,
            r#"{"type":"text","text":"Hello, help me with this."}"#,
        ],
    ).unwrap();

    // Assistant message with text + tool + reasoning parts
    oc_conn.execute(
        "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
        params![
            "msg2",
            "sess1",
            1716200020000i64,
            1716200020000i64,
            r#"{"role":"assistant","parentID":"msg1","agent":"code","modelID":"claude-4-sonnet","providerID":"anthropic","time":{"created":1716200020000}}"#,
        ],
    ).unwrap();
    oc_conn.execute(
        "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            "part2",
            "msg2",
            "sess1",
            1716200020000i64,
            1716200020000i64,
            r#"{"type":"reasoning","thinking":"Let me think about this..."}"#,
        ],
    ).unwrap();
    oc_conn.execute(
        "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            "part3",
            "msg2",
            "sess1",
            1716200021000i64,
            1716200021000i64,
            r#"{"type":"text","text":"I'll check the code."}"#,
        ],
    ).unwrap();
    oc_conn.execute(
        "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            "part4",
            "msg2",
            "sess1",
            1716200022000i64,
            1716200022000i64,
            r#"{"type":"tool","callID":"tc1","tool":"bash","state":{"status":"completed","input":"ls -la","output":"file1.txt\nfile2.txt","title":"List files"}}"#,
        ],
    ).unwrap();
}

#[test]
fn maps_text_messages_from_parts() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = create_test_opencode_db(&dir);
    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    seed_messages(&oc_conn);

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
    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    seed_messages(&oc_conn);

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
    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    seed_messages(&oc_conn);

    let messages = query_messages(&oc_conn, "sess1", false).unwrap();

    let thinking = messages.iter().find(|m| m.kind == "thinking").unwrap();
    assert_eq!(thinking.content_preview, "Let me think about this...");
    assert_eq!(thinking.entry_type, "reasoning");
}

#[test]
fn marks_child_session_messages_as_sidechain() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = create_test_opencode_db(&dir);
    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();

    // Add message for child session
    oc_conn.execute(
        "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
        params!["cmsg1", "child1", 1716200030000i64, 1716200030000i64, r#"{"role":"user","time":{"created":1716200030000}}"#],
    ).unwrap();
    oc_conn.execute(
        "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
        params!["cpart1", "cmsg1", "child1", 1716200030000i64, 1716200030000i64, r#"{"type":"text","text":"sub task"}"#],
    ).unwrap();

    let messages = query_messages(&oc_conn, "child1", true).unwrap();
    assert!(messages[0].is_sidechain);
}
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cargo test -p yeek maps_text_messages -- --nocapture 2>&1 | tail -10`
Expected: compile error — `query_messages` not defined

- [ ] **Step 3: Implement query_messages with part mapping**

Add above the `#[cfg(test)]` block:

```rust
/// Query messages and parts for a single session from the OpenCode DB.
fn query_messages(
    oc_conn: &rusqlite::Connection,
    session_id: &str,
    is_sidechain: bool,
) -> Result<Vec<MessageRecord>, AppError> {
    // Load all messages for this session
    let mut msg_stmt = oc_conn.prepare(
        "SELECT id, time_created, data FROM message WHERE session_id = ? ORDER BY time_created ASC"
    ).map_err(|e| AppError::DbError(format!("Failed to prepare message query: {}", e)))?;

    let messages_raw: Vec<(String, i64, serde_json::Value)> = msg_stmt
        .query_map(params![session_id], |row| {
            let id: String = row.get(0)?;
            let time_created: i64 = row.get(1)?;
            let data_str: String = row.get(2)?;
            let data: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);
            Ok((id, time_created, data))
        })
        .map_err(|e| AppError::DbError(format!("Message query failed: {}", e)))?
        .filter_map(|r| r.ok())
        .collect();

    // Load all parts for this session
    let mut part_stmt = oc_conn.prepare(
        "SELECT id, message_id, time_created, data FROM part WHERE session_id = ? ORDER BY time_created ASC"
    ).map_err(|e| AppError::DbError(format!("Failed to prepare part query: {}", e)))?;

    let parts_raw: Vec<(String, String, i64, serde_json::Value)> = part_stmt
        .query_map(params![session_id], |row| {
            let id: String = row.get(0)?;
            let message_id: String = row.get(1)?;
            let time_created: i64 = row.get(2)?;
            let data_str: String = row.get(3)?;
            let data: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);
            Ok((id, message_id, time_created, data))
        })
        .map_err(|e| AppError::DbError(format!("Part query failed: {}", e)))?
        .filter_map(|r| r.ok())
        .collect();

    // Group parts by message_id
    let mut parts_by_msg: std::collections::HashMap<&str, Vec<&(String, String, i64, serde_json::Value)>> =
        std::collections::HashMap::new();
    for part in &parts_raw {
        parts_by_msg.entry(&part.1).or_default().push(part);
    }

    let prefixed_session_id = format!("opencode:{}", session_id);
    let mut records = Vec::new();

    for (msg_id, time_created, data) in &messages_raw {
        let role = data.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
        let yeek_role = match role {
            "user" => "human",
            _ => role,
        };
        let parent_id = data.get("parentID").and_then(|v| v.as_str()).map(|pid| format!("opencode:{}", pid));

        let msg_metadata = serde_json::json!({
            "role": role,
            "agent": data.get("agent").and_then(|v| v.as_str()).unwrap_or(""),
            "providerID": data.get("providerID").and_then(|v| v.as_str()).unwrap_or(""),
            "modelID": data.get("modelID").and_then(|v| v.as_str()).unwrap_or(""),
        });

        // Collect text parts for preview
        let parts = parts_by_msg.get(msg_id.as_str()).cloned().unwrap_or_default();
        let text_parts: Vec<&str> = parts.iter()
            .filter(|(_, _, _, d)| d.get("type").and_then(|v| v.as_str()) == Some("text"))
            .filter_map(|(_, _, _, d)| d.get("text").and_then(|v| v.as_str()))
            .collect();

        let text_preview = text_parts.join("\n");
        let truncated = truncate_to_chars(&text_preview, 500);
        let content_preview = if truncated.is_empty() {
            format!("[{}]", yeek_role)
        } else {
            truncated.to_string()
        };

        // Primary message record
        records.push(MessageRecord {
            id: format!("opencode:{}", msg_id),
            session_id: prefixed_session_id.clone(),
            parent_id,
            role: yeek_role.to_string(),
            kind: "message".to_string(),
            content_preview,
            timestamp: ms_to_rfc3339(*time_created),
            is_sidechain,
            entry_type: "message".to_string(),
            subtype: None,
            tool_name: None,
            subagent_id: None,
            model: data.get("modelID").and_then(|v| v.as_str()).map(|s| s.to_string()),
            metadata: Some(msg_metadata.to_string()),
        });

        // Extra records from parts (reasoning, tool_use, tool_result)
        for (part_id, _, part_time, part_data) in &parts {
            let part_type = part_data.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match part_type {
                "reasoning" => {
                    let thinking = part_data.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
                    records.push(MessageRecord {
                        id: format!("opencode:{}:part:{}", msg_id, part_id),
                        session_id: prefixed_session_id.clone(),
                        parent_id: Some(format!("opencode:{}", msg_id)),
                        role: "assistant".to_string(),
                        kind: "thinking".to_string(),
                        content_preview: truncate_to_chars(thinking, 500).to_string(),
                        timestamp: ms_to_rfc3339(*part_time),
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
                    let tool_name = part_data.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let state = part_data.get("state").cloned().unwrap_or(serde_json::Value::Null);
                    let status = state.get("status").and_then(|v| v.as_str()).unwrap_or("pending");

                    let (kind, preview) = match status {
                        "completed" => {
                            let output = state.get("output").and_then(|v| v.as_str()).unwrap_or("");
                            let title = state.get("title").and_then(|v| v.as_str()).unwrap_or("");
                            let text = if !title.is_empty() { title } else { output };
                            ("tool_result", truncate_to_chars(text, 500).to_string())
                        }
                        "error" => {
                            let error = state.get("error").and_then(|v| v.as_str()).unwrap_or("error");
                            ("tool_result", truncate_to_chars(error, 500).to_string())
                        }
                        _ => {
                            let input = state.get("input").and_then(|v| v.as_str()).unwrap_or("");
                            ("tool_use", format!("[tool: {}]", tool_name))
                        }
                    };

                    records.push(MessageRecord {
                        id: format!("opencode:{}:part:{}", msg_id, part_id),
                        session_id: prefixed_session_id.clone(),
                        parent_id: Some(format!("opencode:{}", msg_id)),
                        role: "assistant".to_string(),
                        kind: kind.to_string(),
                        content_preview: preview,
                        timestamp: ms_to_rfc3339(*part_time),
                        is_sidechain,
                        entry_type: "tool".to_string(),
                        subtype: None,
                        tool_name: Some(tool_name.to_string()),
                        subagent_id: None,
                        model: None,
                        metadata: None,
                    });
                }
                _ => {
                    // snapshot, patch, file, agent, subtask, compaction, retry, step-start, step-finish
                    // Fold into metadata — skip creating extra records for now
                }
            }
        }
    }

    Ok(records)
}

fn truncate_to_chars(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..s.char_indices().take(max).last().map(|(i, _)| i).unwrap_or(s.len())]
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p yeek opencode -- --nocapture 2>&1 | tail -20`
Expected: all tests passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/adapter/opencode/mod.rs
git commit -m "feat(opencode): message and part mapping from SQLite to MessageRecord"
```

---

### Task 5: Wire up index_sources with the full indexing flow

**Files:**
- Modify: `src-tauri/src/adapter/opencode/mod.rs`

- [ ] **Step 1: Write the failing test for full index_sources flow**

Add inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn index_sources_skips_unchanged_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = create_test_opencode_db(&dir);
    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    seed_messages(&oc_conn);
    drop(oc_conn);

    // Create Yeek DB
    let yeek_path = dir.path().join("yeek.db");
    let yeek_conn = rusqlite::Connection::open(&yeek_path).unwrap();
    crate::store::schema::init_schema(&yeek_conn).unwrap();

    let source = source_descriptor_from_path(&db_path).unwrap();

    // First pass
    let result = index_sources(&yeek_conn, &[source.clone()], |_| {}).unwrap();
    assert!(result.indexed + result.updated > 0);

    // Second pass with same fingerprint — should skip
    let result2 = index_sources(&yeek_conn, &[source], |_| {}).unwrap();
    assert_eq!(result2.indexed + result2.updated, 0);
}

#[test]
fn index_sources_clears_stale_messages_on_reindex() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = create_test_opencode_db(&dir);
    let oc_conn = open_opencode_db_readonly(&db_path).unwrap();
    seed_messages(&oc_conn);
    drop(oc_conn);

    let yeek_path = dir.path().join("yeek.db");
    let yeek_conn = rusqlite::Connection::open(&yeek_path).unwrap();
    crate::store::schema::init_schema(&yeek_conn).unwrap();

    let source = source_descriptor_from_path(&db_path).unwrap();
    let _ = index_sources(&yeek_conn, &[source], |_| {}).unwrap();

    // Verify messages exist
    let count: i64 = yeek_conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE session_id LIKE 'opencode:sess1%'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert!(count > 0);

    // Remove a message from OpenCode DB
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("DELETE FROM part WHERE id = 'part1'", []).unwrap();
        conn.execute("DELETE FROM message WHERE id = 'msg1'", []).unwrap();
    }

    // Force reindex by updating fingerprint
    let new_source = {
        let mut s = source_descriptor_from_path(&db_path).unwrap();
        s.fingerprint = "forced:change".to_string();
        s
    };

    let _ = index_sources(&yeek_conn, &[new_source], |_| {}).unwrap();

    // Stale message should be gone
    let remaining: i64 = yeek_conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE session_id = 'opencode:sess1' AND id = 'opencode:msg1'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(remaining, 0);
}
```

- [ ] **Step 2: Implement the real index_sources function**

Replace the placeholder `index_sources` with:

```rust
pub(crate) fn index_sources<F>(
    conn: &rusqlite::Connection,
    sources: &[SourceDescriptor],
    on_progress: F,
) -> Result<IndexResult, AppError>
where
    F: Fn(i64),
{
    if sources.is_empty() {
        return Ok(IndexResult { indexed: 0, updated: 0, errors: 0 });
    }

    // Load existing fingerprints for skip-if-unchanged
    let mut fp_stmt = conn.prepare("SELECT path, fingerprint FROM sources WHERE status = 'active'")
        .map_err(|e| AppError::DbError(format!("Failed to load fingerprints: {}", e)))?;
    let existing_fingerprints: std::collections::HashMap<String, String> = fp_stmt
        .query_map([], |row| {
            let path: String = row.get(0)?;
            let fp: String = row.get(1)?;
            Ok((path, fp))
        })
        .map_err(|e| AppError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    let mut indexed = 0i64;
    let mut updated = 0i64;
    let mut errors = 0i64;

    conn.execute_batch("BEGIN")?;

    for (i, source) in sources.iter().enumerate() {
        // Skip unchanged
        if let Some(stored_fp) = existing_fingerprints.get(&source.path) {
            if *stored_fp == source.fingerprint {
                on_progress((i + 1) as i64);
                continue;
            }
        }

        let sp = format!("sp_opencode_{}", i);
        conn.execute_batch(&format!("SAVEPOINT {}", sp))?;

        match index_single_source(conn, source, &existing_fingerprints) {
            Ok(is_update) => {
                conn.execute_batch(&format!("RELEASE {}", sp))?;
                if is_update { updated += 1; } else { indexed += 1; }
            },
            Err(e) => {
                tracing::error!("Failed to index OpenCode source {}: {}", source.path, e);
                conn.execute_batch(&format!("ROLLBACK TO {}", sp))?;
                errors += 1;
            },
        }

        on_progress((i + 1) as i64);
    }

    crate::store::actions::record_action(
        conn,
        None,
        "opencode_sync_completed",
        Some(&format!("indexed={}, updated={}, errors={}", indexed, updated, errors)),
    )?;

    conn.execute_batch("COMMIT")?;

    if indexed + updated > 0 {
        if let Err(e) = conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES ('rebuild');") {
            tracing::warn!("OpenCode FTS rebuild failed: {}", e);
        }
    }

    Ok(IndexResult { indexed, updated, errors })
}

fn index_single_source(
    conn: &rusqlite::Connection,
    source: &SourceDescriptor,
    existing_fingerprints: &std::collections::HashMap<String, String>,
) -> Result<bool, AppError> {
    let is_update = existing_fingerprints.contains_key(&source.path);

    let oc_conn = open_opencode_db_readonly(Path::new(&source.path))?;
    validate_opencode_schema(&oc_conn)?;

    let sessions = query_sessions(&oc_conn)?;

    for session in &sessions {
        // Extract the raw session ID (strip "opencode:" prefix)
        let raw_session_id = session.id.strip_prefix("opencode:").unwrap_or(&session.id);
        let is_sidechain = session.parent_session_id.is_some();

        // Delete stale messages for this session before reinserting
        conn.execute(
            "DELETE FROM messages WHERE session_id = ?",
            params![session.id],
        )?;

        // Query and insert messages
        let messages = query_messages(&oc_conn, raw_session_id, is_sidechain)?;
        sessions::upsert_session(conn, session)?;
        for msg in &messages {
            crate::store::messages::upsert_message(conn, msg)?;
        }
    }

    crate::store::sources::upsert_source(conn, source)?;
    for session in &sessions {
        crate::store::sources::link_session_source(
            conn,
            &session.id,
            &source.fingerprint,
            &source.source_type,
            &source.path,
            "not_allowed",
        )?;
    }

    Ok(is_update)
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p yeek opencode -- --nocapture 2>&1 | tail -20`
Expected: all tests passed

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/adapter/opencode/mod.rs
git commit -m "feat(opencode): full indexing flow with fingerprint skip and stale cleanup"
```

---

### Task 6: Integrate into sync pipeline — background scan

**Files:**
- Modify: `src-tauri/src/sync/background.rs`

- [ ] **Step 1: Add OpenCode to background scan**

In `src-tauri/src/sync/background.rs`, add `opencode` to the import:

```rust
use crate::adapter::{claudecode, codex, opencode};
```

In `run_scan`, after the Codex section (after line `let codex_sources = codex::discover_sources()?;`), add:

```rust
    let opencode_sources = opencode::discover_sources()?;
```

Update total:

```rust
    let total = (claude_sources.len() + codex_sources.len() + opencode_sources.len()) as i64;
```

After the Codex indexing block, add:

```rust
    // Index OpenCode sources
    let opencode_result = opencode::index_sources(&conn, &opencode_sources, |delta| {
        emitter.emit_sync_progress(SyncProgressPayload { processed: processed + delta, total });
    })?;
    processed += opencode_sources.len() as i64;
```

Update final counts:

```rust
    let indexed = claude_result.indexed + codex_result.indexed + opencode_result.indexed;
    let updated = claude_result.updated + codex_result.updated + opencode_result.updated;
    let errors = claude_result.errors + codex_result.errors + opencode_result.errors;
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p yeek 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/sync/background.rs
git commit -m "feat(opencode): integrate OpenCode adapter into background scan"
```

---

### Task 7: Integrate into sync pipeline — file watcher

**Files:**
- Modify: `src-tauri/src/sync/watcher.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add OpenCode routing in watcher.rs**

In `src-tauri/src/sync/watcher.rs`, add import:

```rust
use crate::adapter::opencode;
```

In `run_incremental_scan`, change the watcher file filter to also accept `.db` files. Update the path collection logic — after `let mut jsonl_paths: Vec<PathBuf> = Vec::new();` and the `.jsonl` extension check, add `.db` support:

After the block that collects `.jsonl` paths from `event.paths`, add:

```rust
                for p in &event.paths {
                    let ext = p.extension().and_then(|e| e.to_str());
                    if ext == Some("jsonl") {
                        jsonl_paths.push(p.clone());
                    } else if ext == Some("db") {
                        jsonl_paths.push(p.clone());
                    } else if p.is_dir() {
                        if let Ok(entries) = std::fs::read_dir(p) {
                            for entry in entries.flatten() {
                                let ep = entry.path();
                                let ext = ep.extension().and_then(|e| e.to_str());
                                if ext == Some("jsonl") || ext == Some("db") {
                                    jsonl_paths.push(ep);
                                }
                            }
                        }
                    }
                }
```

In the routing section of `run_incremental_scan`, add OpenCode before the existing adapters. Also remove the 10MB size limit for `.db` files:

```rust
    for p in changed_paths {
        let meta = match std::fs::metadata(p) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        // Skip large JSONL files, but allow large .db files (OpenCode DBs can be big)
        let ext = p.extension().and_then(|e| e.to_str());
        if ext != Some("db") && meta.len() > MAX_WATCHER_FILE_SIZE {
            continue;
        }

        if let Some(source) = opencode::source_descriptor_from_path(p) {
            opencode_sources.push(source);
        } else if let Some(source) = codex::source_descriptor_from_path(p) {
            codex_sources.push(source);
        } else if let Some(source) = claudecode::source_descriptor_from_path(p) {
            claude_sources.push(source);
        }
    }
```

Add `opencode_sources` to the declaration:

```rust
    let mut opencode_sources = Vec::new();
    let mut claude_sources = Vec::new();
    let mut codex_sources = Vec::new();
```

Add the OpenCode indexing call after Codex:

```rust
    let opencode_result = opencode::index_sources(&conn, &opencode_sources, |_| {})?;
```

Update the emitted counts:

```rust
    emitter.emit_sync_completed(SyncCompletedPayload {
        sessions_indexed: claude_result.indexed + codex_result.indexed + opencode_result.indexed,
        sessions_updated: claude_result.updated + codex_result.updated + opencode_result.updated,
        errors: claude_result.errors + codex_result.errors + opencode_result.errors,
    });
```

Update the tracing log to include OpenCode:

```rust
    tracing::info!(
        "Watcher: indexing {} claude + {} codex + {} opencode sources",
        claude_sources.len(),
        codex_sources.len(),
        opencode_sources.len()
    );
```

And update the early-return check:

```rust
    if claude_sources.is_empty() && codex_sources.is_empty() && opencode_sources.is_empty() {
        return Ok(());
    }
```

- [ ] **Step 2: Add OpenCode watcher in lib.rs**

In `src-tauri/src/lib.rs`, after the Codex watcher block (after line `}` closing the `if codex_sessions_dir.exists()` block), add:

```rust
            // OpenCode data directory watcher
            let opencode_data_dir = dirs::data_local_dir()
                .unwrap_or_else(|| dirs::home_dir().expect("No home dir").join("Library/Application Support"))
                .join("opencode");

            if opencode_data_dir.exists() {
                watchers.push(
                    sync::watcher::FileWatcher::start(
                        opencode_data_dir,
                        db_path.clone(),
                        emitter.clone(),
                        scan_guard.clone(),
                    )
                    .expect("Failed to start OpenCode file watcher"),
                );
            }
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p yeek 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/sync/watcher.rs src-tauri/src/lib.rs
git commit -m "feat(opencode): integrate into file watcher pipeline"
```

---

### Task 8: Backend command filter + frontend agent label

**Files:**
- Modify: `src-tauri/src/app/commands.rs`
- Modify: `src/components/site-header.tsx`
- Modify: `src/pages/sessions/session-row.tsx`

- [ ] **Step 1: Allow "opencode" in browse filter**

In `src-tauri/src/app/commands.rs`, change the agent filter line (around line 136) from:

```rust
    let agent = request.agent.filter(|a| ["claude_code", "codex"].contains(&a.as_str()));
```

to:

```rust
    let agent = request.agent.filter(|a| ["claude_code", "codex", "opencode"].contains(&a.as_str()));
```

- [ ] **Step 2: Add OpenCode tab in site-header.tsx**

In `src/components/site-header.tsx`, add an import for the OpenCode icon (or use a text label). Add a new `TabsTrigger` after the Codex one in the sessions section (around line 34):

```tsx
            <TabsTrigger value="opencode" className="h-6 rounded-md px-2 text-[12px]">
              OpenCode
            </TabsTrigger>
```

And in the marketplace section (around line 48):

```tsx
            <TabsTrigger value="opencode" className="h-6 rounded-md px-2 text-[12px]">
              OpenCode
            </TabsTrigger>
```

- [ ] **Step 3: Add OpenCode label in session-row.tsx**

In `src/pages/sessions/session-row.tsx`, add to the `formatAgentLabel` function:

```typescript
  if (agent === "opencode") return "OpenCode";
```

- [ ] **Step 4: Run cargo check and npm run build**

Run: `cargo check -p yeek 2>&1 | tail -5 && npm run build 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/app/commands.rs src/components/site-header.tsx src/pages/sessions/session-row.tsx
git commit -m "feat(opencode): add agent filter, UI label, and browse support"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test -p yeek 2>&1 | tail -20`
Expected: all tests passed

- [ ] **Step 2: Run full typecheck and build**

Run: `npm run build 2>&1 | tail -10`
Expected: build succeeded

- [ ] **Step 3: Manual verification checklist**

- Start `cargo tauri dev` with an existing OpenCode DB at `~/Library/Application Support/opencode/opencode.db`
- Confirm OpenCode sessions appear in browse
- Confirm the "OpenCode" tab filter works
- Confirm search finds text from OpenCode messages
- Confirm child sessions are hidden from root browse but visible through session detail
- Confirm the OpenCode DB file is never modified by Yeek (check modified timestamp)
