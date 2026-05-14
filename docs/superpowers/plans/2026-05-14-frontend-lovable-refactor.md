# Frontend Lovable Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the Yeek frontend to consistently follow `DESIGN.md`'s Lovable-inspired warm cream visual system without changing product behavior.

**Architecture:** Start with design contract and shared UI primitives, then migrate page surfaces from the shell outward. Keep API calls, TanStack Query keys, routing state, i18n keys, keyboard behavior, and `data-ai-*` selectors stable while replacing scattered page-level styling with shared tokens and utilities.

**Tech Stack:** React 19, TypeScript, Vite, Tailwind CSS v4, shadcn/Base UI primitives, lucide-react, TanStack Query, Tauri.

---

## File Structure

- Modify: `CLAUDE.md`  
  Align frontend guidance with `DESIGN.md`; remove stale Vercel/Geist/achromatic requirements.
- Modify: `src/index.css`  
  Define Lovable design tokens, global font fallback, surface/list/input/meta utilities, and remove obsolete visual assumptions.
- Modify: `src/components/ui/button.tsx`  
  Align variants, focus, active, inset shadow, radius, and sizes with `DESIGN.md`.
- Modify: `src/components/ui/page-header.tsx`  
  Make page headers compact desktop-app headers, not marketing-style hero sections.
- Modify: `src/components/ui/page-toolbar.tsx`  
  Standardize toolbar spacing, border, wrapping, and control alignment.
- Modify: `src/app/shell/index.tsx`  
  Refine sidebar, section navigation, app overlay, and main workspace framing.
- Modify: `src/pages/dashboard/dashboard-page.tsx`  
  Normalize stat tiles, metric tiles, activity timeline, and proxy error side panel.
- Modify: `src/pages/sessions/session-row.tsx`  
  Stabilize row layout, active state, badges, icon treatment, and text overflow.
- Modify: `src/pages/sessions/session-detail-pane.tsx`  
  Reduce nested cards, normalize summary/meta/actions, and use shared toggle styling.
- Modify: `src/pages/sessions/*.tsx` and `src/pages/sessions/graph/*.tsx` as needed  
  Normalize transcript bubbles, tool accordions, summary/source blocks, graph nodes, and graph detail panel.
- Modify: `src/pages/proxy/proxy-page.tsx`  
  Normalize config groups, field rows, TOML preview, validation panel, and run/stop controls.
- Modify: `src/pages/marketplace/marketplace-page.tsx`  
  Normalize marketplace rows, plugin rows, health badges, registry controls, and dialogs.
- Modify: `src/pages/system/settings-page.tsx`  
  Normalize settings sections, operation notes, language/terminal controls, scan progress, and rebuild confirmation.
- Optional create: `src/components/ui/segmented-control.tsx`  
  Only create if repeated feed/graph, proxy mode, and marketplace agent toggles become clearer as a shared component than as utility classes.

## Task 1: Align The Design Contract

**Files:**
- Modify: `CLAUDE.md`
- Modify: `src/index.css`

- [ ] **Step 1: Update frontend guidance**

Replace the outdated Frontend Guidelines in `CLAUDE.md` with guidance that matches `DESIGN.md`:

```md
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
```

- [ ] **Step 2: Confirm the stale guidance is gone**

Run:

```bash
rg "Vercel|Geist|achromatic|shadow-as-border" CLAUDE.md
```

Expected: no matches.

- [ ] **Step 3: Normalize global design tokens**

In `src/index.css`, keep Tailwind v4 imports and ensure `:root` includes these exact semantic values:

```css
:root {
  --background: #f7f4ed;
  --foreground: #1c1c1c;
  --card: #f7f4ed;
  --card-foreground: #1c1c1c;
  --popover: #fcfbf8;
  --popover-foreground: #1c1c1c;
  --primary: #1c1c1c;
  --primary-foreground: #fcfbf8;
  --secondary: #eceae4;
  --secondary-foreground: #1c1c1c;
  --muted: #eceae4;
  --muted-foreground: #5f5f5d;
  --accent: rgba(28, 28, 28, 0.04);
  --accent-foreground: #1c1c1c;
  --destructive: #dc2626;
  --border: #eceae4;
  --input: #f7f4ed;
  --ring: rgba(59, 130, 246, 0.5);
  --radius: 6px;
  --element-hover: rgba(28, 28, 28, 0.04);
  --element-active: rgba(28, 28, 28, 0.08);
  --focus-shadow: rgba(0, 0, 0, 0.1) 0px 4px 12px;
  --button-inset: rgba(255, 255, 255, 0.2) 0px 0.5px 0px 0px inset, rgba(0, 0, 0, 0.2) 0px 0px 0px 0.5px inset, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px;
  --app-zoom: 1;
}
```

