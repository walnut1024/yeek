use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::Emitter;

use crate::adapter::claudecode::{self, source_descriptor_from_path};
use crate::app::errors::AppError;
use crate::app::events::SyncCompletedPayload;
use crate::store::schema;
use crate::sync::background::ScanGuard;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Start watching a directory for .jsonl changes.
    /// Debounces events: accumulates paths, then scans after 2s of quiet.
    /// If a scan is already running, paths are queued for the next cycle.
    pub fn start(
        watch_dir: PathBuf,
        db_path: PathBuf,
        app_handle: tauri::AppHandle,
        scan_guard: Arc<ScanGuard>,
    ) -> Result<Self, AppError> {
        let pending_paths: Arc<std::sync::Mutex<Vec<PathBuf>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let debounce_active: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

        let db_path_clone = db_path.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                let event = match res {
                    Ok(e) => e,
                    Err(_) => return,
                };

                // Only react to events on .jsonl files
                let jsonl_paths: Vec<PathBuf> = event
                    .paths
                    .into_iter()
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
                    .collect();
                if jsonl_paths.is_empty() {
                    return;
                }

                // Accumulate paths
                {
                    let mut pending = pending_paths.lock().unwrap();
                    for p in jsonl_paths {
                        if !pending.contains(&p) {
                            pending.push(p);
                        }
                    }
                }

                // Only spawn one debounce timer at a time
                if debounce_active.load(Ordering::Relaxed) {
                    return;
                }
                debounce_active.store(true, Ordering::Relaxed);

                let pp = pending_paths.clone();
                let sg = scan_guard.clone();
                let db = db_path_clone.clone();
                let ah = app_handle.clone();
                let da = debounce_active.clone();

                std::thread::Builder::new()
                    .name("yeek-debounce".into())
                    .spawn(move || {
                        std::thread::sleep(Duration::from_secs(2));
                        da.store(false, Ordering::Relaxed);

                        // Drain pending paths
                        let paths: Vec<PathBuf> = {
                            let mut pending = pp.lock().unwrap();
                            std::mem::take(&mut *pending)
                        };
                        if paths.is_empty() {
                            return;
                        }

                        // Try to acquire scan guard
                        if !sg.try_start() {
                            // Another scan is running — put paths back for next cycle
                            let mut pending = pp.lock().unwrap();
                            for p in paths {
                                if !pending.contains(&p) {
                                    pending.push(p);
                                }
                            }
                            return;
                        }

                        let result = run_incremental_scan(&db, &paths, &ah);
                        sg.finish();

                        if let Err(e) = result {
                            log::error!("Watcher incremental scan failed: {}", e);
                        }
                    })
                    .ok();
            },
            Config::default().with_poll_interval(Duration::from_secs(3)),
        )
        .map_err(|e| AppError::Internal(format!("Failed to create file watcher: {}", e)))?;

        watcher
            .watch(&watch_dir, RecursiveMode::Recursive)
            .map_err(|e| AppError::Internal(format!("Failed to start watching: {}", e)))?;

        log::info!("File watcher started on {}", watch_dir.display());

        Ok(Self { _watcher: watcher })
    }
}

fn run_incremental_scan(
    db_path: &Path,
    changed_paths: &[PathBuf],
    app_handle: &tauri::AppHandle,
) -> Result<(), AppError> {
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| AppError::DbError(e.to_string()))?;
    schema::init_schema(&conn)?;

    // Build SourceDescriptors from changed paths, skip files over 10MB
    const MAX_WATCHER_FILE_SIZE: u64 = 10 * 1024 * 1024;
    let sources: Vec<_> = changed_paths
        .iter()
        .filter_map(|p| {
            let meta = std::fs::metadata(p).ok()?;
            if meta.len() > MAX_WATCHER_FILE_SIZE {
                log::info!("Watcher: skipping large file ({}MB): {}", meta.len() / 1024 / 1024, p.display());
                return None;
            }
            source_descriptor_from_path(p)
        })
        .collect();

    if sources.is_empty() {
        return Ok(());
    }

    // Filter out sources already known to the DB (fingerprint matches).
    // The watcher should only index truly new or changed sources.
    let existing: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("SELECT path FROM sources WHERE status = 'active'")?;
        let rows: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        rows.into_iter().collect()
    };

    let new_sources: Vec<_> = sources
        .into_iter()
        .filter(|s| !existing.contains(&s.path))
        .collect();

    if new_sources.is_empty() {
        return Ok(());
    }

    let total = new_sources.len() as i64;
    log::info!("Watcher: indexing {} new sources", total);

    let result = claudecode::index_sources(&conn, &new_sources, |_| {})?;

    let _ = app_handle.emit(
        "sync-completed",
        SyncCompletedPayload {
            sessions_indexed: result.indexed,
            sessions_updated: result.updated,
            errors: result.errors,
        },
    );

    Ok(())
}
