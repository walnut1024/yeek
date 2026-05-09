//! Model name mapping with family-based fallback.
//!
//! Maps incoming model names (e.g. claude-sonnet-4-20250514) to provider-specific
//! names (e.g. deepseek-v4-pro) using the provider's `model_map` config.

use std::collections::HashMap;

/// Map a model name using exact match, then family-based fallback.
///
/// 1. Exact match in model_map
/// 2. Family match: haiku/sonnet/opus keyword → "default" key
/// 3. "default" key fallback
/// 4. Return original if no mapping found
pub fn map_model(original: &str, model_map: &HashMap<String, String>) -> String {
    if model_map.is_empty() {
        return original.to_string();
    }

    // 1. Exact match
    if let Some(mapped) = model_map.get(original) {
        return mapped.clone();
    }

    // 2. Family-based fallback
    let lower = original.to_lowercase();
    let family_key = if lower.contains("haiku") {
        "haiku"
    } else if lower.contains("opus") {
        "opus"
    } else if lower.contains("sonnet") {
        "sonnet"
    } else {
        ""
    };

    if !family_key.is_empty() {
        // Try family-specific keys first
        for (key, value) in model_map {
            if key.to_lowercase().contains(family_key) {
                return value.clone();
            }
        }
    }

    // 3. "default" fallback
    if let Some(mapped) = model_map.get("default") {
        return mapped.clone();
    }

    // 4. No mapping
    original.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("claude-sonnet-4-20250514".into(), "deepseek-v4-pro".into());
        m.insert("claude-haiku-4-5-20251001".into(), "deepseek-v4-flash".into());
        m.insert("default".into(), "deepseek-v4-pro".into());
        m
    }

    #[test]
    fn exact_match() {
        let map = make_map();
        assert_eq!(map_model("claude-sonnet-4-20250514", &map), "deepseek-v4-pro");
    }

    #[test]
    fn family_fallback_sonnet() {
        let map = make_map();
        assert_eq!(map_model("claude-sonnet-4-6", &map), "deepseek-v4-pro");
    }

    #[test]
    fn family_fallback_haiku() {
        let map = make_map();
        assert_eq!(map_model("claude-haiku-4-5", &map), "deepseek-v4-flash");
    }

    #[test]
    fn default_fallback() {
        let map = make_map();
        assert_eq!(map_model("claude-opus-4-7", &map), "deepseek-v4-pro");
    }

    #[test]
    fn no_match_returns_original() {
        let mut map = HashMap::new();
        map.insert("claude-sonnet-4-20250514".into(), "deepseek-v4-pro".into());
        assert_eq!(map_model("gpt-5.4", &map), "gpt-5.4");
    }

    #[test]
    fn empty_map_returns_original() {
        let map = HashMap::new();
        assert_eq!(map_model("claude-sonnet-4-6", &map), "claude-sonnet-4-6");
    }
}
