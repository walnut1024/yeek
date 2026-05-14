# OpenCode Adapter Design

Add a new adapter to Yeek that reads OpenCode agent sessions, following the same convention as the existing Claude Code and Codex adapters.

## Context

OpenCode is an open-source AI coding agent. Unlike Claude Code (centralized JSONL) and Codex (centralized JSONL), OpenCode stores all session data in a **single centralized SQLite database** per user at the XDG data directory (`~/Library/Application Support/opencode/opencode.db` on macOS).

The database contains normalized tables: `session`, `message`, `part`, `project`, and others. Sessions link to projects via `project_id`, and subagent sessions link to parents via `parent_id`.

## Approach: Direct rusqlite Read

Open the OpenCode SQLite database directly using rusqlite (already a dependency) and execute SQL queries to extract sessions and messages. No intermediate format, no JSON parsing of the session files themselves.

**Why not JSONL export:** OpenCode's data is relational SQLite — exporting to JSONL would add an unnecessary layer, duplicate work, and risk data loss. Yeek already uses rusqlite extensively, so direct SQL reads are natural.

## Source Discovery

Single file at a known path:

```
~/Library/Application Support/opencode/opencode.db   (macOS)
~/.local/share/opencode/opencode.db                   (Linux)
```

Resolved via the `dirs` crate (already a dependency). `discover_sources()` checks for the file's existence and returns one `SourceDescriptor` with `source_type: "opencode_db"`.

Fingerprint: `"{file_size_bytes}:{modified_time_millis}"` — since the entire DB changes when any session is updated, a full re-scan is needed on every change. Unchanged fingerprints are skipped (same pattern as existing adapters).

## Data Model Mapping

### Sessions: OpenCode → Yeek `SessionRecord`

| OpenCode column | Yeek field | Transform |
|---|---|---|
| `session.id` | `id` | Direct |
| — | `agent` | Constant `"opencode"` |
| `project.worktree` | `project_path` | JOIN `session.project_id = project.id` |
| `session.title` | `title` | Direct |
| `session.model.id` (JSON) | `model` | Parse JSON `{"id": "..."}`, extract `.id` |
| — | `git_branch` | `None` (not in OpenCode schema) |
| `session.time_created` | `started_at` | Unix ms → ISO 8601 |
| `session.time_updated` | `ended_at` | Unix ms → ISO 8601 |
| `session.parent_id` | `parent_session_id` | Direct |
| — | `status` | `Active` if `time_archived` is NULL, else `Complete` |
| — | `message_count` | Count from query |

Subagent sessions (`parent_id IS NOT NULL`) are included. Their IDs are prefixed as `{parent_id}:{child_id}` for uniqueness within Yeek's unified namespace.

### Messages: OpenCode → Yeek `MessageRecord`

Each row in OpenCode's `message` table becomes one `MessageRecord`. Content details come from the `part` table:

| OpenCode | Yeek field | Notes |
|---|---|---|
| `message.id` | `id` | Direct |
| `message.session_id` | `session_id` | Direct |
| `message.data.role` (JSON) | `role` | `"user"` → `"human"`, `"assistant"` → `"assistant"` |
| Part `type="text"` | `content_preview` | Concatenate text parts, truncate to ~500 chars |
| Part `type="tool"` | `kind="tool_use"`, `tool_name` | Extract from `data.tool` |
| Part `type="tool"` with completed state | `kind="tool_result"` | Extract output from `data.state.output` |
| `message.time_created` | `timestamp` | Unix ms → ISO 8601 |
| Subagent session | `is_sidechain` | `true` if `session.parent_id IS NOT NULL` |
| Part `type="reasoning"` | `kind="thinking"` | Extract thinking text |

## Architecture

### New file: `src-tauri/src/adapter/opencode/mod.rs`

Three public functions matching the existing adapter convention:

1. **`discover_sources() -> Result<Vec<SourceDescriptor>>`** — Check XDG path for `opencode.db`. Return single descriptor or empty vec.

2. **`index_sources(conn, sources, on_progress) -> Result<IndexResult>`** — For each source:
   - Open OpenCode's SQLite with `PRAGMA query_only = ON` and `PRAGMA journal_mode = WAL` for safe concurrent reads
   - Query `session JOIN project` for all sessions (including subagents via `parent_id`)
   - For each session, query `message` and `part` tables to build `MessageRecord`s
   - Upsert into Yeek's `sessions`, `messages`, `sources`, `session_sources` tables
   - Use SAVEPOINT per source for atomicity (same pattern as Claude Code/Codex adapters)

3. **`source_descriptor_from_path(path) -> Option<SourceDescriptor>`** — Return `Some` if path ends with `opencode.db`, `None` otherwise.

### Integration points

1. **`adapter/mod.rs`** — Add `pub mod opencode;`
2. **`sync/background.rs`** — Add `opencode::discover_sources()` and `opencode::index_sources()` calls alongside existing Claude/Codex calls
3. **`sync/watcher.rs`** — Add OpenCode routing in `run_incremental_scan`: try `opencode::source_descriptor_from_path(p)` first (since it checks for `opencode.db`), then Codex, then Claude Code
4. **`lib.rs`** — Add file watcher for `~/Library/Application Support/opencode/` directory if it exists

## SQLite Concurrency

OpenCode runs with WAL mode enabled. Reading its database while OpenCode is writing is safe — WAL allows concurrent readers. The adapter opens with `PRAGMA query_only = ON` to guarantee no accidental writes, and uses `OPEN_READONLY` flag on rusqlite's `Connection::open_with_flags`.

## Files Changed

| File | Change |
|---|---|
| `src-tauri/src/adapter/opencode/mod.rs` | **New** — adapter implementation (~300-400 lines) |
| `src-tauri/src/adapter/mod.rs` | Add `pub mod opencode;` |
| `src-tauri/src/sync/background.rs` | Add OpenCode discover + index calls |
| `src-tauri/src/sync/watcher.rs` | Add OpenCode path routing + watcher import |
| `src-tauri/src/lib.rs` | Add watcher for XDG opencode directory |
