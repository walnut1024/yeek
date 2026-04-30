//! vendor_proxy — OpenAI Responses API → multi-provider LLM proxy.
//!
//! Translates between the Responses API format (consumed by Codex TUI) and
//! provider-native formats (Chat Completions, Anthropic Messages).

pub mod adapters;
pub mod bridge;
pub mod client;
pub mod config;
pub mod server;
pub mod stream;
pub mod types;
