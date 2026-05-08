//! Proxy configuration: providers, server bind address, API keys.

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
    /// Map incoming model names to provider-specific names.
    /// e.g. {"gpt-5.4": "deepseek-v4-pro"} means requests for "gpt-5.4"
    /// are sent to this provider as "deepseek-v4-pro".
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
    /// Load and parse `proxy.toml`, then validate the configuration.
    ///
    /// # Errors
    /// Returns an error if the file is missing, unparseable, or fails validation
    /// (e.g., empty providers list, missing default provider).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate that the configuration is well-formed.
    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.providers.is_empty() {
            return Err("proxy.toml: providers must not be empty".into());
        }
        if !self.providers.contains_key(&self.default_provider) {
            return Err(format!(
                "proxy.toml: default_provider '{}' not found in [providers] table",
                self.default_provider
            )
            .into());
        }
        Ok(())
    }

    /// Look up a provider by name.
    pub fn provider_by_name(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// Get the default provider config.
    ///
    /// # Panics
    /// Only if validation was skipped. After [`load`], this is guaranteed to succeed.
    pub fn default_provider(&self) -> &ProviderConfig {
        self.providers
            .get(&self.default_provider)
            .expect("default_provider must exist (validated in load)")
    }
}
