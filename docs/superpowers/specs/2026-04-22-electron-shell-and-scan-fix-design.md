# Electron Shell + Scan Corruption Fix

**Goal:** Add an Electron shell alongside the existing Tauri app without destabilizing the current dev/build flow, and diagnose then fix the current 322 scan errors in the Claude Code JSONL indexer.

**Principle:** Keep Electron shell work and scan-fix work independent. Do not couple a new desktop shell rollout to a parser/indexer repair. Ship the Electron shell with the smallest viable architecture first, then iterate.

**Current Reality:**
- The frontend already supports two transports: Tauri invoke and plain HTTP fetch.
- `yeek-server` already exists as a separate Rust binary with HTTP routes and SSE.
- The current Tauri app is wired directly to the root Vite app through `src-tauri/tauri.conf.json`.
- The HTTP server currently serves API and SSE only. It does not serve frontend static assets.

---

## 1. Scope and Sequencing

This work should be split into two tracks that can be implemented and validated separately.

### Track A: Electron Shell

Deliver a minimal Electron shell that:
- starts `yeek-server`
- opens the existing frontend
- reuses the current HTTP transport and SSE event flow
- does not require a full frontend workspace migration

### Track B: Scan Diagnosis and Repair

Deliver a diagnosis-first scan fix that:
- classifies where errors occur in the scan pipeline
- captures enough structured evidence to identify the true root cause
- applies a targeted fix based on observed failures, not speculative causes

### Explicit Non-Goals for This Spec

- No immediate npm workspace migration
- No `packages/ui` extraction in the first implementation
- No frontend rewrite
- No scan pipeline redesign unless diagnosis proves the current architecture is fundamentally broken

---

## 2. Electron Shell Plan

### 2.1 Chosen Architecture

Electron should be added as a parallel shell with the smallest possible change set:

```text
yeek/
  package.json
  src/                     ← existing frontend stays in place
  dist/                    ← existing Vite build output
  electron-app/
    package.json
    electron-builder.yml
    src/
      main.ts
      preload.ts           ← optional, minimal
  src-tauri/
    Cargo.toml
    src/
      main.rs              ← existing Tauri binary
      bin/server.rs        ← existing HTTP server binary
```

### 2.2 Why This Architecture

- It preserves the current `npm run dev` and `cargo tauri dev` flow.
- It avoids breaking `src-tauri/tauri.conf.json` path assumptions on day one.
- It reuses the existing frontend code exactly as-is.
- It lets Electron prove value before any package/workspace extraction.

### 2.3 Frontend Loading Model

Electron should use the existing Vite frontend directly.

#### Development

- Start the root Vite dev server on `http://localhost:1420`
- Start `yeek-server` on `http://127.0.0.1:17321`
- Electron loads `http://localhost:1420`
- Frontend talks to `yeek-server` via existing HTTP transport and SSE

#### Production

- Build the existing frontend to `dist/`
- Package `dist/` into the Electron app
- Electron loads the built frontend from local app resources
- Electron starts packaged `yeek-server`
- Frontend talks to `http://127.0.0.1:17321`

### 2.4 Important Constraint

Do not assume `yeek-server` can serve the frontend. Today it only exposes `/api/*` and `/api/events`. If a future version wants a single-origin HTTP app, that requires explicit static-file serving and should be treated as a separate follow-up design.

---

## 3. Electron Runtime Behavior

### 3.1 `main.ts`

`electron-app/src/main.ts` should own the full server lifecycle.

Responsibilities:
1. Resolve the correct `yeek-server` executable path for dev vs packaged app
2. Spawn `yeek-server` as a child process
3. Poll `GET /api/system/status` until ready
4. Create `BrowserWindow`
5. Load frontend URL
6. Kill the child process on app shutdown
7. Handle macOS activate/window recreation cleanly

### 3.2 Readiness Check

The readiness probe must call:

```text
http://127.0.0.1:17321/api/system/status
```

Not `/system/status`.

Use:
- 250ms to 500ms poll interval
- 10s timeout
- clear startup error if readiness never succeeds

### 3.3 `preload.ts`

Keep preload minimal. It is optional unless Electron-specific behavior is needed.

If used, expose only narrow metadata such as:

```ts
window.electronAPI = {
  isElectron: true,
}
```

Do not introduce a custom command bridge unless the existing HTTP transport becomes insufficient.

### 3.4 Single Source of Truth for Server Startup

Only one component should start `yeek-server` in dev:

- Recommended: Electron `main.ts` starts the server
- Not allowed: both dev scripts and `main.ts` starting the server independently

This avoids port conflicts and split ownership.

---

## 4. Electron Dev and Build Scripts

### 4.1 Root Scripts

Keep the existing root scripts unchanged for Tauri. Add Electron-specific scripts alongside them.

Recommended shape:

