pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    project_path TEXT,
    title TEXT,
    model TEXT,
    git_branch TEXT,
    started_at DATETIME,
    ended_at DATETIME,
    status TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'visible',
    pinned INTEGER NOT NULL DEFAULT 0,
    archived_at DATETIME,
    deleted_at DATETIME,
    delete_mode TEXT NOT NULL DEFAULT 'none',
    message_count INTEGER NOT NULL DEFAULT 0,
    updated_at DATETIME NOT NULL,
    parent_session_id TEXT
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    parent_id TEXT,
    role TEXT NOT NULL,
    kind TEXT NOT NULL,
    content_preview TEXT NOT NULL,
    timestamp DATETIME,
    is_sidechain INTEGER NOT NULL DEFAULT 0,
    entry_type TEXT NOT NULL DEFAULT 'message',
    subtype TEXT,
    tool_name TEXT,
    subagent_id TEXT,
    model TEXT,
    metadata TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    source_type TEXT NOT NULL,
    path TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    last_modified DATETIME NOT NULL,
    last_seen_at DATETIME NOT NULL,
    status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS session_sources (
    session_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    source_type TEXT NOT NULL,
    path TEXT NOT NULL,
    delete_policy TEXT NOT NULL,
    PRIMARY KEY (session_id, source_id),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS action_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT,
    action TEXT NOT NULL,
    detail TEXT,
    created_at DATETIME NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    session_id UNINDEXED,
    message_id UNINDEXED,
    role,
    kind,
    content_preview
);
"#;

/// Add a column to a table, ignoring "duplicate column" errors (column already exists).
fn add_column_if_not_exists(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
    def: &str,
) -> Result<(), crate::app::errors::AppError> {
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, def);
    match conn.execute_batch(&sql) {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate column name") {
                Ok(())
            } else {
                Err(crate::app::errors::AppError::DbError(msg))
            }
        }
    }
}

/// Version 1 migration: add columns for rich entry data (existing migration)
fn migrate_v1(conn: &rusqlite::Connection) -> Result<(), crate::app::errors::AppError> {
    add_column_if_not_exists(conn, "messages", "entry_type", "TEXT NOT NULL DEFAULT 'message'")?;
    add_column_if_not_exists(conn, "messages", "subtype", "TEXT")?;
    add_column_if_not_exists(conn, "messages", "tool_name", "TEXT")?;
    add_column_if_not_exists(conn, "messages", "subagent_id", "TEXT")?;
    add_column_if_not_exists(conn, "messages", "model", "TEXT")?;
    add_column_if_not_exists(conn, "messages", "metadata", "TEXT")?;
    add_column_if_not_exists(conn, "sessions", "parent_session_id", "TEXT")?;
    Ok(())
}

/// Version 2 migration: add indexes for query performance
fn migrate_v2(conn: &rusqlite::Connection) -> Result<(), crate::app::errors::AppError> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sessions_visibility ON sessions(visibility);
         CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent);
         CREATE INDEX IF NOT EXISTS idx_sessions_project_path ON sessions(project_path);
         CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at);
         CREATE INDEX IF NOT EXISTS idx_sessions_pinned ON sessions(pinned);
         CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
         CREATE INDEX IF NOT EXISTS idx_messages_session_timestamp ON messages(session_id, timestamp);
         CREATE INDEX IF NOT EXISTS idx_action_log_action ON action_log(action);
         CREATE INDEX IF NOT EXISTS idx_action_log_created ON action_log(created_at);",
    )?;
    Ok(())
}

pub fn init_schema(conn: &rusqlite::Connection) -> Result<(), crate::app::errors::AppError> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         PRAGMA busy_timeout=5000;
         PRAGMA synchronous=NORMAL;",
    )?;
    conn.execute_batch(SCHEMA_SQL)?;

    // Version-based migration system
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);

    if version < 1 {
        migrate_v1(conn)?;
    }
    if version < 2 {
        migrate_v2(conn)?;
    }

    // Set to latest version
    conn.execute_batch("PRAGMA user_version = 2;")?;

    Ok(())
}
