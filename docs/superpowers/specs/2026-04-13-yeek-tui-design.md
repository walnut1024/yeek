# Yeek - Rust + Tauri v2 Session Memory Manager

## Overview

Yeek is a local-first desktop app for managing agent memory through sessions.

This version is designed around:

- Rust core services
- Tauri v2 desktop shell
- Local SQLite index
- Web frontend for management UX

In Yeek, `Session` is the most raw and trustworthy memory unit. The app is not a generic transcript browser and not a chat client. It is a management console for local agent sessions.

Initial agent targets:

- Claude Code
- Codex

## Product Definition

Yeek is a session-first memory manager with safe local operations.

Core product behavior:

- Index local agent session sources
- Show sessions in a fast management UI
- Support inspect, search, filter, pin, archive, hide, and delete
- Make source traceability explicit before destructive actions
- Keep Rust responsible for source access, indexing, delete planning, and policy enforcement

## Why Tauri v2

This design uses Tauri v2 because it fits the product shape well:

- Rust is a strong fit for local filesystem scanning, source parsing, indexing, and deletion safety logic
- A web UI is better than a terminal UI for dense management surfaces, flexible layouts, badges, dialogs, and future source inspectors
- Tauri commands provide a clean boundary for request-response operations
- Tauri events provide a lightweight mechanism for sync status, refresh notifications, and action completion
- The app stays local-first and small without needing a heavyweight Electron-style stack

## Product Goals

- Provide one place to manage local agent sessions across tools
- Make raw session memory easy to inspect and clean up
- Support fast search and filtering
- Make session management safer than manual file deletion
- Preserve source traceability for every managed session
- Create a base for future memory extraction features

## Non-Goals For V1

- Multi-agent response compare
- Cloud sync
- Editing session content in place
- Automatic memory summarization as the primary workflow
- Destructive delete when source ownership is ambiguous
- Multi-window complexity

## Design Principles

1. Session first. Session is the primary object, not a derived memory card.
2. Rust owns correctness. Filesystem access, source parsing, indexing, delete planning, and policy checks belong in Rust.
3. Frontend owns workflow clarity. The web UI should optimize scanning, selection, and safe actions.
4. Read-only by default. Mutating actions must be intentional and well explained.
5. Preview fast, hydrate deeper only when needed.

## UX Direction

The app should feel like a local operations console, not like a chat app.

The main screen should help the user answer:

1. What sessions do I have?
2. Which ones matter?
3. Which ones are stale or noisy?
4. Which ones can I safely clean up?
5. What exact files back this session?

The UI should feel:

- Calm
- Dense but readable
- Operational
- Keyboard-friendly
- Safe around destructive actions

## Information Architecture

V1 uses three top-level sections:

1. `Sessions`
2. `Memory`
3. `System`

### Sessions

Primary workspace.

Responsibilities:

- Browse sessions by project, agent, or date grouping
- Search and filter sessions
- Open session detail
- Pin, archive, hide, or delete sessions
- Select sessions for batch actions
- Inspect source files and delete safety

### Memory

Secondary section for future evolution.

V1 options:

- Placeholder screen
- Pinned sessions view
- Curated long-term session references

This section should not distract from `Sessions`.

### System

Operational visibility.

Responsibilities:

- Show indexing status
- Show watched sources
- Show recent actions
- Show parse or sync errors
- Trigger full rescan

