use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceRef {
    pub source_id: String,
    pub source_type: String,
    pub path: String,
    pub delete_policy: DeletePolicy,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DeletePolicy {
    NotAllowed,
    HideOnly,
    FileSafe,
    NeedsReview,
}

impl DeletePolicy {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            DeletePolicy::NotAllowed => "not_allowed",
            DeletePolicy::HideOnly => "hide_only",
            DeletePolicy::FileSafe => "file_safe",
            DeletePolicy::NeedsReview => "needs_review",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceDescriptor {
    pub source_type: String,
    pub path: String,
    pub agent: String,
    pub fingerprint: String,
    pub last_modified: String,
}
