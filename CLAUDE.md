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

- **Always reference DESIGN.md for all UI/frontend work** — it defines the Vercel-inspired design system including colors, typography, shadows, spacing, and component patterns.
- Use shadow-as-border (`box-shadow: 0px 0px 0px 1px rgba(0,0,0,0.08)`) instead of traditional CSS borders where applicable.
- Three font weights only: 400 (body), 500 (UI), 600 (headings).
- Geist Sans with negative letter-spacing at display sizes.
- Keep the palette achromatic — grays from `#171717` to `#ffffff`.
- **Prioritize shadcn/ui components** — use `Button`, `Separator`, `Skeleton`, `Badge`, `Tooltip`, `AlertDialog`, `ScrollArea`, `Tabs` etc. from `@/components/ui/` before writing raw HTML elements.
- Never use raw `<button>` — always use `<Button variant="..." size="...">`.
- Use `<Separator />` instead of `<div className="border-t ...">` for visual dividers.
- Use `<Skeleton />` instead of custom loading placeholders.
- Use semantic HTML for stable structure: page titles use `<header>`, content groups use `<section>`, secondary panels use `<aside>`, and independent cards/items use `<article>`.
- Add `data-ai-page` only to the AppShell content `<main>`; use `data-ai-region` for stable page-level areas and `data-ai-item` for repeated/interactive instances.
- Keep AI selectors sparse and product-oriented. Prefer `data-ai-*` over class names, and do not mark every layout wrapper as a region.
- Preserve interaction semantics before changing tags. Do not weaken keyboard behavior, focus handling, or nested button validity for semantic cleanup.
- Use `<nav>` only for navigation. Mixed action toolbars should stay as normal containers or use `role="toolbar"`.
- Structural refactors must not change visual styling, copy, state management, business logic, or API calls unless explicitly requested.

## Frontend Demos

- Frontend demo/prototype pages should be generated in the `ui_design/` directory (not in `demo/`).

## Key Patterns

- Dev server: launch `cargo tauri dev` once, don't restart — Vite HMR handles frontend changes, Tauri watches Rust changes.
- Frontend API: all Tauri commands go through typed wrappers in `src/lib/api.ts`.
- Project grouping: sessions grouped by `project_path`, displayed as `name (/Users/…/parent/name)`.
- Delete flow: `get_delete_plan` → AlertDialog confirmation → `destructive_delete_session`.
