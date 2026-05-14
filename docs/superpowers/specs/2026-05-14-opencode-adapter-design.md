# OpenCode Adapter Design

## Goal

Add first-class OpenCode session ingestion to Yeek. The adapter reads OpenCode's local SQLite database directly, converts root and child sessions into Yeek's existing `SessionRecord` and `MessageRecord` model, and integrates with the same full-scan and watcher pipeline used by the Claude Code and Codex adapters.

This is an ingestion-only feature. Yeek must not write to, migrate, compact, delete from, or otherwise mutate the OpenCode database.

## Current Context

Yeek currently has two source adapters:

- `claudecode`: reads Claude Code JSONL transcripts from `~/.claude/projects`.
- `codex`: reads Codex JSONL transcripts from `~/.codex/sessions` and `~/.codex/archived_sessions`.

OpenCode is different. Current vendored OpenCode code defines a normalized SQLite schema under `vendor/opencode/packages/opencode/src/session/session.sql.ts`:

- `session`: session metadata, including `project_id`, `parent_id`, `title`, `directory`, `agent`, `model`, `time_created`, `time_updated`, and `time_archived`.
- `project`: project/worktree metadata, including `worktree`.
- `message`: message metadata, with role and model details in JSON `data`.
- `part`: message parts, with text, reasoning, tool, file, snapshot, patch, and related payloads in JSON `data`.

OpenCode's default database path is `Global.Path.data/opencode.db`. The common runtime locations are:

- macOS desktop: `~/Library/Application Support/opencode/opencode.db`
- XDG/CLI: `${XDG_DATA_HOME:-~/.local/share}/opencode/opencode.db`
- channel builds: `${data_dir}/opencode-<channel>.db`
- explicit override: `OPENCODE_DB`

Yeek should discover existing default/channel DB files from the common data directories. `OPENCODE_DB` is only used when present in Yeek's own environment.

## Product Behavior

OpenCode sessions appear in Yeek like other agent sessions:

- Root sessions are listed in the normal session browser.
- Child sessions with `session.parent_id` are ingested and stored with `parent_session_id`, but they remain hidden from root browse results because Yeek already filters `parent_session_id IS NULL`.
- Search covers OpenCode message previews through the existing `messages_fts` rebuild path.
- Session detail, transcript view, and graph view reuse the existing Yeek message model.
- Destructive source deletion is not supported for OpenCode because one DB file can contain many sessions. Session hide/soft-delete behavior remains Yeek-local.

No resume command is required in this feature. If the UI later exposes OpenCode resume, that should be a separate design because `do_resume_session` currently only supports `claude_code`, `claude_code_subagent`, and `codex`.

## Source Discovery

Create `src-tauri/src/adapter/opencode/mod.rs` with adapter functions matching the existing convention:

```rust
pub(crate) fn discover_sources() -> Result<Vec<SourceDescriptor>, AppError>;
pub(crate) fn source_descriptor_from_path(path: &Path) -> Option<SourceDescriptor>;
pub(crate) fn index_sources<F>(
    conn: &rusqlite::Connection,
    sources: &[SourceDescriptor],
    on_progress: F,
) -> Result<IndexResult, AppError>
where
    F: Fn(i64);
```

Discovery rules:

- Candidate filenames are `opencode.db` and `opencode-*.db`.
- Candidate directories are:
  - `OPENCODE_DB` if set and absolute, or joined to the OpenCode data directory if relative.
  - `~/Library/Application Support/opencode` on macOS.
  - `${XDG_DATA_HOME}/opencode` when `XDG_DATA_HOME` is set.
  - `~/.local/share/opencode` as the XDG fallback.
- `source_descriptor_from_path` returns `Some` only for existing regular files whose filename is `opencode.db` or starts with `opencode-` and ends with `.db`.
- `SourceDescriptor` values use:
  - `source_type: "opencode_db"`
  - `agent: "opencode"`
  - `path`: absolute DB path
  - `fingerprint`: `"{file_size_bytes}:{modified_time_millis}"`
  - `last_modified`: metadata modified time as RFC3339

The fingerprint is coarse by design. A changed OpenCode DB triggers a full adapter pass for that DB.

## SQLite Read Strategy

Open the OpenCode DB with:

```rust
Connection::open_with_flags(
    path,
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
)
```

Then execute:

```sql
PRAGMA query_only = ON;
PRAGMA busy_timeout = 5000;
```