## High-Level Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     Frontend (Web UI)                       │
│ sessions workspace, detail pane, filters, dialogs, system   │
├──────────────────────────────────────────────────────────────┤
│                    Tauri v2 Shell Layer                     │
│ invoke commands, event bridge, window lifecycle             │
├──────────────────────────────────────────────────────────────┤
│                     Rust Application Core                   │
│ services, action policy, sync coordinator, state cache      │
├──────────────────────────────────────────────────────────────┤
│                     Rust Infrastructure                      │
│ adapters, SQLite store, source discovery, delete planner    │
├──────────────────────────────────────────────────────────────┤
│                      Local Source Data                       │
│ Claude files, Codex files, local SQLite metadata            │
└──────────────────────────────────────────────────────────────┘
```

## Frontend / Backend Split

### Rust Responsibilities

Rust should own:

- Source discovery
- Source parsing
- SQLite schema and writes
- Search queries
- Session preview query APIs
- Full session hydration
- Delete planning
- Destructive delete execution
- Action logging
- Sync watcher and poller lifecycle
- Security and policy enforcement

### Frontend Responsibilities

Frontend should own:

- Layout and navigation
- Session list rendering
- Search and filter controls
- Dialogs and confirmations
- Selection state presentation
- Inline feedback and status toasts
- View-level state such as pane expansion, sort mode, and local UI preferences

### Boundary Rule

Frontend should never perform direct source file operations.

All filesystem, indexing, and deletion behavior must go through Rust commands.

## Tauri v2 Communication Model

Use two communication patterns:

### Commands

Use Tauri commands for:

- Initial page queries
- Search
- Filtered browsing
- Session preview fetch
- Session detail hydration
- Session actions such as pin, archive, hide, soft delete
- Delete planning
- Source delete execution
- System queries
- Full rescan trigger

Commands should be request-response and return typed payloads.

### Events

Use Tauri events for:

- Sync started
- Sync progress summary
- Sync completed
- Visible data changed
- Action completed
- Source health changed

Events should not carry full transcripts or high-volume row payloads. They should notify the frontend to refresh or update status.

## Recommended Tech Stack

### Desktop Shell

- Tauri v2
- Rust edition 2021 or current stable toolchain

### Backend

- `tokio` for async runtime
- `serde` and `serde_json`
- `sqlx` or `rusqlite` for SQLite access
- `notify` if needed for file watching strategy
- `thiserror` for error modeling

### Frontend

Chosen stack:

- React + TypeScript + Vite
- shadcn/ui
- Tailwind CSS

React is the default because:

- There will be many interactive controls, derived states, dialogs, and list/detail coordination
- Tauri examples and ecosystem coverage are broad
- The app benefits from mature table, virtual list, and command palette patterns

shadcn/ui is a good fit because:

- The app needs a consistent system for dialogs, popovers, tabs, sheets, badges, inputs, and menus
- Components remain locally owned and customizable instead of hidden behind a large runtime abstraction
- It supports a serious desktop-tool feel better than assembling ad hoc primitives

Tailwind CSS is appropriate because:

- The app has many small stateful visual distinctions such as pinned, archived, hidden, partial, and delete policy states
- It makes it easier to keep dense layouts consistent across list, detail, and system views

## Domain Model

```rust
pub struct SessionRecord {
    pub id: String,
    pub agent: String,
    pub project_path: String,
    pub title: String,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub status: SessionStatus,
    pub visibility: VisibilityStatus,
    pub pinned: bool,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub delete_mode: DeleteMode,
    pub message_count: i64,
    pub updated_at: String,
}

pub enum SessionStatus {
    Active,
    Complete,
    Partial,
}

pub enum VisibilityStatus {
    Visible,
    Hidden,
    Archived,
}

pub enum DeleteMode {
    None,
    SoftDeleted,
    SourceDeleted,
}

pub struct SessionPreview {
    pub record: SessionRecord,
    pub source_refs: Vec<SourceRef>,
    pub preview_messages: Vec<MessagePreview>,
    pub delete_summary: DeleteSummary,
}

pub struct SessionDetail {
    pub record: SessionRecord,
    pub messages: Vec<Message>,
    pub sources: Vec<SourceRef>,
    pub delete_plan_preview: Option<DeletePlanPreview>,
}

pub struct SourceRef {
    pub source_id: String,
    pub source_type: String,
    pub path: String,
    pub delete_policy: DeletePolicy,
}

pub enum DeletePolicy {
    NotAllowed,
    HideOnly,
    FileSafe,
    NeedsReview,
}
```

## SQLite Schema

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    project_path TEXT,
    title TEXT,
    model TEXT,
    git_branch TEXT,
    started_at DATETIME,
    ended_at DATETIME,
    status TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'visible',
    pinned INTEGER NOT NULL DEFAULT 0,
    archived_at DATETIME,
    deleted_at DATETIME,
    delete_mode TEXT NOT NULL DEFAULT 'none',
    message_count INTEGER NOT NULL DEFAULT 0,
    updated_at DATETIME NOT NULL
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    parent_id TEXT,
    role TEXT NOT NULL,
    kind TEXT NOT NULL,
    content_preview TEXT NOT NULL,
    timestamp DATETIME,
    is_sidechain INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    source_type TEXT NOT NULL,
    path TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    last_modified DATETIME NOT NULL,
    last_seen_at DATETIME NOT NULL,
    status TEXT NOT NULL
);

CREATE TABLE session_sources (
    session_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    source_type TEXT NOT NULL,
    path TEXT NOT NULL,
    delete_policy TEXT NOT NULL,
    PRIMARY KEY (session_id, source_id),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE action_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT,
    action TEXT NOT NULL,
    detail TEXT,
    created_at DATETIME NOT NULL
);

CREATE VIRTUAL TABLE messages_fts USING fts5(
    session_id UNINDEXED,
    message_id UNINDEXED,
    role,
    kind,
    content_preview
);
```

