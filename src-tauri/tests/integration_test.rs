//! Integration tests for yeek core functionality.

use yeek_lib::domain::session::{DeleteMode, SessionRecord, SessionStatus, VisibilityStatus};
use yeek_lib::store::schema;
use yeek_lib::store::sessions::{self, BrowseParams, SearchParams};

fn setup_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    schema::init_schema(&conn).expect("schema init");
    conn
}

fn test_session(id: &str, title: &str) -> SessionRecord {
    SessionRecord {
        id: id.to_string(),
        agent: "claude".to_string(),
        project_path: Some("/tmp/test-project".to_string()),
        title: Some(title.to_string()),
        model: Some("claude-sonnet-4-20250514".to_string()),
        git_branch: None,
        started_at: Some("2026-01-01T00:00:00Z".to_string()),
        ended_at: None,
        status: SessionStatus::Active,
        visibility: VisibilityStatus::Visible,
        pinned: false,
        archived_at: None,
        deleted_at: None,
        delete_mode: DeleteMode::None,
        message_count: 0,
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        parent_session_id: None,
    }
}

fn test_session_with_agent(id: &str, title: &str, agent: &str) -> SessionRecord {
    let mut session = test_session(id, title);
    session.agent = agent.to_string();
    session
}

#[test]
fn test_schema_initialization() {
    let conn = setup_db();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("sessions table");
    assert_eq!(count, 0);
}

#[test]
fn test_upsert_and_browse_sessions() {
    let conn = setup_db();

    sessions::upsert_session(&conn, &test_session("s1", "Session One")).expect("upsert s1");
    sessions::upsert_session(&conn, &test_session("s2", "Session Two")).expect("upsert s2");

    let result = sessions::browse_sessions(&conn, &BrowseParams::default()).expect("browse");
    assert_eq!(result.sessions.len(), 2);
    assert_eq!(result.total, 2);
}

#[test]
fn test_soft_delete_updates_record() {
    let conn = setup_db();

    sessions::upsert_session(&conn, &test_session("s-del", "To Delete")).expect("upsert");

    sessions::soft_delete_sessions(&conn, &["s-del".to_string()]).expect("soft delete");

    // After soft delete, the record should be marked as deleted
    let session = sessions::get_session(&conn, "s-del").expect("get session");
    assert!(session.deleted_at.is_some(), "deleted_at should be set");
    assert!(
        matches!(session.delete_mode, DeleteMode::SoftDeleted),
        "delete_mode should be SoftDeleted"
    );
}

#[test]
fn test_get_session_by_id() {
    let conn = setup_db();
    sessions::upsert_session(&conn, &test_session("s-get", "Get Me")).expect("upsert");

    let session = sessions::get_session(&conn, "s-get").expect("get session");
    assert_eq!(session.title.as_deref(), Some("Get Me"));
    assert_eq!(session.agent, "claude");
}

#[test]
fn test_search_sessions() {
    let conn = setup_db();
    sessions::upsert_session(&conn, &test_session("s-alpha", "Alpha Project")).expect("upsert");
    sessions::upsert_session(&conn, &test_session("s-beta", "Beta Research")).expect("upsert");

    let params = SearchParams { query: "Alpha".to_string(), limit: 10, offset: 0, agent: None };
    let result = sessions::search_sessions(&conn, &params).expect("search");
    assert_eq!(result.sessions.len(), 1);
    assert_eq!(result.sessions[0].id, "s-alpha");
}

#[test]
fn test_search_sessions_filters_by_agent() {
    let conn = setup_db();
    sessions::upsert_session(
        &conn,
        &test_session_with_agent("s-claude", "Shared Project", "claude_code"),
    )
    .expect("upsert claude");
    sessions::upsert_session(&conn, &test_session_with_agent("s-codex", "Shared Project", "codex"))
        .expect("upsert codex");

    let params = SearchParams {
        query: "Shared".to_string(),
        limit: 10,
        offset: 0,
        agent: Some("codex".to_string()),
    };
    let result = sessions::search_sessions(&conn, &params).expect("search");

    assert_eq!(result.sessions.len(), 1);
    assert_eq!(result.sessions[0].id, "s-codex");
}
