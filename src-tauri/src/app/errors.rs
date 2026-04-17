use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Database error: {0}")]
    DbError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Source missing: {0}")]
    SourceMissing(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Delete not safe: {0}")]
    DeleteNotSafe(String),

    #[error("Delete failed: {0}")]
    DeleteFailed(String),

    #[error("Hydrate failed: {0}")]
    HydrateFailed(String),

    #[error("Sync failed: {0}")]
    SyncFailed(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct ErrorResponse {
            kind: String,
            message: String,
        }

        let kind = match self {
            AppError::DbError(_) => "db_error",
            AppError::ParseError(_) => "parse_error",
            AppError::SourceMissing(_) => "source_missing",
            AppError::PermissionDenied(_) => "permission_denied",
            AppError::DeleteNotSafe(_) => "delete_not_safe",
            AppError::DeleteFailed(_) => "delete_failed",
            AppError::HydrateFailed(_) => "hydrate_failed",
            AppError::SyncFailed(_) => "sync_failed",
            AppError::NotFound(_) => "not_found",
            AppError::Internal(_) => "internal",
        };

        ErrorResponse {
            kind: kind.to_string(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::DbError(err.to_string())
    }
}
