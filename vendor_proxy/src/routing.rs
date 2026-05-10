//! Bridge route matching and model discovery helpers.

use crate::config::{BridgeConfig, ProviderConfig, ProxyConfig};

pub fn find_bridge_for_request_path<'a>(
    config: &'a ProxyConfig,
    path: &str,
) -> Option<(&'a str, &'a BridgeConfig, &'a ProviderConfig)> {
    config.bridges.iter().find_map(|(name, bridge)| {
        let base = bridge.agent.base_url.trim_end_matches('/');
        let expected = format!("{}{}", base, bridge.agent.api_format.endpoint_path());
        if path == expected {
            let provider = config.bridge_provider(bridge)?;
            Some((name.as_str(), bridge, provider))
        } else {
            None
        }
    })
}

pub fn find_bridge_for_models_path<'a>(
    config: &'a ProxyConfig,
    path: &str,
) -> Option<(&'a str, &'a BridgeConfig)> {
    config.bridges.iter().find_map(|(name, bridge)| {
        let base = bridge.agent.base_url.trim_end_matches('/');
        let expected = format!("{}/v1/models", base);
        (path == expected).then_some((name.as_str(), bridge))
    })
}
