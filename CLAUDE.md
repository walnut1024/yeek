# Yeek — Agent Session Memory Manager

Local-first Tauri v2 desktop app for managing Claude Code agent sessions.

## Tech Stack

- **Backend**: Rust (Tauri v2, rusqlite bundled, SQLite + FTS5)
- **Frontend**: React + TypeScript + Vite + Tailwind CSS v4 + shadcn/ui (Base UI)
- **Data**: TanStack Query for async state, localStorage for UI preferences

## Commands

- `cargo tauri dev` — start dev server (launch once, HMR handles rest)
- `npm run build` — frontend typecheck + build
- `cargo build` — Rust build
- `cargo check` — fast Rust typecheck

## Architecture

```
src-tauri/src/
  adapter/claudecode/   — Claude Code JSONL parser + source discovery
  adapter/codex/        — Codex adapter (placeholder)
  app/commands.rs       — Tauri command handlers (13 commands)
  app/errors.rs         — AppError enum
  app/state.rs          — AppState with Mutex<Connection>
  domain/               — SessionRecord, SourceRef, DeletePolicy types
  service/delete_planner.rs — Delete plan resolution + execution
  store/                — SQLite store (sessions, messages, sources, actions)
  sync/planner.rs       — Startup sync pipeline

src/
  app/shell/index.tsx   — Main UI (AppShell, SessionsPage, MemoryPage, SystemPage)
  lib/api.ts            — Typed Tauri command wrappers
  lib/hooks.ts          — useDebouncedValue, useLocalStorage
  components/ui/        — shadcn/ui components
```

## UI Guidelines

- **Always reference DESIGN.md for all UI/frontend work** — it defines the Vercel-inspired design system including colors, typography, shadows, spacing, and component patterns.
- Use shadow-as-border (`box-shadow: 0px 0px 0px 1px rgba(0,0,0,0.08)`) instead of traditional CSS borders where applicable.
- Three font weights only: 400 (body), 500 (UI), 600 (headings).
- Geist Sans with negative letter-spacing at display sizes.
- Keep the palette achromatic — grays from `#171717` to `#ffffff`.

## Key Patterns

- Dev server: launch `cargo tauri dev` once, don't restart — Vite HMR handles frontend changes, Tauri watches Rust changes.
- Frontend API: all Tauri commands go through typed wrappers in `src/lib/api.ts`.
- Project grouping: sessions grouped by `project_path`, displayed as `name (/Users/…/parent/name)`.
- Delete flow: `get_delete_plan` → AlertDialog confirmation → `destructive_delete_session`.
