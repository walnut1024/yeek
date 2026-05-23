# Yeek — Agent Session Memory Manager

Local-first Tauri v2 desktop app for browsing and managing agent coding sessions, with a built-in multi-provider LLM proxy.

## Tech Stack

- **Backend**: Rust (Tauri v2, Axum HTTP + SSE, rusqlite bundled, SQLite + FTS5)
- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS v4 + shadcn/ui (Base UI)
- **Data**: TanStack Query for async state, localStorage for UI preferences
- **Proxy**: llm-proxy (submodule) — OpenAI Responses API → multi-provider LLM proxy (DeepSeek, Anthropic, OpenAI, Zhipu, Ollama)

## Commands

- `cargo tauri dev` — start dev server (launch once, HMR handles rest)
- `npm run build` — frontend typecheck + build
- `cargo build` — Rust build (workspace: yeek + llm-proxy)
- `cargo check` — fast Rust typecheck
- `cargo test -p llm-proxy` — run llm-proxy tests

## Release

One-command release with signing + GitHub Release upload. See `README.md` for the full release and Homebrew flow.

```bash
scripts/release.sh <version> [release-notes]
# Example:
scripts/release.sh 2.0.0-alpha.6 "## What's New\n- Feature X"
```

Prerequisites:
- `.release.env` — signing key config (copy from `.release.env.example`, gitignored)
- `gh` CLI authenticated
- Signing keypair at `~/.tauri/yeek.key` (generate with `cargo tauri signer generate -w ~/.tauri/yeek.key`)

## Architecture

```
src-tauri/src/
  adapter/
    claudecode/          — Claude Code JSONL parser + source discovery
    codex/               — Codex (OpenAI) session adapter
    opencode/            — OpenCode session adapter
  app/
    commands.rs          — Tauri IPC command handlers
    proxy/               — llm-proxy lifecycle: spawn, kill, watchdog
  bin/                   — Binary entry points (yeek-server)
  domain/                — Core session/source/delete types
  http/                  — Axum HTTP API (REST + SSE)
  service/               — Application workflows
  store/                 — SQLite persistence
  sync/                  — File watchers, background scanner, startup sync
  tauri_bridge/          — Tauri IPC → service layer adapters
llm-proxy/src/           — Standalone LLM proxy binary (git submodule → github.com/walnut1024/llm-proxy)
  adapters/              — Provider adapters (Chat Completions, Anthropic)
  bridge/                — Responses ↔ Chat bidirectional conversion
  stream/                — SSE pipeline: Anthropic SSE → Chat SSE → Responses SSE
src/
  app/shell/index.tsx    — Main UI shell and section routing
  pages/                 — Sessions, Dashboard, Marketplace, Proxy, Memory, System
  lib/api.ts             — Typed Tauri command wrappers
  components/ui/         — shadcn/ui primitives
```

## Frontend Guidelines

- Always reference DESIGN.md for UI/frontend work. It defines the Lovable-inspired warm cream design system.
- Prioritize shadcn/Base UI primitives from `@/components/ui/`.
- Never use raw `<button>`; use `<Button>`.
- Use semantic HTML and preserve `data-ai-page`, `data-ai-region`, and `data-ai-item` selectors.
- Do not introduce landing-page heroes for app screens.
- Structural refactors must not change visual copy, state management, business logic, API calls, or keyboard behavior unless explicitly requested.

## Key Patterns

- Dev server: launch `cargo tauri dev` once, don't restart — Vite HMR handles frontend changes, Tauri watches Rust changes.
- Frontend API: all Tauri commands go through typed wrappers in `src/lib/api.ts`.
- Project grouping: sessions grouped by `project_path`, displayed as `name (/Users/…/parent/name)`.
- Delete flow: `get_delete_plan` → AlertDialog confirmation → `destructive_delete_session`.
