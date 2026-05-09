//! Proxy configuration: proxy pairs, providers, server bind address, API keys.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub proxy_pairs: HashMap<String, ProxyPair>,
    // Legacy fields (backward compat when proxy_pairs is empty)
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

/// A single proxy relationship: Agent ↔ Provider with format translation.
#[derive(Debug, Clone, Deserialize)]
pub struct ProxyPair {
    /// URL path prefix for agent-side routing (e.g. "/anthropic").
    pub route_path: String,
    /// API format the agent sends.
    pub route_format: ApiFormat,
    /// Provider base URL to forward requests to.
    pub provider_base_url: String,
    /// API format the provider expects.
    pub provider_format: ApiFormat,
    /// Environment variable name holding the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Model name mapping: agent model → provider model.
    #[serde(default)]
    pub model_map: HashMap<String, String>,
}

/// API format on either side of the proxy.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    AnthropicMessages,
    ChatCompletions,
    Responses,
}

// Legacy provider config (backward compat)
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub format: ProviderFormat,
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub model_map: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFormat {
    AnthropicMessages,
    ChatCompletions,
}

impl ProxyConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.proxy_pairs.is_empty() {
            // New format: validate pairs
            for (name, pair) in &self.proxy_pairs {
                if !pair.route_path.starts_with('/') {
                    return Err(format!(
                        "proxy_pairs.{}: route_path must start with '/', got '{}'",
                        name, pair.route_path
                    )
                    .into());
                }
                // Reject unsupported format combinations at startup
                match (&pair.route_format, &pair.provider_format) {
                    (ApiFormat::Responses, ApiFormat::AnthropicMessages) => {
                        return Err(format!(
                            "proxy_pairs.{}: Responses → AnthropicMessages translation not supported",
                            name
                        )
                        .into());
                    }
                    (ApiFormat::ChatCompletions, ApiFormat::AnthropicMessages) => {
                        return Err(format!(
                            "proxy_pairs.{}: ChatCompletions → AnthropicMessages translation not supported",
                            name
                        )
                        .into());
                    }
                    _ => {}
                }
            }
        } else if !self.providers.is_empty() {
            // Legacy format: validate providers
            if let Some(ref default) = self.default_provider {
                if !self.providers.contains_key(default) {
                    return Err(format!(
                        "default_provider '{}' not found in [providers]",
                        default
                    )
                    .into());
                }
            }
        } else {
            return Err("config must have either [proxy_pairs] or [providers]".into());
        }
        Ok(())
    }

    /// Whether using new proxy_pairs format.
    pub fn uses_pairs(&self) -> bool {
        !self.proxy_pairs.is_empty()
    }

    // Legacy helpers

    pub fn provider_by_name(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    pub fn default_provider(&self) -> &ProviderConfig {
        self.providers
            .get(self.default_provider.as_deref().unwrap_or(""))
            .expect("default_provider must exist (validated in load)")
    }
}