```bash
npm run dev              # existing Vite dev for Tauri flow
npm run build            # existing frontend build
npm run tauri:dev        # existing Tauri shell dev
npm run tauri:build      # existing Tauri shell build
npm run electron:dev     # Vite dev + Electron
npm run electron:build   # frontend build + yeek-server build + electron-builder
```

### 4.2 Dev Flow

`npm run electron:dev` should:
- start Vite dev server
- start Electron
- let Electron `main.ts` spawn `yeek-server`

It should not separately launch `yeek-server` from the shell script.

### 4.3 Production Build

`npm run electron:build` should:
1. build the root frontend into `dist/`
2. build `yeek-server` with `--release --bin yeek-server --features http-server`
3. package Electron app with:
   - frontend assets
   - compiled `yeek-server`

### 4.4 Packaging Notes

Electron packaging must explicitly handle:
- macOS arm64 and x64 artifact strategy
- packaged binary path resolution
- executable permissions for bundled `yeek-server`
- graceful shutdown to avoid orphan server processes

---

## 5. CORS and Origin Strategy

This is a required part of the Electron plan.

### 5.1 Current Problem

The current HTTP server allows these origins:
- `http://localhost:1420`
- `http://localhost:17321`
- `tauri://localhost`

That is sufficient for current Tauri/Vite flows, but not automatically sufficient for Electron production when the frontend is loaded from local resources.

### 5.2 Required Change

Before Electron production is considered done, the backend must explicitly allow the chosen Electron origin model.

Two acceptable options:

#### Option A: Allow local-resource Electron origin

If the frontend is loaded from packaged local files, update CORS handling so Electron-origin requests are accepted in production.

#### Option B: Use a custom Electron app protocol

Serve the frontend under a custom protocol and allow that protocol origin explicitly.

### 5.3 Recommendation

Prefer a deliberate Electron-specific origin strategy over broad CORS relaxation. Do not fall back to `allow_origin(Any)` just to get the shell working.

---

## 6. Future Workspace Migration

Workspace extraction may still be desirable later, but it should be a second-step refactor after Electron is already working.

If done later, it should be a dedicated spec with:
- updated `tauri.conf.json` paths
- root script changes
- Vite config relocation
- TypeScript project references
- explicit migration and rollback steps

This refactor is intentionally out of scope for the first Electron delivery.

---

## 7. Scan Diagnosis and Repair Plan

### 7.1 Current State

The scan pipeline already has good foundations:
- source discovery
- incremental skip based on fingerprint
- one transaction per batch
- per-source `SAVEPOINT` isolation
- background scan guard to prevent concurrent runs

The immediate problem is not lack of structure. The immediate problem is lack of visibility into which stage is failing for the 322 errors.

### 7.2 Diagnosis First

Do not start by changing parser tolerance, SQLite write behavior, or FTS strategy.

First add structured diagnostics.

### 7.3 Where Diagnostics Should Live

Primary instrumentation should be added around `index_sources()`, not only inside `index_single_source()`.

Reason:
- the error counter is incremented at the `index_sources()` loop level
- failures can occur in multiple stages
- some failures are broader than a single parse function

### 7.4 Diagnostic Data Model

Introduce a structured diagnostic record similar to:

```rust
struct ScanErrorDetail {
    source_path: Option<String>,
    stage: ScanStage,
    error_kind: String,
    message: String,
    line_number: Option<u64>,
}
```

With stage values such as:
- `discover`
- `parse`
- `session_upsert`
- `message_upsert`
- `source_upsert`
- `source_link`
- `fts_rebuild`
- `cleanup`
- `commit`

`line_number` is optional and should only be populated when parsing code can provide it reliably.

### 7.5 Diagnostic Entry Point

For the first implementation, diagnostics should be available through a CLI-oriented path, not a new UI/API flow.

Recommended options:
- `yeek-server --diagnose-scan`
- `yeek-server --diagnose-scan --json`
- structured log output written during a dedicated scan run

Why:
- easier to run offline
- easier to capture complete output
- avoids introducing new UI/API surface before the root cause is known
- avoids mixing operational tooling with product API prematurely

An HTTP endpoint can be added later if diagnosis becomes an ongoing product feature.

### 7.6 Expected Diagnostic Output

The diagnostic run should produce:
- total sources discovered
- total sources attempted
- total successes
- total failures
- failure counts grouped by stage and error kind
- 5 to 10 representative samples with file path and message

It should also preserve enough detail to inspect repeated patterns.

### 7.7 Manual Validation

In parallel with the diagnostic run:
- sample 2 to 3 failing JSONL files
- inspect whether failure is caused by malformed JSON, partial writes, unexpected schema, oversized payloads, or bad path assumptions

This manual sample is part of the diagnosis, not an optional extra.

---

## 8. Root Cause Triage Guidance

