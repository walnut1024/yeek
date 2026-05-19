<div align="center">

# Yeek

### Local mission control for AI coding agents

Your agents already did the work. Yeek makes the work observable.

Turn Claude Code, Codex, and OpenCode runs into a searchable command center: inspect every session, map agent activity, resume the right thread, clean up safely, route models through one gateway, and manage plugins from one desktop app.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)](https://v2.tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev)
[![Rust](https://img.shields.io/badge/Rust-native%20core-orange?logo=rust)](https://www.rust-lang.org)

[Download for macOS](https://github.com/walnut1024/yeek/releases/latest)

</div>

---

## Why Yeek Exists

AI coding agents move fast. They spawn subagents, call tools, edit files, branch conversations, stream partial reasoning, and leave a trail of JSONL and SQLite data across your machine.

That trail is valuable, but it is hard to use.

Yeek turns that raw exhaust into an operating surface:

- **Find the session that matters** across projects, models, tools, and agents.
- **Understand the run** with transcripts, branches, subagent calls, tool records, and a chronological Map view.
- **Resume with context** instead of hunting through terminal history.
- **Clean up safely** with source-aware delete plans and tombstones.
- **Route models consistently** through a local proxy with bridges, metrics, logs, and error events.
- **Operate plugins and skills** from a marketplace-aware UI.

Local-first. Desktop-native. Built for people who use agents all day.

---

## What You Get

```text
Agent logs scattered on disk      Yeek
-----------------------------     --------------------------------
JSONL files                       Searchable session database
Subagent sidechains               Inspectable delegated work
Tool calls hidden in transcripts  Chronological Map and metadata
Provider config drift             Local model gateway
Plugin sprawl                     Marketplace and health overview
Unsafe cleanup                    Source-aware delete plans
```

Yeek sits beside your existing tools. It does not replace Claude Code, Codex, or OpenCode; it gives you the control layer they do not ship with.

---

## The Product

Yeek is not another chat UI. It is the control plane around your AI coding tools.

| Pillar | What Yeek Does |
|--------|----------------|
| **Session Observatory** | Indexes local agent sessions into SQLite, makes them searchable, and exposes transcript, source, subagent, and Map views. |
| **Model Gateway** | Runs a local proxy that bridges agent-facing API formats to provider-facing endpoints, with model remapping and live telemetry. |
| **Plugin Ops** | Lists, installs, toggles, repairs, and removes plugins, skills, and agent extensions from marketplaces. |
| **Safe Maintenance** | Plans destructive cleanup before deleting files, blocks unsafe paths, records actions, and avoids re-importing tombstoned sources. |

The result is a single pane of glass for the messy, powerful reality of modern agent-driven development.

---

## Built For

Yeek is designed for developers who have crossed the line from "trying agents" to **operating agents**.

- You run multiple coding agents across real projects.
- You need to search old runs faster than scrolling terminal history.
- You care which model, tool call, branch, or subagent produced a result.
- You want local session data to stay local.
- You need cleanup controls that understand source files instead of deleting blindly.
- You maintain plugins, skills, and model routes as part of your development environment.

If your AI coding workflow has become infrastructure, Yeek gives it a control surface.

---

## Session Observatory

Yeek automatically discovers local sessions from:

- **Claude Code** transcripts under `~/.claude/projects`
- **Codex** sessions under `~/.codex/sessions` and `~/.codex/archived_sessions`
- **OpenCode** local session databases

Then it normalizes them into one local database and gives you a real operating surface:

- **Project-oriented browsing** with agent, model, status, branch, timing, and message counts.
- **Full-text search** over indexed session records and message previews.
- **Transcript view** for reading the conversation as it happened.
- **Map view** for scanning user turns, assistant responses, tool calls, and subagent activity by chronology.
- **Subagent drill-down** for Claude Code sidechains and delegated work.
- **Source tracking** so every session can point back to the file or database it came from.
- **Real-time sync** through native file watchers and manual rescan controls.

When your agents generate hundreds of runs, Yeek makes the history usable again.

---

## Safe Cleanup

Agent logs pile up quickly. Yeek treats deletion as an operation, not a blind `rm`.

- **Soft delete** hides sessions without touching source files.
- **Project cleanup** can hide whole project histories.
- **Delete plans** show every backing source before destructive cleanup.
- **Path validation** restricts physical deletion to known agent-owned locations.
- **Tombstones** prevent deleted sources from being re-imported on the next scan.
- **Batch delete jobs** report progress back to the UI.

This is built for local trust: you can clean the noise without losing control.

---

## Model Gateway

Yeek includes a local proxy manager for agent tools that expect one API shape while your providers expose another.

Out of the box, the proxy layer supports:

- **Local lifecycle control** from the app: start, stop, restart, status.
- **Bridge configs** that map agent-facing endpoints to provider targets.
- **Model remapping** such as `claude-sonnet` to a provider-specific model name.
- **Provider API format config** for Anthropic-style and compatible endpoints.
- **Metrics** for request count, errors, active connections, RPS, and latency.
- **Logs and error events** exposed directly in the UI.
- **Watchdog behavior** for unexpected proxy exits.

The default config ships with DeepSeek and Zhipu Anthropic-compatible bridges, and the config model is editable from the app.

Example:

```toml
[server]
listen_addr = "127.0.0.1:8787"

[bridges.claude_desktop_deepseek.agent]
base_url = "/deepseek_anthropic"
api_format = "anthropic_messages"

[bridges.claude_desktop_deepseek.provider]
name = "deepseek_anthropic"

[bridges.claude_desktop_deepseek.models]
"claude-sonnet" = "deepseek-v4-pro[1m]"
"claude-haiku" = "deepseek-v4-flash"

[providers.deepseek_anthropic]
base_url = "https://api.deepseek.com/anthropic"
api_format = "anthropic_messages"
api_key_env = "DEEPSEEK_API_KEY"
```

Point compatible tools at `http://127.0.0.1:8787` and let Yeek manage the bridge.

---

## Plugin Ops

Yeek understands agent extensions as operational inventory, not loose files.

- **Plugin overview** with enabled state, install path, version, marketplace, and health.
- **Skills and agents inventory** with descriptions, tool declarations, and health detail.
- **Marketplace management** for adding, updating, and removing registries.
- **Install from marketplace** with target metadata for agent ecosystems.
- **Repair actions** for cleaning broken installs or reinstalling plugins.
- **Config watchers** that refresh plugin state when Claude config files change.

If agents are becoming part of your development stack, their plugins deserve real management.

---

## Desktop And API

Yeek runs as a Tauri desktop app, but the same core operations are also exposed through an HTTP server binary.

The HTTP layer includes routes for:

- session browse, search, preview, detail, transcript, resume, and cleanup
- plugin and marketplace management
- proxy status, config, metrics, logs, and errors

That makes Yeek useful both as a polished desktop tool and as a local automation surface.

---

## Installation

### Homebrew

```bash
brew install --cask walnut1024/yeek/yeek
```

Upgrade:

```bash
brew upgrade --cask yeek
```

### Direct Download

Download the latest `.dmg` from [Releases](https://github.com/walnut1024/yeek/releases/latest).

For Apple Silicon, use:

```text
Yeek_*_aarch64.dmg
```

Yeek is currently ad-hoc signed. On first launch, right-click the app and choose **Open** to pass Gatekeeper.

---

## Quick Start

1. Launch Yeek.
2. Let it scan local agent data from `~/.claude`, `~/.codex`, and OpenCode data directories.
3. Open **Sessions** to browse by project or search across runs.
4. Select a session and inspect **Transcript**, **Map**, and **Sources**.
5. Open **Marketplace** to manage plugins and skills.
6. Open **Proxy** to configure and run the local model gateway.

No cloud account is required for session indexing. Your session database lives locally.

---

## Build From Source

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek
npm install

# Desktop dev mode
npm run tauri:dev

# Frontend build
npm run build

# Desktop production build
npm run tauri:build
```

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop | Tauri v2 |
| Backend | Rust, Tokio, rusqlite, Axum |
| Storage | SQLite with indexed session/message records |
| Frontend | React 19, TypeScript, Vite, Tailwind CSS v4 |
| Data Fetching | TanStack Query |
| Visualization | D3-assisted session Map UI |
| i18n | react-i18next, English and Chinese |
| Distribution | Tauri bundling and updater artifacts |

---

## License

[MIT](LICENSE)
