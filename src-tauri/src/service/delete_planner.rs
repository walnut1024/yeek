use serde::Serialize;

use crate::app::errors::AppError;
use crate::domain::source::{DeletePolicy, SourceRef};
use crate::store::sources;

/// Validate that a path is safe to delete: must be within an allowed directory
/// and must not be a symlink.
fn validate_delete_path(path: &std::path::Path) -> Result<(), String> {
    // Check for symlinks
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err("Symlinks cannot be deleted".to_string());
        }
    }

    // Canonicalize to resolve any .. components
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => {
            // File doesn't exist yet — validate the parent path
            return Ok(());
        },
    };

    let home = dirs::home_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;

    let allowed_prefixes: Vec<std::path::PathBuf> = vec![
        home.join(".claude").join("projects"),
        home.join(".codex").join("sessions"),
        home.join(".codex").join("archived_sessions"),
    ];

    let allowed = allowed_prefixes.iter().any(|prefix| {
        let canonical_prefix = std::fs::canonicalize(prefix).unwrap_or_else(|_| prefix.clone());
        canonical.starts_with(&canonical_prefix)
    });

    if !allowed {
        return Err(format!("Path is outside allowed directories: {}", canonical.display()));
    }

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DeletePlan {
    pub session_id: String,
    pub sources: Vec<SourceDeletePlan>,
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SourceDeletePlan {
    pub source: SourceRef,
    pub can_delete: bool,
    pub target_path: String,
    pub reason: String,
}

pub(crate) fn resolve_delete_plan(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<DeletePlan, AppError> {
    let source_refs = sources::get_session_sources(conn, session_id)?;

    let mut source_plans = Vec::new();
    let mut all_safe = true;

    for src in source_refs {
        let (can_delete, reason) = match &src.delete_policy {
            DeletePolicy::FileSafe => {
                // Check if file actually exists
                let exists = std::path::Path::new(&src.path).exists();
                if exists {
                    (true, "Source file exists and is safe to delete".to_string())
                } else {
                    // File missing — still allow deletion to clean up metadata
                    (true, "Source file no longer exists (metadata cleanup only)".to_string())
                }
            },
            DeletePolicy::NotAllowed => {
                all_safe = false;
                (false, "Delete not allowed for this source type".to_string())
            },
            DeletePolicy::HideOnly => {
                all_safe = false;
                (false, "This source can only be hidden, not deleted".to_string())
            },
            DeletePolicy::NeedsReview => {
                all_safe = false;
                (false, "Source requires manual review before deletion".to_string())
            },
        };

        source_plans.push(SourceDeletePlan {
            source: src.clone(),
            can_delete,
            target_path: src.path.clone(),
            reason,
        });
    }

    let deletable_count = source_plans.iter().filter(|p| p.can_delete).count();
    let (allowed, reason) = if source_plans.is_empty() {
        (true, "No source files to delete".to_string())
    } else if all_safe {
        (true, "All sources are safe to delete".to_string())
    } else if deletable_count > 0 {
        (true, format!("{} of {} sources can be deleted", deletable_count, source_plans.len()))
    } else {
        (false, "No sources can be safely deleted".to_string())
    };

    Ok(DeletePlan { session_id: session_id.to_string(), sources: source_plans, allowed, reason })
}

pub(crate) fn execute_destructive_delete(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<DestructiveDeleteResult, AppError> {
    let plan = resolve_delete_plan(conn, session_id)?;

    if !plan.allowed {
        return Err(AppError::DeleteNotSafe(format!("Delete blocked: {}", plan.reason)));
    }

    let mut deleted_files = 0i64;
    let mut failed_files = 0i64;
    let mut errors = Vec::new();

    for src_plan in &plan.sources {
        if !src_plan.can_delete {
            continue;
        }

        let path = std::path::Path::new(&src_plan.target_path);

        // Validate path is within ~/.claude/projects/ to prevent traversal
        if let Err(e) = validate_delete_path(path) {
            failed_files += 1;
            errors.push(format!("{}: {}", src_plan.target_path, e));
            continue;
        }

        if path.exists() {
            match std::fs::remove_file(path) {
                Ok(_) => deleted_files += 1,
                Err(e) => {
                    failed_files += 1;
                    errors.push(format!("{}: {}", src_plan.target_path, e));
                },
            }
        }
    }

    // Record action before deleting the session row
    // Write tombstones for ALL sources (not just physically deleted ones)
    // so that rescans won't re-import them.
    for src_plan in &plan.sources {
        if let Err(e) = sources::mark_source_deleted(conn, &src_plan.target_path) {
            errors.push(format!("tombstone {}: {}", src_plan.target_path, e));
        }
    }

    crate::store::actions::record_action(
        conn,
        Some(session_id),
        "destructive_delete",
        Some(&format!(
            "deleted={}, failed={}, errors={}",
            deleted_files,
            failed_files,
            errors.len()
        )),
    )?;

    // Delete session row (CASCADE removes messages, sources, etc.)
    conn.execute(
        "DELETE FROM sessions WHERE id = ?1",
        rusqlite::params![session_id],
    )?;

    Ok(DestructiveDeleteResult { success: failed_files == 0, deleted_files, failed_files, errors })
}

#[derive(Debug, Serialize)]
pub struct DestructiveDeleteResult {
    pub success: bool,
    pub deleted_files: i64,
    pub failed_files: i64,
    pub errors: Vec<String>,
}