These are plausible causes, but they should be treated as hypotheses only.

| Hypothesis | Likelihood | Notes |
|---|---|---|
| Malformed or partially written JSONL | High | Common in append-only transcript logs and interrupted writes |
| Unexpected message/schema shape | High | Parser assumptions may be stricter than real Claude transcript variants |
| FTS rebuild failure after successful parse/upsert | Medium | Possible if specific content causes downstream issues |
| Path-derived metadata bug | Medium | Subagent/main-session path assumptions may fail on edge cases |
| Fingerprint weakness (`len:mtime`) | Low for current 322 errors | More likely to miss updates than generate counted index failures |
| SQLite concurrent write conflict | Low for current 322 errors | Current background scan is guarded against concurrent runs |

### Important Note on Fingerprints

The current fingerprint strategy is still worth improving later because `len:mtime` is a weak change detector. But it should not be the first suspected root cause for a large counted error set.

---

## 9. Fix Strategy After Diagnosis

Once diagnostics identify the dominant failure mode, apply the smallest targeted fix.

Examples:

- If failures are malformed trailing lines:
  tolerate and skip invalid trailing JSONL entries with structured warnings

- If failures are schema variants:
  loosen parser assumptions and add fixture coverage for observed variants

- If failures happen during message insertion:
  validate/sanitize problematic fields before DB write

- If failures happen during FTS rebuild:
  isolate the offending message/session content and decide whether to sanitize, skip, or change rebuild behavior

- If path inference is wrong:
  correct project/session/subagent extraction logic and backfill affected records

Avoid broad changes before diagnosis, especially:
- changing scan concurrency
- replacing the whole fingerprint system immediately
- redesigning FTS write strategy without evidence

---

## 10. Success Criteria

### Electron

- [ ] Existing Tauri flow still works unchanged
- [ ] `npm run electron:dev` launches Electron successfully
- [ ] Electron `main.ts` exclusively owns `yeek-server` lifecycle in Electron mode
- [ ] Dev mode loads `http://localhost:1420`
- [ ] Production mode loads packaged frontend assets successfully
- [ ] Electron frontend can call API and receive SSE in both dev and production

### Scan Diagnosis and Fix

- [ ] A dedicated diagnostic scan run produces structured failure classification
- [ ] The dominant root cause of the current 322 errors is identified with concrete samples
- [ ] The fix is targeted to the observed failure stage
- [ ] Error count is reduced from 322 to an explicitly measured post-fix number
- [ ] Remaining errors, if any, are classified and understood rather than opaque

---

## 11. Recommended Implementation Order

1. Add Electron shell with no frontend relocation
2. Fix CORS/origin handling for Electron production
3. Verify API + SSE behavior in dev and packaged app
4. Add diagnostic scan mode
5. Capture and review real failure samples
6. Implement targeted scan fix
7. Re-measure error count
8. Decide later whether workspace extraction is still worth doing

This order minimizes regression risk and keeps each step independently verifiable.

---

## 12. Implementation Checklist

This checklist is intended to be used as the execution plan for the first implementation.

### Phase 0: Baseline and Guardrails

- [ ] Confirm current `npm run dev` still launches the root Vite app successfully
- [ ] Confirm current `npm run tauri:dev` still launches the Tauri app successfully
- [ ] Confirm current `cargo build --manifest-path src-tauri/Cargo.toml --bin yeek-server --features http-server` succeeds
- [ ] Record current scan baseline:
  - [ ] total reported errors = 322
  - [ ] sample current logs or screenshots for later comparison
- [ ] Do not move `src/`, `dist/`, or `src-tauri/` during this phase

**Exit criteria:**
- Existing Tauri and frontend workflows are reproducible before any Electron work begins.

### Phase 1: Electron Project Scaffolding

- [ ] Create `electron-app/`
- [ ] Add `electron-app/package.json`
- [ ] Add `electron-app/src/main.ts`
- [ ] Add `electron-app/src/preload.ts` only if needed
- [ ] Add `electron-app/electron-builder.yml`
- [ ] Add any required TypeScript config for Electron-side code
- [ ] Add root scripts:
  - [ ] `electron:dev`
  - [ ] `electron:build`

**Implementation notes:**
- Keep all existing root scripts intact.
- Do not introduce workspace restructuring.

**Exit criteria:**
- Repository contains a minimal Electron app skeleton without changing Tauri behavior.

### Phase 2: Electron Dev Runtime

- [ ] Implement `main.ts` server path resolution for local dev
- [ ] Implement child-process spawn for `yeek-server`
- [ ] Implement readiness polling against `http://127.0.0.1:17321/api/system/status`
- [ ] Implement `BrowserWindow` creation
- [ ] Load `http://localhost:1420` in dev
- [ ] Pipe server stdout/stderr into Electron logging for diagnosis
- [ ] Implement shutdown cleanup for:
  - [ ] app quit
  - [ ] window close
  - [ ] crashed or failed startup cases
