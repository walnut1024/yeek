//! Persist proxy configuration in SQLite (GUI mode).
//!
//! The `proxy_config` table holds a single row (id = 1) with the full
//! proxy config serialized as JSON. This is the authoritative source
//! when llm-proxy is managed from the GUI. Standalone CLI mode uses
//! the TOML file instead.

use rusqlite::Connection;

use crate::app::errors::AppError;
use crate::app::proxy::ProxyConfig;

/// Read the proxy configuration from the database.
/// Returns `NotFound` if no config has been saved yet.
pub fn read_proxy_config(conn: &Connection) -> Result<ProxyConfig, AppError> {
    let json: String = conn
        .query_row(
            "SELECT config_json FROM proxy_config WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("proxy config not found in database".into())
            }
            _ => AppError::DbError(e.to_string()),
        })?;
    serde_json::from_str(&json).or_else(|e| {
        tracing::warn!(
            "stored proxy config is not compatible with bridge schema; resetting to default config: {}",
            e
        );
        let config = ProxyConfig::default();
        write_proxy_config(conn, &config)?;
        Ok(config)
    })
}

/// Write (insert or replace) the proxy configuration into the database.
pub fn write_proxy_config(conn: &Connection, config: &ProxyConfig) -> Result<(), AppError> {
    let json =
        serde_json::to_string(config).map_err(|e| AppError::Internal(format!("json: {}", e)))?;
    conn.execute(
        "INSERT OR REPLACE INTO proxy_config (id, config_json) VALUES (1, ?1)",
        [&json],
    )?;
    Ok(())
}
