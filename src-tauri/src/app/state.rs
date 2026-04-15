use std::sync::Mutex;
use rusqlite::Connection;
use tauri::AppHandle;

use crate::app::errors::AppError;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub app_handle: Option<AppHandle>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
            app_handle: None,
        }
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
