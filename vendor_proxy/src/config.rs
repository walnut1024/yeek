use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub server: ServerConfig,
    pub default_provider: String,
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub format: ProviderFormat,
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFormat {
    AnthropicMessages,
    ChatCompletions,
}

impl ProxyConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn provider_by_name(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    pub fn default_provider(&self) -> &ProviderConfig {
        self.providers
            .get(&self.default_provider)
            .unwrap_or_else(|| {
                panic!(
                    "default_provider '{}' not found in providers",
                    self.default_provider
                )
            })
    }
}
