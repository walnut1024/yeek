use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::app::errors::AppError;

use super::{discover_sources, index_single_source};

// ── Diagnostic types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScanStage {
    Discover,
    FingerprintLoad,
    Parse,
    SessionUpsert,
    MessageUpsert,
    SourceUpsert,
    SourceLink,
    FtsRebuild,
    Commit,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanErrorDetail {
    pub source_path: String,
    pub stage: ScanStage,
    pub error_kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanDiagnosticResult {
    pub total_discovered: usize,
    pub total_attempted: usize,
    pub total_succeeded: usize,
    pub total_skipped: usize,
    pub total_failed: usize,
    pub failures: Vec<ScanErrorDetail>,
    pub failure_summary: HashMap<String, usize>,
}

// ── Error classification ──────────────────────────────────────────

/// Map an `AppError` to a `(ScanStage, error_kind)` pair.
///
/// Classification is best-effort: it inspects the error message text
/// to determine which pipeline stage produced the failure.
pub fn classify_error(e: &AppError) -> (ScanStage, String) {
    let msg = e.to_string();
    let msg_lower = msg.to_lowercase();

    match e {
        AppError::ParseError(_) => (ScanStage::Parse, "parse_error".into()),

        AppError::Internal(internal_msg) => {
            // "Invalid subagent path" or "subagent" hints → parse failure
            if internal_msg.contains("subagent") || internal_msg.contains("Invalid") {
                (ScanStage::Parse, "internal_path_error".into())
            } else {
                (ScanStage::Unknown, "internal".into())
            }
        }

        AppError::DbError(_) => {
            if msg_lower.contains("fts5") || msg_lower.contains("messages_fts") {
                (ScanStage::FtsRebuild, "db_error".into())
            } else if msg_lower.contains("sessions")
                && (msg_lower.contains("insert") || msg_lower.contains("update"))
            {
                (ScanStage::SessionUpsert, "db_error".into())
            } else if msg_lower.contains("messages")
                && (msg_lower.contains("insert") || msg_lower.contains("update"))
            {
                (ScanStage::MessageUpsert, "db_error".into())
            } else if msg_lower.contains("sources") {
                (ScanStage::SourceUpsert, "db_error".into())
            } else if msg_lower.contains("session_sources") {
                (ScanStage::SourceLink, "db_error".into())
            } else {
                (ScanStage::Unknown, "db_error".into())
            }
        }

        _ => (ScanStage::Unknown, "unknown".into()),
    }
}

// ── Diagnostic scan ───────────────────────────────────────────────

/// Run a read-only diagnostic scan over all discoverable sources.
///
/// Opens a **separate** connection so the running app is not disturbed.
/// All writes happen inside a transaction that is **rolled back** at the
/// end, ensuring the database is not modified.
pub fn run_diagnostic_scan(db_path: &Path) -> Result<ScanDiagnosticResult, AppError> {
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| AppError::DbError(format!("Failed to open {}: {}", db_path.display(), e)))?;
    crate::store::schema::configure_connection(&conn)?;

    // Stage 1 — discover
    let sources = discover_sources()?;
    let total_discovered = sources.len();

    // Stage 2 — load fingerprints (same query as index_sources)
    let existing_fingerprints: HashMap<String, String> = {
        let mut stmt =
            conn.prepare("SELECT path, fingerprint FROM sources WHERE status = 'active'")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let mut total_attempted = 0usize;
    let mut total_succeeded = 0usize;
    let mut total_skipped = 0usize;
    let mut total_failed = 0usize;
    let mut failures: Vec<ScanErrorDetail> = Vec::new();

    // Begin a transaction that we will ROLLBACK — diagnostic must be read-only
    conn.execute_batch("BEGIN")?;

    for source in &sources {
        // Skip unchanged files (same logic as index_sources)
        if let Some(stored_fp) = existing_fingerprints.get(&source.path) {
            if *stored_fp == source.fingerprint {
                total_skipped += 1;
                continue;
            }
        }

        total_attempted += 1;

        let sp_name = format!("diag_sp_{}", total_attempted);
        conn.execute_batch(&format!("SAVEPOINT {}", sp_name))?;

        match index_single_source(&conn, source, &existing_fingerprints) {
            Ok(_) => {
                conn.execute_batch(&format!("RELEASE {}", sp_name))?;
                total_succeeded += 1;
            }
            Err(e) => {
                let _ = conn.execute_batch(&format!("ROLLBACK TO {}", sp_name));
                let _ = conn.execute_batch(&format!("RELEASE {}", sp_name));

                let (stage, error_kind) = classify_error(&e);
                failures.push(ScanErrorDetail {
                    source_path: source.path.clone(),
                    stage,
                    error_kind: error_kind.clone(),
                    message: e.to_string(),
                });
                total_failed += 1;
            }
        }
    }

    // Roll back the entire transaction — no side effects
    conn.execute_batch("ROLLBACK")?;

    // Build summary: group by "{stage}:{error_kind}"
    let mut failure_summary: HashMap<String, usize> = HashMap::new();
    for detail in &failures {
        let stage_str = match &detail.stage {
            ScanStage::Discover => "discover",
            ScanStage::FingerprintLoad => "fingerprint_load",
            ScanStage::Parse => "parse",
            ScanStage::SessionUpsert => "session_upsert",
            ScanStage::MessageUpsert => "message_upsert",
            ScanStage::SourceUpsert => "source_upsert",
            ScanStage::SourceLink => "source_link",
            ScanStage::FtsRebuild => "fts_rebuild",
            ScanStage::Commit => "commit",
            ScanStage::Unknown => "unknown",
        };
        let key = format!("{}:{}", stage_str, detail.error_kind);
        *failure_summary.entry(key).or_insert(0) += 1;
    }

    Ok(ScanDiagnosticResult {
        total_discovered,
        total_attempted,
        total_succeeded,
        total_skipped,
        total_failed,
        failures,
        failure_summary,
    })
}
