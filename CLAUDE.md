# Yeek — Agent Session Memory Manager

Local-first Tauri v2 desktop app for managing Claude Code agent sessions.

## Tech Stack

- **Backend**: Rust (Tauri v2, rusqlite bundled, SQLite + FTS5)
- **Frontend**: React + TypeScript + Vite + Tailwind CSS v4 + shadcn/ui (Base UI)
- **Data**: TanStack Query for async state, localStorage for UI preferences
- **Proxy**: vendor_proxy — OpenAI Responses API → multi-provider LLM proxy (DeepSeek, Zhipu, Ollama)

## Commands

- `cargo tauri dev` — start dev server (launch once, HMR handles rest)
- `npm run build` — frontend typecheck + build
- `cargo build` — Rust build (workspace: yeek + vendor_proxy)
- `cargo check` — fast Rust typecheck
- `cargo test -p vendor-proxy` — run vendor_proxy tests

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
  app/commands.rs       — Tauri command handlers
  adapter/              — Agent session source adapters
  domain/               — Core session/source/delete types
  service/              — Application workflows
  store/                — SQLite persistence
  sync/                 — Startup sync pipeline
vendor_proxy/src/       — LLM proxy workspace member
src/
  app/shell/index.tsx   — Main UI shell and section routing
  lib/api.ts            — Typed Tauri command wrappers
  components/ui/        — shadcn/ui primitives
```

## Frontend Guidelines

- Always reference DESIGN.md for UI/frontend work. It defines the Lovable-inspired warm cream design system.
- Use `#f7f4ed` as the app foundation and `#1c1c1c` as the base ink. Derive neutral states from charcoal opacity instead of arbitrary gray hex values.
- Use the available humanist sans font fallback (`DM Sans` in this repo) unless a licensed Camera Plain Variable asset is added.
- Use shallow depth: warm borders and button inset shadows, not heavy card shadows.
- Keep product surfaces dense, calm, and operational. Do not introduce landing-page heroes for app screens.
- Prioritize shadcn/Base UI primitives from `@/components/ui/`.
- Never use raw `<button>`; use `<Button>`.
- Use semantic HTML and preserve `data-ai-page`, `data-ai-region`, and `data-ai-item` selectors.
- Structural refactors must not change visual copy, state management, business logic, API calls, or keyboard behavior unless explicitly requested.

## Frontend Demos

- Frontend demo/prototype pages should be generated in the `ui_design/` directory (not in `demo/`).

## Key Patterns

- Dev server: launch `cargo tauri dev` once, don't restart — Vite HMR handles frontend changes, Tauri watches Rust changes.
- Frontend API: all Tauri commands go through typed wrappers in `src/lib/api.ts`.
- Project grouping: sessions grouped by `project_path`, displayed as `name (/Users/…/parent/name)`.
- Delete flow: `get_delete_plan` → AlertDialog confirmation → `destructive_delete_session`.