- [ ] **Step 4: Add shared utility classes**

In `src/index.css`, ensure these utilities exist and are used by later tasks:

```css
@layer components {
  .surface-panel {
    @apply rounded-xl border border-border bg-card;
  }

  .surface-section {
    @apply border-b border-border bg-card;
  }

  .metric-tile {
    @apply rounded-xl border border-border bg-card px-3 py-3;
  }

  .meta-pill {
    @apply inline-flex items-center gap-1 rounded-md border border-border bg-secondary px-1.5 py-0.5 text-[11px];
  }

  .segmented-control {
    @apply inline-flex items-center gap-0.5 rounded-md border border-border bg-secondary p-0.5;
  }

  .segmented-control-item {
    @apply h-7 rounded-sm px-2.5 text-[12px] text-muted-foreground transition-colors hover:bg-element-hover hover:text-foreground;
  }

  .segmented-control-item-active {
    @apply bg-card text-foreground;
  }
}
```

- [ ] **Step 5: Run verification**

Run:

```bash
npm run typecheck
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md src/index.css
git commit -m "style: align frontend design contract"
```

## Task 2: Refactor Shared UI Primitives

**Files:**
- Modify: `src/components/ui/button.tsx`
- Modify: `src/components/ui/page-header.tsx`
- Modify: `src/components/ui/page-toolbar.tsx`

- [ ] **Step 1: Update Button variants**

In `src/components/ui/button.tsx`, update `buttonVariants` so:

```ts
default: "border-border bg-secondary text-foreground hover:bg-element-hover active:opacity-80 aria-expanded:bg-element-active",
primary: "border-primary bg-primary text-primary-foreground shadow-[var(--button-inset)] hover:opacity-90 active:opacity-80 focus-visible:shadow-[var(--focus-shadow)] aria-expanded:opacity-90",
outline: "border-[rgba(28,28,28,0.4)] bg-transparent text-foreground hover:bg-element-hover active:opacity-80 focus-visible:shadow-[var(--focus-shadow)] aria-expanded:bg-element-active",
secondary: "border-transparent bg-transparent text-muted-foreground hover:bg-element-hover hover:text-foreground active:opacity-80 aria-expanded:bg-element-active aria-expanded:text-foreground",
ghost: "border-transparent bg-transparent text-muted-foreground hover:bg-element-hover hover:text-foreground active:opacity-80 aria-expanded:bg-element-active aria-expanded:text-foreground",
destructive: "border-destructive/30 bg-destructive/10 text-destructive hover:bg-destructive/15 active:opacity-80 focus-visible:border-destructive/30",
link: "text-primary underline-offset-4 hover:underline",
```

Keep the existing `ButtonPrimitive` and `cva` structure.

- [ ] **Step 2: Normalize icon button radius**

Keep rectangular buttons at `rounded-md`. For icon-only sizes, use full pill only where the calling site explicitly adds `rounded-full`; do not globally make all buttons pill-shaped.

- [ ] **Step 3: Update PageHeader**

In `page-header.tsx`, keep props unchanged and use:

```tsx
className="border-b border-border bg-card px-4 py-3 sm:px-5"
```

For the title:

```tsx
className={`${kicker ? "mt-1" : ""} text-[19px] font-semibold leading-[1.08] tracking-[-0.02em] text-foreground`}
```

For description:

```tsx
className="mt-1.5 max-w-2xl text-[13px] leading-[1.5] text-muted-foreground"
```

- [ ] **Step 4: Update PageToolbar**

In `page-toolbar.tsx`, keep props unchanged and use:

```tsx
className="flex min-h-11 flex-wrap items-center justify-between gap-2.5 border-b border-border bg-card px-4 py-2.5 sm:px-5"
```

- [ ] **Step 5: Run verification**

Run:

```bash
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/components/ui/button.tsx src/components/ui/page-header.tsx src/components/ui/page-toolbar.tsx
git commit -m "style: refine shared frontend primitives"
```

