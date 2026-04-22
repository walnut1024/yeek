# Yeek

A local-first desktop app for managing Claude Code agent sessions.

Built with **Tauri v2** (Rust) + **React** + **TypeScript** + **Tailwind CSS**.

## Features

- **Session Browser** — Browse and inspect Claude Code conversations grouped by project, with full transcript view, message graph, and source file references
- **Skills & Plugins** — View installed plugins, toggle enable/disable, clean orphaned entries, and reinstall broken ones
- **Marketplace** — Manage plugin marketplaces (add, update, remove), browse available plugins, and install with one click
- **System Pulse** — Health checks, sync status, activity log, and index maintenance
- **Real-time Sync** — File watcher detects new/changed session files automatically via OS-native notifications

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Tauri v2, rusqlite (SQLite + FTS5) |
| Frontend | React, TypeScript, Vite, Tailwind CSS v4, shadcn/ui |
| State | TanStack Query, localStorage |
| i18n | react-i18next (English + Chinese) |

## Architecture

```
src-tauri/src/
  adapter/claudecode/   — Claude Code JSONL parser + source discovery
  app/commands.rs       — Tauri command handlers (22 commands)
  app/state.rs          — AppState with Mutex<Connection>
  domain/               — Shared types (SessionRecord, PluginInfo, etc.)
  service/              — Delete planner, plugin scanner
  store/                — SQLite store (sessions, messages, sources, actions)
  sync/                 — Startup sync, background scanner, file watcher

src/
  app/shell/            — Main layout with sidebar navigation
  pages/                — Sessions, Skills, Marketplace, System pages
  lib/api.ts            — Typed Tauri command wrappers
  components/ui/        — shadcn/ui components
  i18n/                 — English and Chinese locale files
```

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

## Design

Yeek uses the **Hermes Dark** design system — a black-first, warm cream interface inspired by Nous Research's Hermes Agent. See [DESIGN.md](DESIGN.md) for the full specification.

## License

MIT
