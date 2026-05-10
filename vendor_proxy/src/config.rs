//! Bridge-based proxy configuration.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub bridges: HashMap<String, BridgeConfig>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BridgeConfig {
    pub agent: AgentEndpointConfig,
    pub provider: BridgeProviderRef,
    #[serde(default)]
    pub models: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentEndpointConfig {
    pub base_url: String,
    pub api_format: ApiFormat,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BridgeProviderRef {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_format: ApiFormat,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    AnthropicMessages,
    ChatCompletions,
    Responses,
}

impl ApiFormat {
    pub fn endpoint_path(&self) -> &'static str {
        match self {
            ApiFormat::AnthropicMessages => "/v1/messages",
            ApiFormat::Responses => "/v1/responses",
            ApiFormat::ChatCompletions => "/v1/chat/completions",
        }
    }
}

impl ProxyConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::from_toml_str(&content)
    }

    pub fn from_toml_str(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config: Self = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.bridges.is_empty() {
            return Err("config must define at least one [bridges.*] entry".into());
        }
        if self.providers.is_empty() {
            return Err("config must define at least one [providers.*] entry".into());
        }

        let mut seen_paths = HashSet::new();
        let mut paths: Vec<(String, String)> = Vec::new();

        for (name, bridge) in &self.bridges {
            let path = normalize_agent_base_url(&bridge.agent.base_url)?;
            if !seen_paths.insert(path.clone()) {
                return Err(
                    format!("bridge '{}': duplicate agent base_url '{}'", name, path).into()
                );
            }
            if bridge.models.is_empty() {
                return Err(format!("bridge '{}': models mapping must not be empty", name).into());
            }
            if bridge.models.contains_key("default") {
                return Err(
                    format!("bridge '{}': default model fallback is not supported", name).into()
                );
            }
            let provider = self.providers.get(&bridge.provider.name).ok_or_else(|| {
                format!("bridge '{}': provider '{}' not found", name, bridge.provider.name)
            })?;
            validate_format_pair(name, &bridge.agent.api_format, &provider.api_format)?;
            paths.push((name.clone(), path));
        }

        for i in 0..paths.len() {
            for j in (i + 1)..paths.len() {
                let (left_name, left_path) = (&paths[i].0, &paths[i].1);
                let (right_name, right_path) = (&paths[j].0, &paths[j].1);
                if is_path_prefix(left_path, right_path) || is_path_prefix(right_path, left_path) {
                    return Err(format!(
                        "ambiguous bridge paths: '{}' ({}) conflicts with '{}' ({})",
                        left_path, left_name, right_path, right_name
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    pub fn bridge_provider<'a>(&'a self, bridge: &'a BridgeConfig) -> Option<&'a ProviderConfig> {
        self.providers.get(&bridge.provider.name)
    }
}

fn normalize_agent_base_url(base_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !base_url.starts_with('/') {
        return Err(format!("agent base_url must start with '/', got '{}'", base_url).into());
    }
    let normalized = base_url.trim_end_matches('/');
    if normalized.is_empty() {
        return Err("agent base_url must not be '/'".into());
    }
    Ok(normalized.to_string())
}

fn is_path_prefix(left: &str, right: &str) -> bool {
    right.strip_prefix(left).is_some_and(|suffix| suffix.starts_with('/'))
}

fn validate_format_pair(
    bridge_name: &str,
    agent: &ApiFormat,
    provider: &ApiFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match (agent, provider) {
        (ApiFormat::AnthropicMessages, ApiFormat::AnthropicMessages) => Ok(()),
        _ => Err(format!(
            "bridge '{}': unsupported format pair {:?} -> {:?}",
            bridge_name, agent, provider
        )
        .into()),
    }
}