Do not run `PRAGMA journal_mode = WAL` on the OpenCode connection. That pragma can write database metadata; the adapter must stay strictly read-only.

Before querying, validate the minimum expected schema with `sqlite_master` or `PRAGMA table_info`:

- Required tables: `session`, `project`, `message`, `part`.
- Required columns:
  - `session`: `id`, `project_id`, `parent_id`, `directory`, `title`, `agent`, `model`, `time_created`, `time_updated`, `time_archived`
  - `project`: `id`, `worktree`
  - `message`: `id`, `session_id`, `time_created`, `data`
  - `part`: `id`, `message_id`, `session_id`, `time_created`, `data`

If validation fails, return one adapter error for that source and leave Yeek data unchanged for the source savepoint.

## Mapping

### Session Mapping

Use one Yeek session per OpenCode `session` row.

| OpenCode | Yeek | Rule |
|---|---|---|
| `session.id` | `SessionRecord.id` | Prefix as `opencode:{id}` to avoid cross-agent collisions. |
| constant | `agent` | `"opencode"` |
| `project.worktree` | `project_path` | Prefer joined `project.worktree`; fallback to `session.directory`. |
| `session.title` | `title` | Direct. Empty string becomes `None`. |
| `session.model` JSON | `model` | Prefer `model.id`; fallback to `providerID/modelID`; fallback `None`. |
| `session.agent` | message/session metadata | Preserve in metadata where useful; Yeek `agent` stays `"opencode"`. |
| none | `git_branch` | `None`. |
| `session.time_created` | `started_at` | Unix milliseconds to RFC3339 UTC. |
| `session.time_updated` | `ended_at`, `updated_at` | Unix milliseconds to RFC3339 UTC. |
| `session.time_archived` | `status` | `Complete` if not null, else `Active`. |
| `session.parent_id` | `parent_session_id` | Prefix as `opencode:{parent_id}`. |
| current message count | `message_count` | Count `message` rows for this session. |
| Yeek defaults | visibility/delete fields | `Visible`, `false`, `None`, no archived/deleted timestamps. |

Do not create synthetic child IDs like `{parent_id}:{child_id}`. OpenCode session IDs are already globally unique inside the DB, and prefixing with `opencode:` is enough for Yeek's unified namespace.

### Message Mapping

Use one primary Yeek `MessageRecord` per OpenCode `message` row, plus optional extra records for significant tool/reasoning parts when needed for transcript fidelity.

Message base fields:

| OpenCode | Yeek | Rule |
|---|---|---|
| `message.id` | `id` | Prefix as `opencode:{message.id}`. |
| `message.session_id` | `session_id` | Prefix as `opencode:{session_id}`. |
| assistant `data.parentID` | `parent_id` | Prefix as `opencode:{parentID}`. User messages use `None`. |
| `message.data.role` | `role` | `"user"` -> `"human"`, `"assistant"` -> `"assistant"`. |
| `message.time_created` | `timestamp` | Unix milliseconds to RFC3339 UTC. |
| session has `parent_id` | `is_sidechain` | `true` for child sessions. |
| message JSON | `metadata` | Compact JSON with OpenCode role, agent, model/provider, and any error/finish fields. |

Content preview rules:

- Join `part.data.text` from `text` parts for the message.
- Include `reasoning` text only in separate `thinking` records, not in the normal assistant preview.
- For tool parts, include a concise line in the primary preview only if no text part exists.
- Truncate previews using the same safe UTF-8 character boundary approach used by existing adapters.
- Empty previews become a short type label such as `[tool: bash]`, `[reasoning]`, `[snapshot]`, or `[message]`.

Part-specific records:

- `part.data.type = "reasoning"` creates `kind: "thinking"`, `entry_type: "reasoning"`.
- `part.data.type = "tool"` creates:
  - `kind: "tool_use"` for pending/running tool state.
  - `kind: "tool_result"` for completed/error tool state.
  - `tool_name` from `data.tool`.
  - `content_preview` from `state.title`, `state.output`, or `state.error`, in that order.
- `snapshot`, `patch`, `file`, `agent`, `subtask`, `compaction`, `retry`, `step-start`, and `step-finish` parts may be folded into metadata or represented as concise system/attachment records if needed, but they must not break ingestion when unfamiliar fields appear.

Record IDs for part-derived messages must be deterministic:

```text
opencode:{message_id}:part:{part_id}
```

## Indexing Flow