- [ ] Ensure only Electron `main.ts` starts `yeek-server`

**Verification:**
- [ ] Run `npm run electron:dev`
- [ ] Verify Electron window opens
- [ ] Verify frontend renders correctly
- [ ] Verify API-backed pages load
- [ ] Verify SSE-backed status/progress updates still work

**Exit criteria:**
- Electron dev mode works end-to-end without breaking existing Tauri dev mode.

### Phase 3: Electron Production Packaging

- [ ] Build root frontend into `dist/`
- [ ] Build `yeek-server` release binary with HTTP feature enabled
- [ ] Configure `electron-builder` to package:
  - [ ] Electron app code
  - [ ] frontend build output
  - [ ] compiled `yeek-server` binary
- [ ] Implement packaged binary path resolution in `main.ts`
- [ ] Ensure packaged `yeek-server` is executable
- [ ] Load packaged frontend assets in production
- [ ] Verify graceful server shutdown in packaged app

**Verification:**
- [ ] Run `npm run electron:build`
- [ ] Launch packaged app locally
- [ ] Verify frontend loads in packaged app
- [ ] Verify API requests succeed
- [ ] Verify SSE connection succeeds

**Exit criteria:**
- A locally packaged Electron app can launch and function against bundled `yeek-server`.

### Phase 4: CORS and Origin Handling

- [ ] Decide Electron production origin model:
  - [ ] local resource origin
  - [ ] custom Electron protocol
- [ ] Update backend CORS policy to explicitly support the chosen model
- [ ] Verify both fetch and `EventSource` behavior under that origin
- [ ] Confirm no broad `allow_origin(Any)` fallback is introduced

**Verification:**
- [ ] Production Electron app can call `/api/*`
- [ ] Production Electron app can receive `/api/events`
- [ ] Existing Tauri and Vite-origin flows still work

**Exit criteria:**
- Electron production networking works with explicit origin handling and no CORS regression.

### Phase 5: Diagnostic Scan Mode

- [ ] Define `ScanErrorDetail`
- [ ] Define stage classification enum or equivalent
- [ ] Add instrumentation around `index_sources()`
- [ ] Add finer-grained error mapping in parse/upsert/FTS paths where useful
- [ ] Add a dedicated diagnostic entry point:
  - [ ] `--diagnose-scan`
  - [ ] optional `--json`
- [ ] Ensure diagnostic runs output grouped counts and representative samples
- [ ] Ensure normal scan flow remains unchanged when diagnosis mode is not used

**Verification:**
- [ ] Run diagnostic mode successfully
- [ ] Confirm output includes:
  - [ ] total discovered
  - [ ] total attempted
  - [ ] total failed
  - [ ] grouped failure counts
  - [ ] representative failing samples

**Exit criteria:**
- The 322 errors are no longer opaque; the dominant failure stage is identifiable.

### Phase 6: Root Cause Analysis

- [ ] Review grouped diagnostic output
- [ ] Manually inspect 2 to 3 failing JSONL files
- [ ] Identify the dominant failure category
- [ ] Write down the concrete root cause before modifying scan behavior

**Examples of acceptable findings:**
- malformed trailing JSON line
- unexpected Claude transcript schema variant
- DB write failure caused by specific field content
- FTS rebuild failure caused by specific message content
- path parsing bug for subagent transcripts

**Exit criteria:**
- A specific root cause is documented with real sample evidence.

### Phase 7: Targeted Scan Fix

- [ ] Implement the smallest fix that addresses the observed root cause
- [ ] Add or update tests around the failing scenario
- [ ] Re-run diagnostic scan after the fix
- [ ] Compare post-fix error count against baseline

**Verification:**
- [ ] Error count drops materially from 322
- [ ] Remaining errors are classified
- [ ] No obvious regression in normal scan behavior

**Exit criteria:**
- The dominant failure mode is fixed and verified with measurable improvement.

### Phase 8: Regression Validation

- [ ] Re-run frontend build
- [ ] Re-run Tauri dev/build smoke checks
- [ ] Re-run Electron dev smoke checks
- [ ] Re-run packaged Electron smoke checks
- [ ] Re-run scan diagnostics

**Smoke checklist:**
- [ ] sessions list loads
- [ ] session detail loads
- [ ] transcript loads
- [ ] rescan action still works
- [ ] action log still works
- [ ] SSE sync progress still appears

**Exit criteria:**
- Electron shell and scan fix both land without regressing the existing app.

### Nice-to-Have Follow-Ups

- [ ] Improve fingerprint strategy beyond `len:mtime`
- [ ] Add developer docs for Electron packaging and troubleshooting
- [ ] Revisit workspace extraction only after Electron proves stable
