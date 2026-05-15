<div align="center">

# Yeek

**Browse, search, and manage your Claude Code agent sessions — with a built-in multi-provider LLM proxy and plugin marketplace.**

A local-first desktop app. Rust backend + Tauri v2 + React 19.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)](https://v2.tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev)

[Download for macOS](https://github.com/walnut1024/yeek/releases/latest)

</div>

---

## What is Yeek?

Yeek turns the raw JSONL session files that Claude Code writes to disk into a
browsable, searchable library. Every conversation — including subagent calls,
tool use, and file edits — becomes a structured record you can revisit and
inspect.

Beyond session management, Yeek also ships a **local LLM proxy** that lets you
connect any AI coding tool to multiple model providers (DeepSeek, Anthropic,
OpenAI, Zhipu, Ollama) through a single, unified API — no cloud gateway
required.

---

## Key Capabilities

**Session Management**
- Browse conversations grouped by project, with full transcript and message graph
- Inspect subagent calls, tool executions, and source file references per message
- Full-text search (FTS5) across titles, messages, and model names with highlighted results
- Real-time sync — OS-native file watchers pick up new and changed sessions instantly

**LLM Proxy (VendorProxy)**
- Connect any Responses API-compatible client to DeepSeek, Anthropic, OpenAI, Zhipu, or Ollama
- Automatic format translation: OpenAI Responses API ↔ Chat Completions ↔ Anthropic Messages
- Streaming and batch modes with full SSE lifecycle event emission
- Provider-specific compatibility fixes for DeepSeek thinking mode, Anthropic message format, and more
- Built-in monitoring: request rate, latency, per-provider error tracking, error event feed

**Extensibility**
- Plugin marketplace — browse, install, enable/disable, and manage plugins from multiple registries
- Skills management — view installed skills, check health status, clean orphaned entries
- Bilingual UI — full English and Chinese localization

---

## How It Works

### Data Pipeline

Yeek watches your local `~/.claude/` directory for changes and builds a
searchable SQLite index from Claude Code's JSONL session files.

```
~/.claude/projects/  ──file watcher──▶  SQLite + FTS5  ──HTTP API──▶  React UI
~/.claude/plugins/   ──config watcher──▶  SSE events   ──▶  auto-invalidate
```

- **File watchers** detect new, modified, and deleted session files in real time
- **Background scanner** performs full rebuilds on startup and periodically
- **HTTP API** (Axum + REST + SSE) serves session data to the frontend and streams live updates
- **SQLite + FTS5** provides full-text search with instant highlighted results

### LLM Proxy

VendorProxy is a lightweight Rust HTTP server that runs as a local sidecar
process. It solves a concrete problem: different LLM providers speak different
API dialects, but AI coding tools expect a single, consistent interface.

```
Any Responses API client           VendorProxy                  LLM Providers
═══════════════════════           ═══════════                  ═════════════
POST /v1/responses ──────▶  responses_to_chat() ──────▶  DeepSeek (Chat Completions)
                            chat_to_anthropic()  ──────▶  Anthropic (Messages API)
                            chat_to_responses()  ◀──────  Zhipu (Chat / Anthropic)
◀────── Responses JSON ────                        ◀──────  OpenAI (Chat Completions)
◀────── Responses SSE  ────  SSE translators      ◀──────  Ollama (Chat Completions)
```

**Architecture decisions:**

- **Responses API as the client-facing contract.** This is the richest,
  most structured format — it carries instructions, tools, reasoning,
  truncation, and metadata in a single request envelope. Every provider,
  regardless of its native format, is reached through this one interface.

- **Chat Completions as the universal intermediate.** All format conversion
  goes through Chat Completions. A `chat_completions` provider skips the
  second hop; an `anthropic_messages` provider gets an extra Chat→Anthropic
  translation. This two-tier design keeps the adapters simple and
  independently testable.

- **Streaming with full event lifecycle.** Both Chat SSE and Anthropic SSE
  are translated to Responses SSE events in real time — `response.created`,
  `output_item.added`, content deltas, `response.completed` — so streaming
  clients receive a complete, spec-compliant event stream regardless of
  which provider is behind the proxy.

**Supported formats:**

| `proxy.toml` format | Providers | Conversion path |
|---------------------|-----------|----------------|
| `chat_completions` | DeepSeek, OpenAI, Ollama, Zhipu | Responses ↔ Chat |
| `anthropic_messages` | Anthropic, Zhipu-An, DeepSeek-An | Responses → Chat → Anthropic → Chat → Responses |

**Provider selection** resolves at request time: `x-codex-provider` header →
model-name matching → configured default. API keys are forwarded from the
incoming `Authorization` header or read from environment variables.

**Provider-specific fixes** (what general-purpose proxies miss):

| Issue | Fix |
|-------|-----|
| DeepSeek thinking mode requires `reasoning_content` on every historical assistant message | Backfills `""` when missing |
| DeepSeek rejects `[Assistant(tool_calls), User, Tool]` ordering | Reorders to `[Assistant, Tool, User]` |
| Orphaned `tool_calls` without matching `tool` results in history | Inserts dummy result placeholder |
| Anthropic and DeepSeek reject `content: ""` | Sanitizes to `" "` |

---

## Getting Started

### Download (macOS)

Install with Homebrew:

```bash
brew install --cask walnut1024/yeek/yeek
```

Or add the tap first:

```bash
brew tap walnut1024/yeek
brew install --cask yeek
```

Upgrade later with:

```bash
brew upgrade --cask yeek
```

Get the latest build from [Releases](https://github.com/walnut1024/yeek/releases/latest):

- **`Yeek_*_aarch64.dmg`** — drag to Applications

> First launch: Yeek is currently ad-hoc signed but not Apple notarized, so macOS may show a Gatekeeper warning. Right-click → Open to bypass it.

### Build from Source

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek

# Frontend dependencies
npm install

# Dev mode (hot reload for both Rust and React)
cargo tauri dev

# Production build
npm run build && cargo build --release
```

### Configure the Proxy

Yeek ships with built-in provider presets. Open Settings → Proxy to enable
providers and set API keys, or edit `proxy.toml` directly:

```toml
default_provider = "deepseek"

[server]
listen_addr = "127.0.0.1:8787"

[providers.deepseek]
format = "chat_completions"
base_url = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
models = ["deepseek-v4-pro", "deepseek-v4-flash"]

[providers.zhipu]
format = "anthropic_messages"
base_url = "https://open.bigmodel.cn/api/anthropic/v1"
api_key_env = "ZHIPU_API_KEY"
models = ["glm-5.1"]
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust · Axum (HTTP + SSE) · rusqlite (SQLite + FTS5) · Tokio |
| Proxy | vendor_proxy — Rust sidecar: Responses API ↔ Chat ↔ Anthropic translation |
| Desktop | Tauri v2 |
| Frontend | React 19 · TypeScript · Vite · Tailwind CSS v4 · shadcn/ui |
| State | TanStack Query · localStorage |
| i18n | react-i18next (English + 中文) |

---

## Project Layout

```
yeek/
├── src/                          # React frontend
│   ├── app/shell/                #   Main layout + sidebar navigation
│   ├── pages/                    #   Sessions, Dashboard, Marketplace, Proxy, Memory, System
│   ├── lib/                      #   API client, transport, SSE events, i18n
│   └── components/ui/            #   shadcn/ui components
│
├── src-tauri/src/                # Rust backend (Tauri + HTTP server)
│   ├── adapter/
│   │   ├── claudecode/           #   Claude Code JSONL parser + source discovery
│   │   ├── codex/                #   Codex (OpenAI) session adapter
│   │   └── opencode/             #   OpenCode session adapter
│   ├── app/
│   │   ├── commands.rs           #   Tauri IPC command handlers
│   │   └── proxy/                #   VendorProxy lifecycle: spawn, kill, watchdog
│   ├── bin/                      #   Binary entry points (yeek-server)
│   ├── domain/                   #   Core session/source/delete types
│   ├── http/                     #   Axum HTTP API (REST + SSE)
│   ├── service/                  #   Application workflows
│   ├── store/                    #   SQLite store (sessions, messages, sources, actions)
│   ├── sync/                     #   File watchers, background scanner, startup sync
│   └── tauri_bridge/             #   Tauri IPC → service layer adapters
│
├── vendor_proxy/                 # Standalone LLM proxy binary
│   └── src/
│       ├── adapters/             #   Provider adapters (Chat Completions, Anthropic)
│       ├── bridge/               #   Responses ↔ Chat bidirectional conversion
│       ├── stream/               #   SSE pipeline: Anthropic SSE → Chat SSE → Responses SSE
│       ├── types/                #   API type definitions
│       ├── client.rs             #   HTTP client (JSON + SSE streaming)
│       ├── config.rs             #   TOML config parser + validation
│       ├── server.rs             #   Axum server (proxy, health, admin, models)
│       └── main.rs               #   Binary entry point (PID lock, config, startup)
│
├── DESIGN.md                     # Visual design system specification
└── CLAUDE.md                     # Coding conventions and architecture notes
```

---

## Design

Yeek uses a warm cream design system inspired by [Lovable](https://lovable.dev).

- Cream canvas (`#f7f4ed`) with charcoal ink (`#1c1c1c`)
- Warm borders and button inset shadows — no heavy card shadows
- DM Sans for UI, monospace for technical detail

See [DESIGN.md](DESIGN.md) for the full token specification and component guidelines.

---

## Contributing

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek && npm install
cargo tauri dev
```

PRs welcome. Keep changes focused and surgical — see [CLAUDE.md](CLAUDE.md)
for the coding conventions used in this project.

---

## License

[MIT](LICENSE)