## Source-Driven Sync Model

Sync must be source-driven, not session-file-driven.

Reason:

- A session may depend on multiple upstream sources
- Delete safety may depend on source ownership
- Metadata can live outside the primary transcript file

### Startup Sequence

1. Start Tauri app
2. Initialize Rust app state
3. Open SQLite database
4. Discover sources from all adapters
5. Compare fingerprints against indexed sources
6. Parse and upsert affected sessions
7. Start watcher and fallback poller
8. Frontend loads first session page through command

### Incremental Sequence

1. Watcher or poller detects source change
2. Sync manager coalesces events
3. Adapter reparses affected sources
4. Store updates sessions, messages, and source mappings
5. Rust emits a lightweight `sync:updated` event
6. Frontend refreshes active list or status panels

### Delete Interaction

When source delete succeeds:

- Remove or mark source as deleted
- Mark session as `source_deleted`
- Recompute dependent sessions if needed
- Append an action log entry
- Emit an action-completed event

## Agent Source Mapping

### Claude Code

Likely sources:

- `~/.claude/history.jsonl`
- `~/.claude/sessions/{pid}.json`
- `~/.claude/projects/{project}/{sessionId}.jsonl`

Primary session source:

- Project session transcript JSONL

Delete policy expectation:

- Transcript file may be deletable in some cases
- Shared metadata sources should not be blindly removed

### Codex

Likely sources:

