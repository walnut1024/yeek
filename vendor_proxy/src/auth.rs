//! Provider-side authentication policy.

use crate::client::AuthHeaders;
use crate::config::ApiFormat;

pub fn provider_api_key(api_key_env: Option<&str>) -> Option<String> {
    api_key_env.and_then(|name| std::env::var(name).ok())
}

pub fn provider_auth(format: &ApiFormat, api_key: Option<&str>) -> AuthHeaders {
    match format {
        ApiFormat::AnthropicMessages => AuthHeaders::anthropic(api_key),
        ApiFormat::ChatCompletions | ApiFormat::Responses => AuthHeaders::bearer(api_key),
    }
}
