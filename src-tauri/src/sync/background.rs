use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::Emitter;

use crate::adapter::claudecode;
use crate::app::errors::AppError;
use crate::app::events::{SyncCompletedPayload, SyncProgressPayload, SyncStartedPayload};
use crate::store::schema;

/// Prevents concurrent scans via an AtomicBool.
pub struct ScanGuard {
    running: AtomicBool,
}

impl ScanGuard {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
        }
    }

    /// Try to mark a scan as running. Returns false if one is already active.
    pub fn try_start(&self) -> bool {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn finish(&self) {
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
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| AppError::DbError(e.to_string()))?;
    schema::init_schema(&conn)?;
    Ok(conn)
}

/// Spawn a background thread to run a full scan.
/// Returns false if a scan is already in progress.
pub fn spawn_background_scan(
    db_path: std::path::PathBuf,
    app_handle: tauri::AppHandle,
    scan_guard: Arc<ScanGuard>,
) -> bool {
    if !scan_guard.try_start() {
        log::info!("Scan already in progress, skipping");
        return false;
    }

    std::thread::Builder::new()
        .name("yeek-sync".into())
        .spawn(move || {
            let result = run_scan(&db_path, &app_handle);
            scan_guard.finish();

            match result {
                Ok(summary) => {
                    log::info!(
                        "Background scan completed: indexed={}, updated={}, errors={}",
                        summary.sessions_indexed,
                        summary.sessions_updated,
                        summary.errors,
                    );
                }
                Err(e) => {
                    log::error!("Background scan failed: {}", e);
                    let _ = app_handle.emit(
                        "sync-completed",
                        SyncCompletedPayload {
                            sessions_indexed: 0,
                            sessions_updated: 0,
                            errors: 1,
                        },
                    );
                }
            }
        })
        .expect("Failed to spawn sync thread");

    true
}

fn run_scan(
    db_path: &std::path::Path,
    app_handle: &tauri::AppHandle,
) -> Result<SyncSummary, AppError> {
    let conn = open_sync_connection(db_path)?;

    // Discover all sources first (pure filesystem, no DB needed)
    let sources = claudecode::discover_sources()?;
    let total = sources.len() as i64;

    let _ = app_handle.emit(
        "sync-started",
        SyncStartedPayload {
            source_count: total,
        },
    );

    // Delegate to incremental indexer with progress callback
    let result = claudecode::index_sources(&conn, &sources, |processed| {
        let _ = app_handle.emit(
            "sync-progress",
            SyncProgressPayload {
                processed,
                total,
            },
        );
    })?;

    let _ = app_handle.emit(
        "sync-completed",
        SyncCompletedPayload {
            sessions_indexed: result.indexed,
            sessions_updated: result.updated,
            errors: result.errors,
        },
    );

    Ok(SyncSummary {
        sessions_indexed: result.indexed,
        sessions_updated: result.updated,
        errors: result.errors,
    })
}
