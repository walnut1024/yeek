use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::adapter::{claudecode, codex, opencode};
use crate::app::errors::AppError;
use crate::app::events::{
    EventEmitter, SyncCompletedPayload, SyncProgressPayload, SyncStartedPayload,
};
use crate::store::schema;

/// Prevents concurrent scans via an AtomicBool.
pub struct ScanGuard {
    running: AtomicBool,
}

impl ScanGuard {
    pub fn new() -> Self {
        Self { running: AtomicBool::new(false) }
    }

    /// Try to mark a scan as running. Returns false if one is already active.
    pub(crate) fn try_start(&self) -> bool {
        self.running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }

    pub(crate) fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

pub struct SyncSummary {
    pub sessions_indexed: i64,
    pub sessions_updated: i64,
    pub errors: i64,
}

/// Open a second SQLite connection configured identically to the primary.
fn open_sync_connection(db_path: &std::path::Path) -> Result<rusqlite::Connection, AppError> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| AppError::DbError(e.to_string()))?;
    schema::init_schema(&conn)?;
    Ok(conn)
}

/// Spawn a background thread to run a full scan.
/// Returns false if a scan is already in progress.
pub fn spawn_background_scan(
    db_path: std::path::PathBuf,
    emitter: Arc<dyn EventEmitter>,
    scan_guard: Arc<ScanGuard>,
) -> bool {
    if !scan_guard.try_start() {
        tracing::info!("Scan already in progress, skipping");
        return false;
    }

    std::thread::Builder::new()
        .name("yeek-sync".into())
        .spawn(move || {
            let result = run_scan(&db_path, emitter.as_ref());
            scan_guard.finish();

            match result {
                Ok(summary) => {
                    tracing::info!(
                        "Background scan completed: indexed={}, updated={}, errors={}",
                        summary.sessions_indexed,
                        summary.sessions_updated,
                        summary.errors,
                    );
                },
                Err(e) => {
                    tracing::error!("Background scan failed: {}", e);
                    emitter.emit_sync_completed(SyncCompletedPayload {
                        sessions_indexed: 0,
                        sessions_updated: 0,
                        errors: 1,
                    });
                },
            }
        })
        .expect("Failed to spawn sync thread");

    true
}

fn run_scan(
    db_path: &std::path::Path,
    emitter: &dyn EventEmitter,
) -> Result<SyncSummary, AppError> {
    let conn = open_sync_connection(db_path)?;

    // Discover sources from each adapter separately
    let claude_sources = claudecode::discover_sources()?;
    let codex_sources = codex::discover_sources()?;
    let opencode_sources = opencode::discover_sources()?;
    let total = (claude_sources.len() + codex_sources.len() + opencode_sources.len()) as i64;

    emitter.emit_sync_started(SyncStartedPayload { source_count: total });

    // Index Claude sources
    let mut processed = 0i64;
    let claude_result = claudecode::index_sources(&conn, &claude_sources, |delta| {
        emitter.emit_sync_progress(SyncProgressPayload { processed: processed + delta, total });
    })?;
    processed += claude_sources.len() as i64;

    // Index Codex sources
    let codex_result = codex::index_sources(&conn, &codex_sources, |delta| {
        emitter.emit_sync_progress(SyncProgressPayload { processed: processed + delta, total });
    })?;
    processed += codex_sources.len() as i64;

    // Index OpenCode sources
    let opencode_result = opencode::index_sources(&conn, &opencode_sources, |delta| {
        emitter.emit_sync_progress(SyncProgressPayload { processed: processed + delta, total });
    })?;
    processed += opencode_sources.len() as i64;

    let indexed = claude_result.indexed + codex_result.indexed + opencode_result.indexed;
    let updated = claude_result.updated + codex_result.updated + opencode_result.updated;
    let errors = claude_result.errors + codex_result.errors + opencode_result.errors;

    emitter.emit_sync_completed(SyncCompletedPayload {
        sessions_indexed: indexed,
        sessions_updated: updated,
        errors,
    });

    Ok(SyncSummary {
        sessions_indexed: indexed,
        sessions_updated: updated,
        errors,
    })
}
