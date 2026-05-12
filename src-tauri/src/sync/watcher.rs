use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::adapter::claudecode;
use crate::adapter::codex;
use crate::app::errors::AppError;
use crate::app::events::{EventEmitter, SyncCompletedPayload};
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
        emitter: Arc<dyn EventEmitter>,
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

                // Collect all .jsonl paths from the event.
                // On macOS, FSEvents may report directories (e.g. a parent dir
                // was touched by an atomic write/rename). Scan directories and
                // add contained .jsonl files so we never miss an update.
                let mut jsonl_paths: Vec<PathBuf> = Vec::new();
                for p in &event.paths {
                    let ext = p.extension().and_then(|e| e.to_str());
                    if ext == Some("jsonl") {
                        jsonl_paths.push(p.clone());
                    } else if p.is_dir() {
                        // Directory changed — scan for .jsonl files inside
                        if let Ok(entries) = std::fs::read_dir(p) {
                            for entry in entries.flatten() {
                                let ep = entry.path();
                                if ep.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                                    jsonl_paths.push(ep);
                                }
                            }
                        }
                    }
                }

                tracing::info!(
                    "Watcher: {} event paths → {} jsonl files",
                    event.paths.len(),
                    jsonl_paths.len()
                );

                if jsonl_paths.is_empty() {
                    return;
                }

                // Accumulate paths
                {
                    let mut pending = pending_paths.lock().expect("mutex poisoned");
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
                let em = emitter.clone();
                let da = debounce_active.clone();

                std::thread::Builder::new()
                    .name("yeek-debounce".into())
                    .spawn(move || {
                        std::thread::sleep(Duration::from_secs(2));
                        da.store(false, Ordering::Relaxed);

                        // Drain pending paths
                        let paths: Vec<PathBuf> = {
                            let mut pending = pp.lock().expect("mutex poisoned");
                            std::mem::take(&mut *pending)
                        };
                        if paths.is_empty() {
                            return;
                        }

                        // Try to acquire scan guard
                        if !sg.try_start() {
                            // Another scan is running — put paths back for next cycle
                            let mut pending = pp.lock().expect("mutex poisoned");
                            for p in paths {
                                if !pending.contains(&p) {
                                    pending.push(p);
                                }
                            }
                            return;
                        }

                        let result = run_incremental_scan(&db, &paths, em.as_ref());
                        sg.finish();

                        if let Err(e) = result {
                            tracing::error!("Watcher incremental scan failed: {}", e);
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

        tracing::info!("File watcher started on {}", watch_dir.display());

        Ok(Self { _watcher: watcher })
    }

    /// Watch plugin config files for changes.
    /// Monitors `~/.claude/plugins/installed_plugins.json` and `~/.claude/settings.json`.
    /// Emits `"plugin-config-changed"` event with 500ms debounce.
    pub fn start_plugin_config_watcher(emitter: Arc<dyn EventEmitter>) -> Result<Self, AppError> {
        let home =
            dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
        let claude_dir = home.join(".claude");
        let plugins_dir = claude_dir.join("plugins");

        let installed_plugins = plugins_dir.join("installed_plugins.json");
        let settings_json = claude_dir.join("settings.json");

        let debounce_active: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                let event = match res {
                    Ok(e) => e,
                    Err(_) => return,
                };

                let relevant =
                    event.paths.iter().any(|p| p == &installed_plugins || p == &settings_json);
                if !relevant {
                    return;
                }

                if debounce_active.load(Ordering::Relaxed) {
                    return;
                }
                debounce_active.store(true, Ordering::Relaxed);

                let em = emitter.clone();
                let da = debounce_active.clone();

                std::thread::Builder::new()
                    .name("yeek-plugin-config-debounce".into())
                    .spawn(move || {
                        std::thread::sleep(Duration::from_millis(500));
                        da.store(false, Ordering::Relaxed);

                        em.emit_plugin_config_changed();
                        tracing::info!("Plugin config changed, emitted event");
                    })
                    .ok();
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| {
            AppError::Internal(format!("Failed to create plugin config watcher: {}", e))
        })?;

        // Watch ~/.claude/ (non-recursive) for settings.json
        watcher
            .watch(&claude_dir, RecursiveMode::NonRecursive)
            .map_err(|e| AppError::Internal(format!("Failed to watch ~/.claude/: {}", e)))?;

        // Watch ~/.claude/plugins/ (non-recursive) for installed_plugins.json
        watcher.watch(&plugins_dir, RecursiveMode::NonRecursive).map_err(|e| {
            AppError::Internal(format!("Failed to watch ~/.claude/plugins/: {}", e))
        })?;

        tracing::info!("Plugin config watcher started");

        Ok(Self { _watcher: watcher })
    }
}

fn run_incremental_scan(
    db_path: &Path,
    changed_paths: &[PathBuf],
    emitter: &dyn EventEmitter,
) -> Result<(), AppError> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| AppError::DbError(e.to_string()))?;
    schema::init_schema(&conn)?;

    // Route changed paths to the correct adapter
    const MAX_WATCHER_FILE_SIZE: u64 = 10 * 1024 * 1024;
    let mut claude_sources = Vec::new();
    let mut codex_sources = Vec::new();

    for p in changed_paths {
        let meta = match std::fs::metadata(p) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.len() > MAX_WATCHER_FILE_SIZE {
            tracing::info!(
                "Watcher: skipping large file ({}MB): {}",
                meta.len() / 1024 / 1024,
                p.display()
            );
            continue;
        }

        if let Some(source) = codex::source_descriptor_from_path(p) {
            codex_sources.push(source);
        } else if let Some(source) = claudecode::source_descriptor_from_path(p) {
            claude_sources.push(source);
        }
    }

    if claude_sources.is_empty() && codex_sources.is_empty() {
        return Ok(());
    }

    tracing::info!(
        "Watcher: indexing {} claude + {} codex sources",
        claude_sources.len(),
        codex_sources.len()
    );

    // Let each adapter's fingerprint logic decide skip/update
    let claude_result = claudecode::index_sources(&conn, &claude_sources, |_| {})?;
    let codex_result = codex::index_sources(&conn, &codex_sources, |_| {})?;

    emitter.emit_sync_completed(SyncCompletedPayload {
        sessions_indexed: claude_result.indexed + codex_result.indexed,
        sessions_updated: claude_result.updated + codex_result.updated,
        errors: claude_result.errors + codex_result.errors,
    });

    Ok(())
}
