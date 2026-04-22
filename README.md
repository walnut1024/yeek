# Yeek

A local-first desktop app for browsing, inspecting, and managing Claude Code agent sessions.

Built with **Tauri v2** (Rust) + **React** + **TypeScript** + **Tailwind CSS**.

---

## Features

<table>
<tr><td><b>Session Browser</b></td><td>Browse conversations grouped by project. Full transcript view with message graph, branch navigation, source file references, and subagent inspection.</td></tr>
<tr><td><b>Skills & Plugins</b></td><td>View installed plugins and their health status. Toggle, clean orphaned entries, or reinstall broken ones with one click.</td></tr>
<tr><td><b>Marketplace</b></td><td>Manage plugin marketplaces — add, update, remove. Expand any marketplace to browse available plugins and install directly.</td></tr>
<tr><td><b>System Pulse</b></td><td>Health checks, sync status, activity audit log, and index maintenance in one calm operations surface.</td></tr>
<tr><td><b>Real-time Sync</b></td><td>OS-native file watcher detects new and changed session files instantly. Plugin config watcher picks up installs and uninstalls from external tools.</td></tr>
<tr><td><b>Bilingual UI</b></td><td>Full English and Chinese localization with react-i18next.</td></tr>
</table>

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Tauri v2, rusqlite (SQLite + FTS5) |
| Frontend | React, TypeScript, Vite, Tailwind CSS v4, shadcn/ui (Base UI) |
| State | TanStack Query, localStorage |
| i18n | react-i18next (English + 中文) |

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v20+)
- macOS / Linux

### Install & Run

```bash
npm install
cargo tauri dev
```

The app window opens immediately. Vite HMR handles frontend changes; Tauri watches Rust changes.

### Build

```bash
npm run build       # Frontend typecheck + build
cargo build         # Rust build
cargo tauri build   # Production bundle
```

---

## Architecture

```
src-tauri/src/
  adapter/claudecode/   — JSONL parser + source discovery
  app/commands.rs       — 22 Tauri command handlers
  app/state.rs          — AppState (Mutex<Connection> + watchers)
  domain/               — Shared types (SessionRecord, PluginInfo, MarketplaceEntry, …)
  service/              — Delete planner, plugin scanner
  store/                — SQLite store (sessions, messages, sources, actions)
  sync/                 — Startup sync, background scanner, file watchers

src/
  app/shell/            — Main layout with sidebar navigation
  pages/                — Sessions, Skills, Marketplace, System
  lib/api.ts            — Typed Tauri command wrappers
  components/ui/        — shadcn/ui components
  i18n/                 — English and Chinese locale files
```

### Data Flow

```
~/.claude/projects/ ──watcher──▶ SQLite ──Tauri commands──▶ React (TanStack Query)
~/.claude/plugins/  ──watcher──▶ emit("plugin-config-changed") ──▶ auto-invalidate queries
```

---

## Design

Yeek uses the **Hermes Dark** design system — a black-first, warm cream interface inspired by Nous Research's Hermes Agent.

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

---

## License

MIT
