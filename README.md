<div align="center">

# Yeek

**Your Claude Code sessions, organized and inspectable**

A local-first Tauri v2 + Rust desktop app for browsing, searching, and managing
Claude Code agent sessions — with a built-in plugin marketplace.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)](https://v2.tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev)

[Download for macOS](https://github.com/walnut1024/yeek/releases/latest) · [English](#features) · [中文](#features)

</div>

---

## Features

- **Dashboard** — Home screen with at-a-glance overview: session counts, sync health, proxy metrics, and a timestamped action audit log.
- **Session Browser** — Browse conversations grouped by project. Full transcript with branch navigation, message graph, source file references, and subagent inspection.
- **Settings** — Configure default terminal for session resume, switch language, and trigger index rebuilds.
- **Full-text Search** — FTS5-powered search across titles, messages, and model names. Results highlighted in real-time.
- **Skills & Plugins** — View installed plugins, health status, toggle enable/disable, clean orphaned entries, or reinstall broken ones.
- **Marketplace** — Manage plugin marketplaces (add, update, remove). Expand any marketplace to browse and install plugins with one click.
- **Real-time Sync** — OS-native file watcher detects new/changed session files instantly. Plugin config watcher picks up installs/uninstalls from external tools.
- **Bilingual UI** — Full English and Chinese localization.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Axum, rusqlite (SQLite + FTS5) |
| Proxy | vendor_proxy — Responses API → multi-provider LLM proxy |
| Shell | Tauri v2 |
| Frontend | React 19, TypeScript, Vite, Tailwind CSS v4, shadcn/ui |
| State | TanStack Query, localStorage |
| i18n | react-i18next (English + 中文) |

---

## Install

### macOS (Apple Silicon)

Download from [Latest Release](https://github.com/walnut1024/yeek/releases/latest):

- **DMG** — `Yeek-*-arm64.dmg` — drag to Applications
- **ZIP** — `Yeek-*-arm64-mac.zip` — portable, run directly

> Unsigned app. On first launch: right-click → Open to bypass Gatekeeper.

### From Source

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek
npm install

# Dev mode
cargo tauri dev

# Production build
npm run build
cargo build --release
```

---

## Architecture

```
src-tauri/src/
  adapter/claudecode/ — JSONL parser + source discovery
  app/commands.rs     — Business logic (shared by HTTP routes)
  http/routes.rs      — Axum HTTP API (REST + SSE)
  bin/server.rs       — yeek-server binary entry point
  store/              — SQLite store (sessions, messages, sources, actions)
  sync/               — Startup sync, background scanner, file watchers

vendor_proxy/src/
  adapters/           — Provider format adapters (Anthropic, Chat Completions)
  bridge/             — Responses API ↔ Chat Completions conversion
  stream/             — SSE streaming translation
  types/              — API type definitions

src/
  app/shell/          — Main layout with sidebar navigation
  pages/              — Sessions, Dashboard, Skills, Marketplace, Proxy, Settings
  lib/api.ts          — Typed Tauri command wrappers
  lib/transport.ts    — HTTP API client
  lib/events.ts       — SSE event stream
  components/ui/      — shadcn/ui components
  i18n/               — English and Chinese locale files
```

### Data Flow

```
~/.claude/projects/ ──file watcher──▶ SQLite ──HTTP API──▶ Tauri ──▶ React (TanStack Query)
~/.claude/plugins/  ──config watcher──▶ SSE events ──▶ auto-invalidate
```

---

## Design

Yeek uses the **Hermes Dark** design system — a black-first, warm cream interface inspired by [Nous Research's Hermes Agent](https://github.com/NousResearch/hermes-agent).

- Black canvas with warm cream (`#ffe6cb`) foreground and teal-dark (`#041C1C`) panels
- Single accent blue (`#74ade8`) for interactive focus only
- Flat surfaces with 1px borders — no shadows
- Monospace for technical detail, sans-serif for body

See [DESIGN.md](DESIGN.md) for the full specification.

---

## Contributing

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek
npm install
cargo tauri dev
```

PRs welcome. Keep changes surgical — see [CLAUDE.md](CLAUDE.md) for the coding guidelines used in this project.

---

## License

[MIT](LICENSE)
