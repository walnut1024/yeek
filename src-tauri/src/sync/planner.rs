use rusqlite::Connection;

use crate::adapter::claudecode;
use crate::app::errors::AppError;

pub fn run_startup_sync(conn: &Connection) -> Result<SyncSummary, AppError> {
    log::info!("Starting sync: discovering Claude Code sources...");

    let result = claudecode::index_all(conn)?;

    log::info!(
        "Sync completed: indexed={}, errors={}",
        result.indexed,
        result.errors
    );

    Ok(SyncSummary {
        sessions_indexed: result.indexed,
        sessions_updated: result.updated,
        errors: result.errors,
    })
}

pub struct SyncSummary {
    pub sessions_indexed: i64,
    pub sessions_updated: i64,
    pub errors: i64,
}