## Task 3: Refactor App Shell And Navigation

**Files:**
- Modify: `src/app/shell/index.tsx`
- Modify: `src/index.css`

- [ ] **Step 1: Reduce decorative overlay**

In `.app-overlay`, replace strong gradients with a barely visible paper-depth wash:

```css
background:
  radial-gradient(circle at 18% 0%, rgba(28, 28, 28, 0.025), transparent 32%),
  linear-gradient(180deg, rgba(28, 28, 28, 0.018), transparent 180px);
```

- [ ] **Step 2: Stabilize sidebar**

In `AppShell`, keep `section`, `selectedId`, query invalidation, and lazy routes unchanged. Update sidebar class to:

```tsx
className="flex w-[184px] shrink-0 flex-col border-r border-border bg-card px-2.5 py-3"
```

- [ ] **Step 3: Normalize nav buttons**

For each sidebar button:

```tsx
variant={section === key ? "default" : "ghost"}
size="sm"
className="h-8 w-full justify-start gap-2 rounded-md px-2 text-left"
```

Keep lucide icons, labels, badge, and click handlers unchanged.

- [ ] **Step 4: Stabilize badge size**

Use this badge class:

```tsx
className="inline-flex h-5 min-w-[22px] items-center justify-center rounded-full border border-border bg-card px-1.5 font-mono text-[10px] text-muted-foreground"
```

- [ ] **Step 5: Run verification**

Run:

```bash
npm run typecheck
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/shell/index.tsx src/index.css
git commit -m "style: refine app shell navigation"
```

## Task 4: Refactor Dashboard Surfaces

**Files:**
- Modify: `src/pages/dashboard/dashboard-page.tsx`

- [ ] **Step 1: Replace high-contrast stat card treatment**

Update `StatCard` so the non-danger/non-accent base uses `metric-tile`, and `accent` does not use a dark gradient. Use:

```tsx
<article data-ai-item="stat-card" className={`metric-tile flex min-h-[112px] flex-col ${
  c === "accent" ? "border-primary/20 bg-[rgba(28,28,28,0.035)]" :
  c === "danger" ? "border-destructive/20 bg-destructive/5" :
  ""
}`}>
```

Keep label, value, and sub props unchanged.

- [ ] **Step 2: Normalize metric cards**

Update `MetricCard` wrapper to:

```tsx
className={`metric-tile text-center ${onClick ? "cursor-pointer transition-colors hover:bg-element-hover" : ""}`}
```

Keep click behavior unchanged.

- [ ] **Step 3: Replace raw close glyph**

Import `X` from `lucide-react` and replace the error panel close button content with:

```tsx
<X size={14} />
```

Keep `aria` behavior no worse than current behavior by adding `aria-label={t("common.close", { defaultValue: "Close" })}` if no existing close key is present.

- [ ] **Step 4: Run verification**

Run:

```bash
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pages/dashboard/dashboard-page.tsx
git commit -m "style: normalize dashboard surfaces"
```

## Task 5: Refactor Sessions List And Detail

**Files:**
- Modify: `src/pages/sessions/session-row.tsx`
- Modify: `src/pages/sessions/session-detail-pane.tsx`
- Modify: session transcript, source, summary, tool, bubble, graph files only where their current styling conflicts with the shared token system.

- [ ] **Step 1: Stabilize session rows**

In `session-row.tsx`, preserve `role`, keyboard handlers, manage mode, context menu, selection callbacks, and labels. Use `surface-panel`-style active/hover states:

```tsx
isSelected
  ? "border-border bg-accent shadow-[0_0_0_1px_rgba(28,28,28,0.06)]"
  : "border-transparent hover:border-border hover:bg-element-hover"
```

- [ ] **Step 2: Keep row dimensions stable**

Keep icon size at `size-8`, time pill at `h-auto`, and all metadata in fixed text sizes. Ensure title remains `truncate` and project/model metadata remains wrapped via `flex-wrap`.

- [ ] **Step 3: Replace detail meta pills**

In `session-detail-pane.tsx`, update `MetaPill` wrapper to:

```tsx
<span className="meta-pill">
```

Keep label/value rendering unchanged.

- [ ] **Step 4: Replace feed/graph toggle styling**

Wrap the feed/graph buttons in:

```tsx
<div className="segmented-control">
```

Use:

