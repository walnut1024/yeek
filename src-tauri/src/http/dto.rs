use serde::Deserialize;

#[derive(Deserialize)]
pub struct BrowseQuery {
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct ActionLogQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct SoftDeleteRequest {
    pub ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct SoftDeleteProjectRequest {
    pub project_path: String,
}

#[derive(Deserialize)]
pub struct ResumeRequest {
    pub session_id: String,
    pub agent: String,
    pub cwd: Option<String>,
}

#[derive(Deserialize)]
pub struct PluginActionRequest {
    pub key: String,
}

#[derive(Deserialize)]
pub struct PluginScopeQuery {
    pub scope: String,
}

#[derive(Deserialize)]
pub struct AddMarketplaceRequest {
    pub name: String,
    pub repo: String,
}

#[derive(Deserialize)]
pub struct RemoveMarketplaceRequest {
    pub remove_plugins: Option<bool>,
}

#[derive(Deserialize)]
pub struct InstallMarketplacePluginRequest {
    pub marketplace_name: String,
    pub plugin_name: String,
}