`index_sources` follows the Codex/Claude adapter pattern:

1. Load active source fingerprints from Yeek's `sources` table.
2. Begin one Yeek transaction for all OpenCode sources.
3. For each changed source, create `SAVEPOINT sp_opencode_<i>`.
4. Open the OpenCode DB read-only and parse sessions/messages from that source.
5. For each parsed session:
   - `DELETE FROM messages WHERE session_id = ?` before reinserting that session's current messages. This prevents stale messages after OpenCode compaction, retry, or part removal.
   - Upsert `sessions`.
   - Upsert current messages.
   - Upsert `sources`.
   - Link `session_sources` with `delete_policy = "not_allowed"`.
6. Release or roll back the savepoint.
7. Record an `opencode_sync_completed` action with indexed/updated/error counts.
8. Commit the transaction.
9. If any source was indexed or updated, rebuild `messages_fts` using the existing global rebuild statement.

Stale OpenCode sessions missing from a later DB scan should not be hard-deleted from Yeek in the first version. They can remain visible until a future deletion/reconciliation design decides how to distinguish archived, compacted, and removed sessions safely.

## Integration Points

Backend integration:

- `src-tauri/src/adapter/mod.rs`: add `pub mod opencode;`.
- `src-tauri/src/sync/background.rs`: import `opencode`, discover OpenCode sources, include them in `total`, index after Codex, and add counts into the final summary.
- `src-tauri/src/sync/watcher.rs`: add `opencode_sources`; route changed `.db` files through `opencode::source_descriptor_from_path` before JSONL adapters; remove the current 10 MB skip for OpenCode DB files because the DB can legitimately exceed that limit.
- `src-tauri/src/lib.rs`: start file watchers for existing OpenCode data directories, not for every possible file. Watch directories that exist at startup.
- `src-tauri/src/app/commands.rs`: allow `"opencode"` in browse filters so the frontend can filter OpenCode sessions. Do not add resume support in this feature.

Frontend integration:

- Add `"opencode"` to any agent filter options and labels where Claude Code/Codex are listed.
- Display label: `OpenCode`.
- Do not add OpenCode-specific transcript UI. Use existing message kinds.

## Error Handling

- Missing DB: discovery returns an empty source list.
- Locked/busy DB: respect `busy_timeout`; if still unavailable, count one source error and keep existing Yeek rows.
- Unknown schema: count one source error with a clear log message naming the missing table/column.
- Invalid JSON in `session.model`, `message.data`, or `part.data`: skip only the malformed row or part, count it in source errors, and continue parsing the rest of the DB.
- Invalid timestamp: store `None` for optional timestamps; use current UTC only where Yeek requires `updated_at` and OpenCode has no usable value.

## Test Plan

Add unit tests in `src-tauri/src/adapter/opencode/mod.rs` using temporary SQLite fixtures:

- `source_descriptor_accepts_default_and_channel_db_names`: accepts `opencode.db` and `opencode-beta.db`; rejects unrelated `.db` files.
- `discover_sources_finds_existing_db`: finds a fixture DB in a controlled data directory helper.
- `indexes_root_session`: maps a root OpenCode session, project worktree, title, model JSON, timestamps, and text messages.
- `indexes_child_session`: maps `parent_id` to prefixed `parent_session_id` and marks messages `is_sidechain`.
- `maps_tool_and_reasoning_parts`: creates `thinking`, `tool_use`, and `tool_result` records with stable IDs and tool names.
- `skips_unchanged_fingerprint`: second index pass with same fingerprint produces no indexed/updated sessions.
- `rejects_missing_schema`: fixture without required tables rolls back the source savepoint and reports an error.
- `clears_stale_messages_on_reindex`: removing a part/message from the fixture DB and updating the fingerprint removes the stale Yeek messages for that session.

Run:

```bash
cargo test -p yeek opencode
cargo check
npm run typecheck
```

Manual checks:

- Start Yeek with an existing OpenCode DB present.
- Confirm root OpenCode sessions appear in browse.
- Confirm search finds text from OpenCode messages.
- Confirm child sessions are hidden from root browse but visible through session graph/detail where parent relationships are used.
- Confirm OpenCode DB files are never deleted through destructive session cleanup.

## Non-Goals

- No OpenCode resume command.
- No OpenCode write-back, migration, compaction, or deletion.
- No new Yeek database schema migration.
- No special UI for OpenCode-only part types in the first version.
- No JSONL export/import bridge.
