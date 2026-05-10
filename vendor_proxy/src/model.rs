//! Strict bridge-local model mapping and response model restoration.

use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ModelPolicy<'a> {
    bridge_name: &'a str,
    models: &'a HashMap<String, String>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("model '{agent_model}' is not configured for bridge '{bridge_name}'")]
pub struct UnknownModel {
    pub bridge_name: String,
    pub agent_model: String,
}

impl<'a> ModelPolicy<'a> {
    pub fn new(bridge_name: &'a str, models: &'a HashMap<String, String>) -> Self {
        Self { bridge_name, models }
    }

    pub fn resolve_provider_model(&self, agent_model: &str) -> Result<&'a str, UnknownModel> {
        self.models.get(agent_model).map(String::as_str).ok_or_else(|| UnknownModel {
            bridge_name: self.bridge_name.to_string(),
            agent_model: agent_model.to_string(),
        })
    }

    pub fn agent_models(&self) -> Vec<String> {
        let mut models: Vec<_> = self.models.keys().cloned().collect();
        models.sort();
        models
    }
}

pub fn restore_model_fields(value: &mut Value, provider_model: &str, agent_model: &str) -> usize {
    let aliases = provider_model_aliases(provider_model);
    restore_model_fields_matching(value, &aliases, agent_model)
}

fn restore_model_fields_matching(
    value: &mut Value,
    provider_models: &[String],
    agent_model: &str,
) -> usize {
    let mut restored = 0;
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "model"
                    && child.as_str().is_some_and(|model| {
                        provider_models.iter().any(|candidate| candidate == model)
                    })
                {
                    *child = Value::String(agent_model.to_string());
                    restored += 1;
                } else {
                    restored += restore_model_fields_matching(child, provider_models, agent_model);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                restored += restore_model_fields_matching(item, provider_models, agent_model);
            }
        }
        _ => {}
    }
    restored
}

fn provider_model_aliases(provider_model: &str) -> Vec<String> {
    let mut aliases = vec![provider_model.to_string()];
    if let Some((base, _)) = provider_model.split_once('[') {
        let base = base.trim_end();
        if !base.is_empty() && base != provider_model {
            aliases.push(base.to_string());
        }
    }
    aliases
}

pub fn restore_model_in_sse_data(
    data: &str,
    provider_model: &str,
    agent_model: &str,
) -> (String, usize) {
    let Ok(mut value) = serde_json::from_str::<Value>(data) else {
        return (data.to_string(), 0);
    };
    let restored = restore_model_fields(&mut value, provider_model, agent_model);
    (serde_json::to_string(&value).unwrap_or_else(|_| data.to_string()), restored)
}