- `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
- `~/.codex/state_5.sqlite`
- `~/.codex/history.jsonl`

Primary session source:

- Rollout transcript JSONL

Delete policy expectation:

- Transcript files may be safely deletable
- SQLite metadata should not be deleted per session unless explicitly proven safe

## Sessions Workspace Design

This is the main product surface.

### Layout

Desktop layout should use three horizontal regions where screen width permits:

- Left rail: filters, grouping, counts
- Main list: grouped session rows
- Right detail pane: preview, actions, source inspector

On smaller windows:

- Collapse left rail into a slide-over or top filter bar
- Keep list and detail as the main split

Recommended page composition:

- `AppShell`
- `TopBar`
- `SessionsSidebar`
- `SessionListPane`
- `SessionDetailPane`
- `ActionToolbar`
- `ConfirmDeleteDialog`
- `SourceInspectorPanel`

### Layout Goals

- List remains the primary scanning surface
- Detail remains decision support, not the main focus by default
- Destructive actions are visible but clearly separated
- Source information is reachable in one click

### Session List Row Design

Each row should show:

- Title or first prompt preview
- Agent badge
- Pinned badge if applicable
- Archived or hidden state
- Partial status if parse is incomplete
- Relative updated time
- Message count
- Delete safety hint if useful

Rows should optimize for:

- Skimmability
- State recognition
- Fast cleanup decisions

### Detail Pane Design

The detail pane should have three tabs or stacked sections:

1. `Overview`
2. `Transcript`
3. `Sources`

#### Overview

Show:

- Title
- Project
- Agent
- Model
- Branch
- Time
- Status
- Quick actions
- Delete policy summary

#### Transcript

Show:

- First human message near the top
- Preview messages immediately
- Full transcript after hydrate
- Collapsed tool output by default
- Sidechain markers

#### Sources

Show:

- Source type
- Full path
- Last indexed time
- Delete policy
- Delete target count
- Safety reasoning

This tab is critical for building trust before deletion.

## Interaction Design

### Primary User Flow

1. Open app
2. See recent sessions immediately
3. Search or filter if needed
4. Select a session
5. Read overview and preview
6. Pin, archive, hide, or inspect delete policy
7. Optionally perform source delete after confirmation

### Selection Model

Selection should be frontend-local and lightweight.

Behavior:

- Single click selects a row
- Arrow keys move selection
- Space toggles multi-select
- Shift+click range selection is optional
- Bulk actions appear only when selection is non-empty

### Search UX

Search should be session-oriented, not message-oriented.

Search input behavior:

- Debounced live search
- Results update without full page jank
- Match snippets shown in detail pane
- Clear escape hatch for returning to browse state

### Filter UX

Filters should appear as a compact command bar or chip rail.

V1 filters:

- Agent
- Project
- Date range
- Visibility
- Pinned
- Deletion safety

Filter behavior:

- Multiple values within one category use OR
- Different categories use AND
- Active filter count always visible

### Bulk Actions

V1 batch-safe actions:

- Pin
- Unpin
- Archive
- Unarchive
- Hide
- Unhide

Bulk source delete is not required for V1.

## React UI Architecture

The React app should be feature-oriented rather than route-fragmented.

### Recommended Frontend Structure

- `app/`
  - app shell, providers, top-level layout
- `pages/`
  - sessions page, memory page, system page
- `features/sessions/`
  - list, row rendering, grouping, selection
- `features/detail/`
  - overview tab, transcript tab, source tab
- `features/search/`
  - search input, result state, query synchronization
- `features/filters/`
  - filter bar, chips, presets
- `features/actions/`
  - bulk actions, inline actions, delete confirmations
- `features/system/`
  - sync status, errors, action log
- `components/ui/`
  - shadcn/ui generated and customized primitives

### State Management Guidance

Keep state simple:

- React local state for view-only interactions
- A lightweight query/cache layer for command-backed data fetching
- Avoid a heavy global client store unless complexity truly requires it

Reasonable options:

- TanStack Query for command-backed async data
- React context for app-wide UI preferences

This app should not start with a complicated frontend state machine unless later complexity proves necessary.

## shadcn/ui Component Strategy

shadcn/ui should be used as a design-system base, not a final design.

Recommended components:

- `Button`
- `Input`
- `Dialog`
- `AlertDialog`
- `Tabs`
- `Badge`
- `Popover`
- `DropdownMenu`
- `Tooltip`
- `ScrollArea`
- `Separator`
- `Sheet`
- `Command`
- `Skeleton`

Suggested mapping:

- Search input: `Input` or `Command`-style surface
- Filter rail or popover: `Popover`, `Badge`, `Button`, `Separator`
- Session detail tabs: `Tabs`
- Delete confirm: `AlertDialog`
- Source inspector on narrow screens: `Sheet`
- Sync/loading states: `Skeleton`

### Custom Components Needed

Do not force everything into generic shadcn primitives.

Yeek should define custom product components such as:

- `SessionRow`
- `SessionGroup`
- `SessionStateBadges`
- `DeletePolicyBadge`
- `SessionOverviewCard`
- `TranscriptEventRow`
- `SourcePathCard`
- `ActionFeedbackBar`

These should compose shadcn primitives but own Yeek-specific semantics.

## Keyboard Design

Even though this is a desktop app, keyboard flow should remain strong.

### Global Keys

- `CmdOrCtrl+1`: Sessions
- `CmdOrCtrl+2`: Memory
- `CmdOrCtrl+3`: System
- `CmdOrCtrl+f`: Focus search
- `CmdOrCtrl+r`: Full rescan
- `?`: Toggle shortcut help
- `Esc`: Close current transient UI or clear focus from temporary surface

### Sessions Keys

- `j/k` or `ArrowDown/ArrowUp`: Move selection
- `g` / `G`: Jump to top or bottom
- `Enter`: Open or focus detail
- `Space`: Toggle selection
- `p`: Pin or unpin
- `a`: Archive or unarchive
- `m`: Hide or unhide
- `d`: Soft delete
- `D`: Open destructive delete confirm if allowed
- `o`: Open Sources tab
- `s`: Cycle sort mode
- `t`: Cycle grouping mode
- `[ / ]`: Previous or next group

### Search And Filter Keys

- `/`: Focus search input
- `Enter`: Commit current search focus and return to list
- `Esc`: Clear transient search state
- `f`: Open filter bar
- `Tab`: Move across filter controls
- `Backspace`: Clear current chip value where applicable

### Dialog Keys

- `Enter`: Confirm focused action
- `Tab`: Move between buttons
- `Esc`: Cancel
- `y`: Confirm if dialog is simple and binary
- `n`: Cancel

Dangerous confirmations must default focus to cancel.

## Desktop UX With React

Because this is a desktop-style app, the frontend should feel more like an IDE utility than a marketing site.

Guidance:

- Keep primary actions visible without making every row look button-heavy
- Prefer hover enhancement, but never hide essential actions behind hover only
- Support both mouse and keyboard equally well
- Remember the selected session while data refreshes
- Avoid route-heavy navigation for simple pane changes

### Empty States

Important empty states should be designed intentionally:

- No indexed sessions yet
- No search results
- No visible sessions because everything is filtered out
- No source delete available

Empty states should always suggest a next action:

- rescan
- clear filters
- inspect sources
- switch section

## Window And Navigation Model

V1 should stay single-window.

Reasons:

- Simpler mental model
- Easier state synchronization
- Lower complexity around sync status and destructive actions

Optional later:

- Deep-link into a session detail route
- Open source path inspector in a secondary utility window

## Frontend State Model

The frontend should keep UI state lean and derived where possible.

```ts
type AppState = {
  section: "sessions" | "memory" | "system";
  helpVisible: boolean;
  syncStatus: SyncStatusView;
};

