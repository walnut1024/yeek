use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::app::errors::AppError;
use crate::domain::session::{DeleteMode, SessionRecord, SessionStatus, VisibilityStatus};
use crate::domain::source::SourceDescriptor;
use crate::store::messages::MessageRecord;
use crate::store::sessions;

// ── Discovery ──

/// Resolve candidate OpenCode data directories.
fn opencode_data_dirs() -> Vec<PathBuf> {
    let mut dirs_list = Vec::new();

    // OPENCODE_DB override
    if let Ok(val) = std::env::var("OPENCODE_DB") {
        if Path::new(&val).is_absolute() {
            if let Some(parent) = Path::new(&val).parent() {
                dirs_list.push(parent.to_path_buf());
            }
        }
    }

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return dirs_list,
    };

    // macOS: ~/Library/Application Support/opencode
    dirs_list.push(home.join("Library/Application Support/opencode"));

    // XDG_DATA_HOME/opencode
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        dirs_list.push(PathBuf::from(xdg).join("opencode"));
    }

    // Fallback: ~/.local/share/opencode
    dirs_list.push(home.join(".local/share/opencode"));

    dirs_list
}

pub(crate) fn discover_sources() -> Result<Vec<SourceDescriptor>, AppError> {
    let mut sources = Vec::new();
    for data_dir in opencode_data_dirs() {
        if !data_dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(source) = source_descriptor_from_path(&path) {
                    sources.push(source);
                }
            }
        }
    }
    Ok(sources)
}

pub(crate) fn source_descriptor_from_path(path: &Path) -> Option<SourceDescriptor> {
    let file_name = path.file_name().and_then(|n| n.to_str())?;
    if file_name != "opencode.db"
        && !(file_name.starts_with("opencode-") && file_name.ends_with(".db"))
    {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }

    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            DateTime::from_timestamp(d.as_secs() as i64, 0)
                .unwrap_or_default()
                .to_rfc3339()
        })
        .unwrap_or_default();

    Some(SourceDescriptor {
        source_type: "opencode_db".to_string(),
        path: path.to_string_lossy().to_string(),
        agent: "opencode".to_string(),
        fingerprint: compute_fingerprint(&path),
        last_modified: modified,
    })
}

pub(crate) fn compute_fingerprint(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) => {
            let len = m.len();
            let modified = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("{}:{}", len, modified)
        }
        Err(_) => "unknown".to_string(),
    }
}

// ── Indexing ──

pub struct IndexResult {
    pub indexed: i64,
    pub updated: i64,
    pub errors: i64,
}

pub(crate) fn index_sources<F>(
    _conn: &rusqlite::Connection,
    sources: &[SourceDescriptor],
    _on_progress: F,
) -> Result<IndexResult, AppError>
where
    F: Fn(i64),
{
    // Placeholder — implemented in a later task
    Ok(IndexResult {
        indexed: 0,
        updated: 0,
        errors: sources.len() as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_descriptor_accepts_default_db_name() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode.db");
        std::fs::write(&db_path, b"fake sqlite").unwrap();
        let result = source_descriptor_from_path(&db_path);
        assert!(result.is_some());
        let desc = result.unwrap();
        assert_eq!(desc.source_type, "opencode_db");
        assert_eq!(desc.agent, "opencode");
    }

    #[test]
    fn source_descriptor_accepts_channel_db_name() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode-beta.db");
        std::fs::write(&db_path, b"fake sqlite").unwrap();
        let result = source_descriptor_from_path(&db_path);
        assert!(result.is_some());
    }

    #[test]
    fn source_descriptor_rejects_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("other.db");
        std::fs::write(&db_path, b"not opencode").unwrap();
        assert!(source_descriptor_from_path(&db_path).is_none());

        let jsonl_path = dir.path().join("session.jsonl");
        std::fs::write(&jsonl_path, b"{}").unwrap();
        assert!(source_descriptor_from_path(&jsonl_path).is_none());
    }

    #[test]
    fn source_descriptor_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("opencode.db");
        std::fs::create_dir(&subdir).unwrap();
        assert!(source_descriptor_from_path(&subdir).is_none());
    }

    #[test]
    fn compute_fingerprint_returns_size_colon_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::fs::write(&path, b"hello world").unwrap();
        let fp = compute_fingerprint(&path);
        assert!(fp.starts_with("11:"));
    }
}
