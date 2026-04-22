<div align="center">

# Yeek

**Your Claude Code sessions, organized and inspectable**

A local-first Tauri v2 desktop app for browsing, searching, and managing
Claude Code agent sessions — with a built-in plugin marketplace.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-blue?logo=tauri)](https://v2.tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev)

[English](#) · [中文](#)

</div>

---

## Features

- **Session Browser** — Browse conversations grouped by project. Full transcript with branch navigation, message graph, source file references, and subagent inspection.
- **Full-text Search** — FTS5-powered search across titles, messages, and model names. Results highlighted in real-time.
- **Skills & Plugins** — View installed plugins, health status, toggle enable/disable, clean orphaned entries, or reinstall broken ones.
- **Marketplace** — Manage plugin marketplaces (add, update, remove). Expand any marketplace to browse and install plugins with one click.
- **System Pulse** — Health checks, sync status, activity audit log, and index maintenance.
- **Real-time Sync** — OS-native file watcher detects new/changed session files instantly. Plugin config watcher picks up installs/uninstalls from external tools.
- **Bilingual UI** — Full English and Chinese localization.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Tauri v2, rusqlite (SQLite + FTS5) |
| Frontend | React 19, TypeScript, Vite, Tailwind CSS v4, shadcn/ui (Base UI) |
| State | TanStack Query, localStorage |
| i18n | react-i18next (English + 中文) |

---

## Install

### From Source

```bash
git clone https://github.com/walnut1024/yeek.git
cd yeek
npm install
cargo tauri dev
```

The app window opens immediately. Vite HMR handles frontend changes; Tauri watches Rust changes.

### Production Build

```bash
npm run build       # Frontend typecheck + build
cargo tauri build   # Production bundle (.dmg / .deb / .AppImage)
```

---

## 30-Second Tour

### Sessions

| Action | How |
|--------|-----|
| Browse sessions | Grouped by project in the left panel |
| Search | Type in the search bar — matches titles, messages, models |
| Inspect a session | Click to open transcript, metadata, and source files |
| Delete sessions | Select → Manage → Delete (soft delete preserves source files) |
| Destructive delete | Session detail → Sources → Destructive Delete (removes files) |

### Skills & Plugins

| Action | How |
|--------|-----|
| View plugins | Skills tab shows all plugins with health status |
| Toggle plugin | Click the enable/disable switch |
| Fix broken plugin | Clean (remove orphan) or Reinstall (re-download from marketplace) |
| Uninstall | Confirm dialog removes plugin from disk and registry |

### Marketplace

| Action | How |
|--------|-----|
| Add marketplace | Click "Add" → enter name + `owner/repo` |
| Update all | "Update All" button pulls latest from each remote |
| Browse plugins | Click a marketplace row to expand and see available plugins |
| Install a plugin | Click "Install" on any uninstalled plugin |
| Remove marketplace | Click "Remove" — optionally remove all its plugins |

---

## Architecture

```
src-tauri/src/
  adapter/claudecode/   — JSONL parser + source discovery
  app/commands.rs       — 22 Tauri command handlers
  app/state.rs          — AppState (Mutex<Connection> + watchers)
  domain/               — Shared types
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
~/.claude/projects/ ──file watcher──▶ SQLite ──Tauri commands──▶ React (TanStack Query)
~/.claude/plugins/  ──config watcher──▶ emit("plugin-config-changed") ──▶ auto-invalidate
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