type SessionsPageState = {
  query: string;
  filters: FilterState;
  sort: SortMode;
  group: GroupMode;
  selectedIds: string[];
  focusedSessionId: string | null;
  detailTab: "overview" | "transcript" | "sources";
  detailHydration: "idle" | "loading" | "ready" | "error";
  dialog: null | ConfirmDialogState;
};
```

### State Rules

- List query state should be serializable
- Detail hydration state should be separate from list state
- Closing a dialog must not clear list selection
- Sync updates should not blindly reset sort, filters, or selection

### Query Layer Guidance

Recommended command-query mapping:

- sessions list queries keyed by `query + filters + sort + group`
- preview query keyed by `focusedSessionId`
- detail query keyed by `focusedSessionId + detailTab`
- system queries keyed separately and refreshed less often

Frontend should treat Tauri events as invalidation signals rather than as the full data transport.

## Rust Service Design

Rust should expose clear application services.

### Query Services

- Browse sessions
- Search sessions
- Get session preview
- Get session detail
- Get source inspector data
- Get sync status
- Get action log

### Action Services

- Pin sessions
- Archive sessions
- Hide sessions
- Soft delete sessions
- Resolve delete plan
- Delete source
- Trigger rescan

### Policy Services

- Determine delete safety
- Explain why delete is blocked
- Compute visible actions for a session or selection

## Tauri Command API Design

Recommended commands:

```rust
#[tauri::command]
async fn browse_sessions(params: BrowseParams) -> Result<SessionListResult, AppError>;

#[tauri::command]
async fn search_sessions(params: SearchParams) -> Result<SessionListResult, AppError>;

#[tauri::command]
async fn get_session_preview(session_id: String) -> Result<SessionPreview, AppError>;

#[tauri::command]
async fn get_session_detail(session_id: String) -> Result<SessionDetail, AppError>;

#[tauri::command]
async fn get_source_inspector(session_id: String) -> Result<SourceInspectorPayload, AppError>;

#[tauri::command]
async fn set_pinned(ids: Vec<String>, value: bool) -> Result<(), AppError>;

#[tauri::command]
async fn set_archived(ids: Vec<String>, value: bool) -> Result<(), AppError>;

#[tauri::command]
async fn set_hidden(ids: Vec<String>, value: bool) -> Result<(), AppError>;

#[tauri::command]
async fn soft_delete_sessions(ids: Vec<String>) -> Result<(), AppError>;

#[tauri::command]
async fn resolve_delete_plan(session_id: String) -> Result<DeletePlanPayload, AppError>;

#[tauri::command]
async fn delete_session_source(session_id: String) -> Result<ActionResult, AppError>;

#[tauri::command]
async fn rescan_sources() -> Result<ActionResult, AppError>;