```tsx
className={`segmented-control-item ${viewMode === "feed" ? "segmented-control-item-active" : ""}`}
```

and the same pattern for graph. Keep `viewMode` storage key and click handlers unchanged.

- [ ] **Step 5: Normalize transcript and graph surfaces**

For `ai-bubble.tsx`, `user-bubble.tsx`, `tool-accordion.tsx`, `summary-section.tsx`, `sources-tab.tsx`, `session-graph.tsx`, `graph/nodes.tsx`, and `graph/node-detail-panel.tsx`:

- Use `bg-card`, `bg-secondary`, `border-border`, `text-muted-foreground`, and `text-foreground` tokens.
- Avoid saturated decorative backgrounds except destructive/error states.
- Preserve all props, API calls, lazy loading, graph layout logic, and user interaction handlers.

- [ ] **Step 6: Run verification**

Run:

```bash
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/pages/sessions
git commit -m "style: normalize session surfaces"
```

## Task 6: Refactor Proxy, Marketplace, And Settings Pages

**Files:**
- Modify: `src/pages/proxy/proxy-page.tsx`
- Modify: `src/pages/marketplace/marketplace-page.tsx`
- Modify: `src/pages/system/settings-page.tsx`
- Modify: `src/index.css` if shared config/list utilities need small additions.

- [ ] **Step 1: Normalize proxy config surfaces**

In `proxy-page.tsx`, keep config cloning, validation, serialization, mutations, and mode switching unchanged. Replace page-specific card chrome with shared shallow surfaces:

```tsx
className="surface-panel"
className="metric-tile"
className="zed-input"
```

Use `segmented-control` for cards/TOML mode.

- [ ] **Step 2: Normalize marketplace rows**

In `marketplace-page.tsx`, keep all query/mutation logic unchanged. Convert registry/plugin containers to shallow bordered rows using `surface-panel`, `metric-tile`, or a local list row class. Keep health colors only in dot/text/badge accents, not large colored backgrounds.

- [ ] **Step 3: Normalize settings layout**

In `settings-page.tsx`, avoid card-in-card nesting. Use:

```tsx
<section className="surface-panel p-3">
```

for major groups and plain field rows inside. Keep rebuild confirmation, scan progress, terminal select, and language switching unchanged.

- [ ] **Step 4: Run verification**

Run:

```bash
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pages/proxy/proxy-page.tsx src/pages/marketplace/marketplace-page.tsx src/pages/system/settings-page.tsx src/index.css
git commit -m "style: normalize configuration pages"
```

## Task 7: Final Visual And Build Verification

**Files:**
- No planned edits unless verification reveals a regression.

- [ ] **Step 1: Run full checks**

Run:

```bash
npm run typecheck
npm run lint
npm run build
```

Expected: PASS.

- [ ] **Step 2: Start dev server**

Run:

```bash
npm run dev
```

Expected: Vite starts on `http://localhost:1420`.

- [ ] **Step 3: Manually verify key screens**

Check these screens at `1440x900`, `1280x720`, `1024x768`, and `390x844`:

- Dashboard overview, proxy metrics, activity timeline, proxy error side panel.
- Sessions list, selected row, manage mode, detail summary, feed view, graph view.
- Proxy cards mode and TOML mode.
- Marketplace registry/plugin list, health badges, add/remove dialogs.
- Settings preferences, maintenance, scan progress, rebuild confirmation.

- [ ] **Step 4: Acceptance checks**

Confirm:

- No pure white main page background.
- No heavy box shadows on cards.
- Long session titles, project paths, plugin names, and model names do not overflow their containers.
- Buttons show hover, active, keyboard focus, and disabled states.
- Chinese and English UI text fit in buttons, badges, and panels.
- Existing app behavior is unchanged.

- [ ] **Step 5: Commit verification fixes if needed**

If visual or lint fixes were required:

```bash
git add <changed-files>
git commit -m "fix: polish frontend refactor"
```

## Self-Review Checklist

- [ ] Every design-system requirement in `DESIGN.md` is mapped to tokens, primitives, shell, or page surfaces.
- [ ] No task changes backend APIs, DTOs, query keys, i18n semantics, or business logic.
- [ ] Existing dirty user changes are reviewed before editing; unrelated changes are not reverted.
- [ ] Every task has a verification command and an expected result.
- [ ] Implementation can stop after any committed task with the app still typechecking.
