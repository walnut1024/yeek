use std::collections::HashMap;

use vendor_proxy::auth::provider_api_key;
use vendor_proxy::config::{ApiFormat, ProxyConfig};
use vendor_proxy::logging::{header_names, provider_error_preview, request_body_summary};
use vendor_proxy::model::{restore_model_in_sse_data, ModelPolicy};
use vendor_proxy::routing::{find_bridge_for_models_path, find_bridge_for_request_path};

const VALID_CONFIG: &str = r#"
[server]
listen_addr = "127.0.0.1:8787"

[bridges.claude_desktop_deepseek.agent]
base_url = "/deepseek"
api_format = "anthropic_messages"

[bridges.claude_desktop_deepseek.provider]
name = "deepseek_anthropic"

[bridges.claude_desktop_deepseek.models]
"claude-sonnet" = "deepseek-v4-pro[1m]"
"claude-haiku" = "deepseek-v4-flash"
"claude-opus" = "deepseek-v4-pro[1m]"

[providers.deepseek_anthropic]
base_url = "https://api.deepseek.com/anthropic"
api_format = "anthropic_messages"
api_key_env = "DEEPSEEK_API_KEY"
"#;

#[test]
fn loads_bridge_based_config() {
    let config = ProxyConfig::from_toml_str(VALID_CONFIG).expect("config should load");
    let bridge = config.bridges.get("claude_desktop_deepseek").expect("bridge");
    let provider = config.providers.get("deepseek_anthropic").expect("provider");

    assert_eq!(bridge.agent.base_url, "/deepseek");
    assert_eq!(bridge.agent.api_format, ApiFormat::AnthropicMessages);
    assert_eq!(bridge.provider.name, "deepseek_anthropic");
    assert_eq!(provider.api_format, ApiFormat::AnthropicMessages);
    assert_eq!(bridge.models.get("claude-sonnet").map(String::as_str), Some("deepseek-v4-pro[1m]"));
}

#[test]
fn rejects_unknown_provider_reference() {
    let config = VALID_CONFIG.replace("name = \"deepseek_anthropic\"", "name = \"missing\"");
    let err = ProxyConfig::from_toml_str(&config).expect_err("missing provider should fail");
    assert!(err.to_string().contains("provider 'missing' not found"));
}

#[test]
fn rejects_ambiguous_bridge_paths() {
    let config = format!(
        r#"{VALID_CONFIG}

[bridges.review.agent]
base_url = "/deepseek/review"
api_format = "anthropic_messages"

[bridges.review.provider]
name = "deepseek_anthropic"

[bridges.review.models]
"claude-sonnet" = "deepseek-v4-pro[1m]"
"#
    );
    let err = ProxyConfig::from_toml_str(&config).expect_err("path prefix should fail");
    assert!(err.to_string().contains("ambiguous bridge paths"));
}

#[test]
fn rejects_non_anthropic_provider_format_for_current_scope() {
    let config = VALID_CONFIG.replace(
        "api_format = \"anthropic_messages\"\napi_key_env = \"DEEPSEEK_API_KEY\"",
        "api_format = \"chat_completions\"\napi_key_env = \"DEEPSEEK_API_KEY\"",
    );
    let err = ProxyConfig::from_toml_str(&config).expect_err("chat provider should fail");
    assert!(err.to_string().contains("unsupported format pair"));
}

#[test]
fn strict_model_policy_rejects_unconfigured_models() {
    let mut models = HashMap::new();
    models.insert("claude-sonnet".to_string(), "deepseek-v4-pro[1m]".to_string());
    let policy = ModelPolicy::new("claude_deepseek", &models);

    assert_eq!(policy.resolve_provider_model("claude-sonnet").unwrap(), "deepseek-v4-pro[1m]");

    let err = policy.resolve_provider_model("claude-opus").unwrap_err();
    assert_eq!(err.agent_model, "claude-opus");
    assert_eq!(err.bridge_name, "claude_deepseek");
}

