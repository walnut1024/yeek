use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginInfo {
    pub key: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub marketplace: Option<MarketplaceInfo>,
    pub install_path: String,
    pub enabled: bool,
    pub health: String,
    pub health_issues: Vec<String>,
    pub skills: Vec<SkillInfo>,
    pub agents: Vec<SkillInfo>,
    pub installed_at: Option<String>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub skill_type: String,
    pub tools: Option<String>,
    pub file_path: String,
    pub health: String,
    pub health_detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketplaceInfo {
    pub name: String,
    pub repo: String,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillsOverview {
    pub plugins: Vec<PluginInfo>,
    pub total_plugins: usize,
    pub total_skills: usize,
    pub total_agents: usize,
    pub health_summary: HealthSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthSummary {
    pub ok: usize,
    pub partial: usize,
    pub hook: usize,
    pub broken: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FixPluginResult {
    pub action: String,
    pub message: String,
}
