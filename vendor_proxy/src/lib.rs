//! vendor_proxy — OpenAI Responses API → multi-provider LLM proxy.
//!
//! Translates between the Responses API format (consumed by Codex TUI) and
//! provider-native formats (Chat Completions, Anthropic Messages).

pub mod adapters;
pub mod auth;
pub mod bridge;
pub mod client;
pub mod config;
pub mod logging;
pub mod model;
pub mod pipeline;
pub mod routing;
pub mod server;
pub mod stream;
pub mod types;
