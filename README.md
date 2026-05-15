<div align="center">

# Yeek

### Agent Session Manager

Browse, search, and review your AI coding sessions.
Keep every conversation at your fingertips.

Built with a local LLM proxy and plugin marketplace on the side.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)](https://v2.tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev)

[Download for macOS](https://github.com/walnut1024/yeek/releases/latest)

</div>

---

## What is Yeek?

Yeek is an **agent management tool** — a local-first desktop app that turns the raw session logs your coding agents write to disk into something you can browse, search, and revisit.

Every conversation, every subagent call, every tool execution and file edit becomes a structured, inspectable record. You can finally see what your agents have been doing.

Beyond session management, Yeek also ships:

- A **local LLM proxy** — route any coding tool through a single endpoint to DeepSeek, Anthropic, OpenAI, Zhipu, and Ollama
- A **plugin marketplace** — discover, install, and manage plugins and skills

All local. All private. Nothing leaves your machine.

---

## Session Management

This is the core. Yeek indexes session files from multiple agents into a fast, searchable SQLite database and exposes them through a clean desktop UI.

- **Multi-agent support** — Claude Code, Codex (OpenAI), and OpenCode sessions in one place
- **Project grouping** — sessions organized by project, with full transcript and message graph views
- **Deep inspection** — drill into subagent calls, tool executions, and source file references per message
- **Full-text search** — SQLite FTS5 across titles, messages, and model names, with highlighted results
- **Real-time sync** — OS-native file watchers detect new, modified, and deleted sessions instantly

---

## LLM Proxy

A lightweight Rust sidecar that runs alongside Yeek. It solves a concrete problem: different LLM providers speak different API dialects, but coding tools expect one consistent interface.

- **Unified Responses API endpoint** — `POST /v1/responses`, backed by any provider
- **Auto format translation** — Responses ↔ Chat Completions ↔ Anthropic Messages
- **Full SSE streaming** — spec-compliant event lifecycle regardless of backend provider
- **Live monitoring** — request rate, latency, per-provider errors, error event feed
- **Bridge system** — remap model names (e.g. `claude-sonnet` → `deepseek-v4-pro`) so existing configs work unchanged
- **Provider-specific fixes** — DeepSeek thinking mode, Anthropic message quirks, orphaned tool call repair

---

## Plugin Marketplace

Extend Yeek through its built-in plugin and skills platform.

- Browse and install plugins from multiple registries
- Enable, disable, and manage installed plugins
- Skills overview with health status and orphan cleanup
- Bilingual UI — English and 中文

---

## Installation

### Homebrew (recommended)

```bash
brew install --cask walnut1024/yeek/yeek
```

Upgrade:

```bash
brew upgrade --cask yeek
```

### Direct Download

Grab the latest `.dmg` from [Releases](https://github.com/walnut1024/yeek/releases/latest):

- `Yeek_*_aarch64.dmg` — drag to Applications

> **First launch**: Yeek is ad-hoc signed. Right-click → Open to bypass Gatekeeper.

---

## Quick Start

Launch Yeek. It automatically discovers sessions from `~/.claude/`, `~/.codex/`, and `~/.opencode/`.

### Configure the Proxy

Go to **Settings → Proxy** to enable providers and set API keys. Or place a `proxy.toml` next to the app:

```toml
[server]
listen_addr = "127.0.0.1:8787"

[providers.deepseek]
base_url = "https://api.deepseek.com"
api_format = "chat_completions"
api_key_env = "DEEPSEEK_API_KEY"
models = ["deepseek-v4-pro", "deepseek-v4-flash"]

[providers.anthropic]
base_url = "https://api.anthropic.com/v1"
api_format = "anthropic_messages"
api_key_env = "ANTHROPIC_API_KEY"
models = ["claude-sonnet-4-20250514"]

# Bridge: remap model names for your client
[bridges.claude_desktop_deepseek.agent]
base_url = "/deepseek_anthropic"
api_format = "anthropic_messages"

[bridges.claude_desktop_deepseek.provider]
name = "deepseek_anthropic"

[bridges.claude_desktop_deepseek.models]
"claude-sonnet" = "deepseek-v4-pro"
"claude-haiku" = "deepseek-v4-flash"
```

Point your AI coding tool at `http://127.0.0.1:8787/v1/responses`.

---

## Building from Source

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek && npm install

# Dev mode (HMR for both Rust and React)
cargo tauri dev

# Production build
npm run build && cargo build --release
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri v2 |
| Backend | Rust · Axum (HTTP + SSE) · rusqlite (SQLite + FTS5) · Tokio |
| Proxy | Standalone Rust sidecar — Responses API ↔ Chat ↔ Anthropic |
| Frontend | React 19 · TypeScript · Vite · Tailwind CSS v4 · shadcn/ui |
| State | TanStack Query · localStorage |
| i18n | react-i18next (English + 中文) |

---

## License

[MIT](LICENSE)
