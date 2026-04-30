//! Application state shared across Tauri commands and HTTP handlers.
//!
//! # Database access pattern
//!
//! The SQLite connection is wrapped in [`std::sync::Mutex`] (not [`tokio::sync::Mutex`]).
//! This is intentional:
//!
//! - SQLite operations are synchronous and fast (local file or in-memory)
//! - Lock contention is minimal — each request acquires, queries, releases
//! - [`std::sync::Mutex`] provides better performance for uncontended locks
//! - The lock is **never** held across `.await` points (see §8 of CONTRIBUTING.md)
//!
//! Callers acquire the lock via [`AppState::db()`] and the returned [`MutexGuard`]
//! must be dropped before any async operation.

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    pub proxy_manager: crate::app::proxy::ProxyManager,
}

impl AppState {
    pub fn new(
        conn: Connection,
        db_path: PathBuf,
        emitter: Arc<dyn EventEmitter>,
        proxy_manager: crate::app::proxy::ProxyManager,
    ) -> Self {
        Self {
            db: Mutex::new(conn),
            event_emitter: emitter,
            scan_guard: Arc::new(ScanGuard::new()),
            db_path,
            watcher: None,
            config_watcher: None,
            proxy_manager,
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

    pub(crate) fn db(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.db.lock().map_err(|e| AppError::Internal(e.to_string()))
    }
}
