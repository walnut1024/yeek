use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionRecord {
    pub id: String,
    pub agent: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub status: SessionStatus,
    pub visibility: VisibilityStatus,
    pub pinned: bool,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub delete_mode: DeleteMode,
    pub message_count: i64,
    pub updated_at: String,
    pub parent_session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Complete,
    Partial,
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Complete => "complete",
            SessionStatus::Partial => "partial",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityStatus {
    Visible,
    Hidden,
    Archived,
}

impl VisibilityStatus {
    pub fn as_str(&self) -> &str {
        match self {
            VisibilityStatus::Visible => "visible",
            VisibilityStatus::Hidden => "hidden",
            VisibilityStatus::Archived => "archived",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    None,
    SoftDeleted,
    SourceDeleted,
}

impl DeleteMode {
    pub fn as_str(&self) -> &str {
        match self {
            DeleteMode::None => "none",
            DeleteMode::SoftDeleted => "soft_deleted",
            DeleteMode::SourceDeleted => "source_deleted",
        }
    }
}
