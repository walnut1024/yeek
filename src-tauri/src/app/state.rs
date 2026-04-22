use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;

use crate::app::errors::AppError;
use crate::app::events::EventEmitter;
use crate::sync::background::ScanGuard;
use crate::sync::watcher::FileWatcher;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub event_emitter: Arc<dyn EventEmitter>,
    pub scan_guard: Arc<ScanGuard>,
    pub db_path: PathBuf,
    pub watcher: Option<FileWatcher>,
    pub config_watcher: Option<FileWatcher>,
}

impl AppState {
    pub fn new(conn: Connection, db_path: PathBuf, emitter: Arc<dyn EventEmitter>) -> Self {
        Self {
            db: Mutex::new(conn),
            event_emitter: emitter,
            scan_guard: Arc::new(ScanGuard::new()),
            db_path,
            watcher: None,
            config_watcher: None,
        }
    }

    pub fn with_watcher(mut self, watcher: FileWatcher) -> Self {
        self.watcher = Some(watcher);
        self
    }

    pub fn with_config_watcher(mut self, watcher: FileWatcher) -> Self {
        self.config_watcher = Some(watcher);
        self
    }

    pub fn db(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.db.lock().map_err(|e| AppError::Internal(e.to_string()))
    }
}
