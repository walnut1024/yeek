use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use tauri::AppHandle;

use crate::app::errors::AppError;
use crate::sync::background::ScanGuard;
use crate::sync::watcher::FileWatcher;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub app_handle: Option<AppHandle>,
    pub scan_guard: Arc<ScanGuard>,
    pub db_path: PathBuf,
    pub watcher: Option<FileWatcher>,
    pub config_watcher: Option<FileWatcher>,
}

impl AppState {
    pub fn new(conn: Connection, db_path: PathBuf) -> Self {
        Self {
            db: Mutex::new(conn),
            app_handle: None,
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

    pub fn with_handle(mut self, handle: AppHandle) -> Self {
        self.app_handle = Some(handle);
        self
    }

    pub fn db(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.db.lock().map_err(|e| AppError::Internal(e.to_string()))
    }

    pub fn app_handle(&self) -> Option<&AppHandle> {
        self.app_handle.as_ref()
    }
}