#[test]
fn restores_only_json_sse_model_field() {
    let input = r#"{"type":"message_start","message":{"model":"deepseek-v4-pro[1m]","content":[{"text":"deepseek-v4-pro[1m] should remain"}]}}"#;
    let (restored, restored_count) =
        restore_model_in_sse_data(input, "deepseek-v4-pro[1m]", "claude-sonnet");
    let value: serde_json::Value = serde_json::from_str(&restored).expect("json");

    assert_eq!(restored_count, 1);
    assert_eq!(value["message"]["model"], "claude-sonnet");
    assert_eq!(value["message"]["content"][0]["text"], "deepseek-v4-pro[1m] should remain");
}

#[test]
fn restores_model_alias_without_provider_suffix() {
    let input = r#"{"model":"deepseek-v4-pro","content":[{"type":"text","text":"ok"}]}"#;
    let (restored, restored_count) =
        restore_model_in_sse_data(input, "deepseek-v4-pro[1m]", "claude-sonnet");
    let value: serde_json::Value = serde_json::from_str(&restored).expect("json");

    assert_eq!(restored_count, 1);
    assert_eq!(value["model"], "claude-sonnet");
}

#[test]
fn routes_match_only_their_configured_endpoint() {
    let config = ProxyConfig::from_toml_str(VALID_CONFIG).expect("config should load");

    let (name, bridge, provider) =
        find_bridge_for_request_path(&config, "/deepseek/v1/messages").expect("route");
    assert_eq!(name, "claude_desktop_deepseek");
    assert_eq!(bridge.agent.api_format, ApiFormat::AnthropicMessages);
    assert_eq!(provider.base_url, "https://api.deepseek.com/anthropic");

    assert!(find_bridge_for_request_path(&config, "/deepseek/v1/responses").is_none());
    assert!(find_bridge_for_request_path(&config, "/unknown/v1/messages").is_none());
}

#[test]
fn bridge_models_path_exposes_agent_models_only() {
    let config = ProxyConfig::from_toml_str(VALID_CONFIG).expect("config should load");
    let (_, bridge) = find_bridge_for_models_path(&config, "/deepseek/v1/models").expect("models");
    let models = ModelPolicy::new("claude_desktop_deepseek", &bridge.models).agent_models();

    assert_eq!(
        models,
        vec!["claude-haiku".to_string(), "claude-opus".to_string(), "claude-sonnet".to_string()]
    );
    assert!(!models.contains(&"deepseek-v4-pro[1m]".to_string()));
}

#[test]
fn provider_key_comes_from_configured_env() {
    std::env::set_var("VENDOR_PROXY_TEST_KEY", "provider-secret");
    assert_eq!(provider_api_key(Some("VENDOR_PROXY_TEST_KEY")).as_deref(), Some("provider-secret"));
    assert_eq!(provider_api_key(Some("VENDOR_PROXY_MISSING_KEY")), None);
    assert_eq!(provider_api_key(None), None);
    std::env::remove_var("VENDOR_PROXY_TEST_KEY");
}

#[test]
fn debug_header_names_redact_sensitive_values() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", "Bearer secret-token".parse().unwrap());
    headers.insert("x-api-key", "secret-api-key".parse().unwrap());
    headers.insert("anthropic-version", "2023-06-01".parse().unwrap());

    let summary = header_names(&headers);

    assert!(summary.contains("authorization"));
    assert!(summary.contains("x-api-key"));
    assert!(summary.contains("anthropic-version"));
    assert!(!summary.contains("secret-token"));
    assert!(!summary.contains("secret-api-key"));
}

#[test]
fn debug_request_body_summary_omits_prompt_content() {
    let body = r#"{
        "model": "claude-sonnet",
        "stream": true,
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "do not log this private prompt"}]
    }"#;

    let summary = request_body_summary(body);

    assert!(summary.contains("bytes="));
    assert!(summary.contains("model=claude-sonnet"));
    assert!(summary.contains("stream=true"));
    assert!(summary.contains("max_tokens=1024"));
    assert!(!summary.contains("do not log this private prompt"));
}

#[test]
fn provider_error_preview_is_truncated() {
    let body = format!("{}{}", "x".repeat(1100), "sensitive-tail");
    let preview = provider_error_preview(&body);

    assert!(preview.len() < body.len());
    assert!(preview.contains("truncated"));
    assert!(!preview.contains("sensitive-tail"));
}