#[tauri::command]
async fn get_system_status() -> Result<SystemStatusPayload, AppError>;
```

## Event Design

Recommended event names:

- `sync:started`
- `sync:progress`
- `sync:updated`
- `sync:completed`
- `action:completed`
- `system:error`

Event payloads should stay small.

Example:

```json
{
  "type": "sync:updated",
  "changedSessionIds": ["abc", "def"],
  "lastRefreshAt": "2026-04-13T12:34:56Z"
}
```

## Security Model

Tauri should be configured conservatively.

Principles:

- Allow only the capabilities the app actually uses
- Do not expose broad arbitrary filesystem access to the frontend
- Keep delete execution inside Rust only
- Validate all paths before deletion
- Require adapter-approved delete plans before destructive operations

If plugins are used, permission scope should be narrow and explicit.

## Error Model

Suggested error categories:

- `parse_error`
- `source_missing`
- `permission_denied`
- `delete_not_safe`
- `delete_failed`
- `hydrate_failed`
- `sync_failed`
- `db_error`

UX rules:

- Search and browse should remain usable even when detail hydration fails
- Unsafe delete should explain the block reason
- System screen should surface actionable error details
- Toasts should stay brief; deep error detail belongs in `System`

## Visual Design Guidance

The app should not look like a generic admin dashboard.

Suggested visual direction:

- Warm neutral background with one strong accent color
- Dense list with restrained badges
- Clear separation between safe actions and dangerous actions
- Strong path typography in source inspector
- Thoughtful empty states instead of blank panels

The UI should feel tool-like and trustworthy.

### shadcn Theming Guidance

The default shadcn look should be customized.

Guidelines:

- Define Yeek-specific semantic tokens for session state and delete policy
- Avoid the default generic SaaS feel
- Use typography that feels editorial and operational, not landing-page polished
- Make danger styles precise and rare so they keep meaning
- Keep surfaces layered enough to separate list, detail, and system information clearly

## Suggested Screen Structure

### Header

Show:

- App title
- Current section
- Sync status
- Search field or search trigger
- Help trigger

### Sessions Page

Show:

- Left utility rail with filters and counts
- Central list with grouped sessions
- Right detail pane with action buttons and source visibility

### System Page

Show:

- Sync health cards
- Recent errors
- Recent actions
- Rescan trigger

## Project Structure

```text
yeek/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── app/
│       │   ├── commands.rs
│       │   ├── events.rs
│       │   ├── state.rs
│       │   └── errors.rs
│       ├── domain/
│       │   ├── session.rs
│       │   ├── source.rs
│       │   ├── actions.rs
│       │   └── system.rs
│       ├── adapter/
│       │   ├── mod.rs
│       │   ├── claudecode/
│       │   └── codex/
│       ├── store/
│       │   ├── mod.rs
│       │   ├── schema.rs
│       │   ├── sessions.rs
│       │   ├── search.rs
│       │   └── actions.rs
│       ├── service/
│       │   ├── browse.rs
│       │   ├── search.rs
│       │   ├── detail.rs
│       │   ├── actions.rs
│       │   ├── delete.rs
│       │   └── sync.rs
│       └── sync/
│           ├── mod.rs
│           ├── watcher.rs
│           ├── poller.rs
│           └── planner.rs
├── src/
│   ├── app/
│   │   ├── providers/
│   │   ├── router/
│   │   └── shell/
│   ├── pages/
│   │   ├── sessions/
│   │   ├── memory/
│   │   └── system/
│   ├── components/
│   │   ├── session-row.tsx
│   │   ├── session-badges.tsx
│   │   ├── delete-policy-badge.tsx
│   │   └── action-feedback-bar.tsx
│   ├── components/ui/
│   ├── features/
│   │   ├── search/
│   │   ├── filters/
│   │   ├── detail/
│   │   ├── actions/
│   │   └── source-inspector/
│   ├── lib/
│   │   ├── tauri.ts
│   │   ├── events.ts
│   │   └── api.ts
│   └── styles/
└── package.json
```

## Implementation Plan

### Milestone 1

- Tauri v2 app shell
- Rust SQLite store
- Claude adapter
- Sessions list page
- Session preview and overview pane

### Milestone 2

- Search and filter UX
- Session actions: pin, archive, hide, soft delete
- System page
- Sync events and refresh behavior

### Milestone 3

- Codex adapter
- Source inspector
- Delete plan resolution
- Safe destructive delete flow

### Milestone 4

- Bulk operations polish
- UI refinement
- Preferences persistence
- Performance tuning

## Step-By-Step Implementation Plan

This section is the recommended execution order for building Yeek V1.

The plan is intentionally staged so that:

- The app becomes usable early
- Rust safety-critical logic lands before destructive features
- The React UI can be exercised against real commands quickly
- Delete complexity comes after session browsing is stable

### Phase 0: Project Bootstrap

1. Create the Tauri v2 app with React + TypeScript + Vite.
2. Add Tailwind CSS and initialize shadcn/ui.
3. Create the basic desktop shell with three sections:
   - `Sessions`
   - `Memory`
   - `System`
4. Set up Rust crate structure under `src-tauri/src/`:
   - `app/`
   - `domain/`
   - `adapter/`
   - `store/`
   - `service/`
   - `sync/`
5. Add shared error types and a minimal application state container.
6. Confirm the frontend can invoke a simple Rust command and render the result.

Exit criteria:

- Tauri app launches successfully
- React frontend renders inside the desktop shell
- One sample command like `get_system_status` works end to end

### Phase 1: Database And Core Rust Domain

1. Implement the SQLite schema for:
   - `sessions`
   - `messages`
   - `sources`
   - `session_sources`
   - `action_log`
   - `messages_fts`
2. Create Rust domain types for:
   - `SessionRecord`
   - `SessionPreview`
   - `SessionDetail`
   - `SourceRef`
   - `DeletePolicy`
   - `BrowseParams`
   - `SearchParams`
3. Build a store layer with intent-oriented methods:
   - browse sessions
   - search sessions
   - upsert indexed sessions
   - set pinned/archived/hidden
   - tombstone sessions
   - record action log
4. Add migration/bootstrap logic for first launch.
5. Add unit tests for schema initialization and basic store operations.

Exit criteria:

- DB initializes cleanly
- Store methods work against a local test database
- Browse and search methods return typed Rust payloads

### Phase 2: Claude Adapter First

1. Implement Claude source discovery.
2. Model `SourceDescriptor` and source fingerprinting.
3. Parse Claude session transcript files into unified indexed records.
4. Parse enough metadata to populate:
   - project path
   - title or first prompt
   - model if available
   - timestamps
   - message previews
5. Implement `LoadSession` or equivalent full hydration path for Claude sessions.
6. Map Claude session sources into `session_sources`.
7. Add adapter tests using representative fixture data.

Exit criteria:

- Real Claude sessions can be indexed
- Browse data is populated from actual source files
- Detail hydration works for at least one real transcript

### Phase 3: Sync Pipeline And Initial Indexing

1. Implement a sync planner that compares discovered source fingerprints with indexed fingerprints.
2. Support source states:
   - new
   - changed
   - deleted
   - unchanged
3. Build the startup indexing flow:
   - discover
   - diff
   - parse
   - upsert
4. Implement a low-risk watcher and fallback polling model.
5. Emit lightweight Tauri events:
   - `sync:started`
   - `sync:updated`
   - `sync:completed`
   - `system:error`
6. Add logging around indexing and parse failures.

Exit criteria:

- App can index Claude sources on startup
- Re-running startup performs incremental rather than full reprocessing
- Frontend can receive sync events

### Phase 4: Sessions Page Skeleton In React

1. Build the top-level app shell with section navigation.
2. Implement the `Sessions` page layout:
   - top bar
   - optional filter rail
   - session list pane
   - detail pane
3. Add a typed frontend API wrapper for Tauri commands.
4. Use a lightweight query layer such as TanStack Query for:
   - browse sessions
   - get session preview
   - get session detail
   - get system status
5. Build loading, empty, and error states for the sessions page.
6. Render a selectable session list using real indexed data.

Exit criteria:

- Sessions page loads actual Claude sessions
- Selecting a row updates the detail pane
- Loading and empty states look intentional

### Phase 5: Session Preview And Detail Experience

1. Build `SessionRow` and `SessionStateBadges`.
2. Build `SessionDetailPane` with tabs:
   - `Overview`
   - `Transcript`
   - `Sources`
3. Show preview content immediately from indexed data.
4. Hydrate full detail asynchronously when the detail pane needs it.
5. Keep preview visible while hydration is loading.
6. Add transcript rendering components:
   - human message row
   - assistant message row
   - tool event row
   - progress event row
7. Collapse verbose tool output by default.

Exit criteria:

- Session browsing feels fast
- Detail view is readable and stable
- Selection changes do not produce jarring UI resets

### Phase 6: Search, Filter, Sort, And Grouping

1. Implement `browse_sessions` and `search_sessions` command params fully.
2. Add search input with debounced query behavior.
3. Add filter controls for:
   - agent
   - project
   - date range
   - visibility
   - pinned
   - delete policy
4. Add sort modes and grouping modes.
5. Persist lightweight UI preferences:
   - last grouping mode
   - last sort mode
   - show hidden or not
6. Add frontend invalidation logic so sync events refresh queries without blowing away current UI state.

Exit criteria:

- User can reliably narrow large session histories
- Search remains session-oriented
- Filter and sort state survive simple refreshes

### Phase 7: Safe Session Management Actions

1. Implement Rust action services for:
   - pin
   - archive
   - hide
   - soft delete
2. Add `action_log` writes for each action.
3. Expose action commands to the frontend.
4. Build inline action buttons in the detail pane.
5. Build batch-safe actions in the sessions toolbar.
6. Add optimistic or near-immediate UI refresh behavior after safe actions.
7. Add toasts or inline feedback for action completion.

Exit criteria:

- Users can manage sessions without leaving the sessions page
- Action results are visible immediately
- Action logs appear in `System`

### Phase 8: Keyboard-First Polish

1. Add keyboard navigation for the sessions page:
   - list navigation
   - detail focus
   - search focus
   - filter open
   - selection toggle
2. Implement shortcuts for:
   - pin
   - archive
   - hide
   - soft delete
   - source inspector
   - sort and group cycling
3. Add a `?` help overlay.
4. Ensure dialogs trap focus correctly.
5. Test mixed mouse and keyboard workflows.

Exit criteria:

- Core session workflows are efficient without a mouse
- Keyboard hints are discoverable
- No major focus traps or accidental destructive behavior exist

### Phase 9: Codex Adapter

1. Implement Codex source discovery.
2. Parse rollout transcript files into the same unified model.
3. Enrich metadata from Codex SQLite or history sources where needed.
4. Handle source mapping carefully when one session depends on multiple upstream sources.
5. Add fixtures and tests covering:
   - normal sessions
   - partial metadata
   - multi-source sessions
6. Validate that search, browse, and detail hydrate behave the same as Claude sessions.

Exit criteria:

- Codex sessions appear alongside Claude sessions
- Cross-agent browse feels consistent
- Multi-source mapping is visible in the source inspector

### Phase 10: Source Inspector And Delete Planning

1. Implement Rust delete planning logic.
2. Add `ResolveDelete` or equivalent service that returns:
   - delete policy
   - target paths
   - reason
   - reversibility
3. Build the `Sources` tab in detail view.
4. Show:
   - source type
   - path
   - delete policy
   - why delete is or is not allowed
5. Add tests ensuring ambiguous or unsafe sessions return blocked policies.

Exit criteria:

- Users can understand source ownership clearly
- Delete trust is based on visible evidence
- Unsafe sessions are explicitly blocked from destructive deletion

### Phase 11: Destructive Delete Flow

1. Implement destructive delete command in Rust.
2. Validate all delete targets before execution.
3. Ensure delete only runs when backed by an approved delete plan.
4. Write action log entries for success and failure.
5. Build a dedicated destructive confirmation dialog in React using `AlertDialog`.
6. Default the dialog to cancel.
7. Refresh affected sessions after delete completes.

Exit criteria:

- Safe file-backed session delete works
- Unsafe delete is refused cleanly
- The user always sees exact targets before confirmation

### Phase 12: System Page And Operational Visibility

1. Build `System` page cards and panels for:
   - sync status
   - source health
   - recent actions
   - recent errors
2. Add a manual rescan control.
3. Surface parse and sync failures with enough detail to debug them.
4. Show last successful scan times and source counts.

Exit criteria:

- Users can trust what the app is doing in the background
- Errors are visible without digging through logs

### Phase 13: Memory Section Minimal V1

1. Implement a very lightweight `Memory` section.
2. Recommended initial behavior:
   - show pinned sessions
   - or show a placeholder for future curation
3. Do not let this section expand scope away from session management.

Exit criteria:

- IA is complete
- `Memory` exists without becoming a distracting second product

### Phase 14: Hardening And Polish

1. Improve empty states and onboarding copy.
2. Polish badge semantics and danger styling.
3. Test large datasets and long transcripts.
4. Ensure sync refreshes do not reset user context unnecessarily.
5. Test delete edge cases:
   - source already missing
   - permissions failure
   - partial multi-source ownership
6. Audit Tauri capability and permission scope.
7. Add packaging and release build validation.

Exit criteria:

- App feels coherent and trustworthy
- Core session management workflows are resilient
- Packaging is ready for real usage

## Suggested First Week Breakdown

If you want a practical near-term cadence, this is a good first week:

### Day 1

- Bootstrap Tauri + React + shadcn/ui
- Add one test Rust command
- Create app shell

### Day 2

- Create SQLite schema and store
- Add browse and search placeholders

### Day 3

- Implement Claude discovery and minimal indexing
- Show real session rows in the UI

### Day 4

- Build detail pane with overview and transcript preview
- Add command wrappers and query invalidation basics

### Day 5

- Add search, filters, pin, archive, and hide
- Add basic system status and action logging

## Summary

This Tauri v2 design keeps the product centered on session management while using the platform more appropriately than a terminal-first design.

Rust is responsible for correctness, safety, indexing, and local source access. The frontend is responsible for turning that power into a clear management workflow.

The result should feel like a serious local operations tool for agent memory: fast to scan, safe to clean up, and trustworthy around destructive actions.
