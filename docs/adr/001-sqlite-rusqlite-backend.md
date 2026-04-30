# ADR 001: SQLite + rusqlite (bundled) as storage backend

- **Status**: Accepted
- **Date**: 2026-04-28
- **Deciders**: hipnusleo

## Context

Yeek indexes and queries local Claude Code agent session data (~/.claude/projects/).
Sessions are stored as JSONL files on disk; the backend must:

1. Parse and index thousands of sessions efficiently
2. Support full-text search (titles, messages, model names)
3. Run entirely local — no external database server
4. Work across macOS (primary), Linux, and Windows
5. Embed in a Tauri desktop app (no server process dependency in Tauri mode)

## Decision

**Use SQLite via `rusqlite` with the `bundled` feature.**

- `rusqlite` provides synchronous, blocking SQLite access
- `bundled` compiles SQLite from source, eliminating system-library compatibility issues
- FTS5 extension enabled for full-text search on messages
- `Mutex<Connection>` wraps the single connection for thread-safe access from Tauri command handlers and HTTP server routes

## Alternatives Considered

| Alternative | Rejected Because |
|-------------|-----------------|
| PostgreSQL / MySQL | Requires external server; violates "local-first" requirement |
| `sqlx` (async SQLite) | Adds compile-time overhead; async SQLite provides no throughput benefit for local single-writer workloads |
| `rusqlite` without `bundled` | System SQLite may lack FTS5 or have version mismatches across platforms |
| In-memory only | Session data must persist across app restarts |
| Plain file scanning (no DB) | FTS5 provides orders-of-magnitude faster full-text search than `grep` over JSONL |

## Consequences

### Positive
- Zero external dependencies beyond SQLite source (compiled in)
- FTS5 provides fast ranked full-text search
- Single-file database simplifies backup and migration
- `bundled` ensures consistent SQLite version across all platforms

### Negative
- `Mutex<Connection>` serializes all DB access; concurrent readers block each other
- FTS5 external content table migration (~34MB savings) requires a heavy background migration
- Synchronous API means each query blocks the calling thread; acceptable for local SQLite but would not scale to high-concurrency server workloads

## Related

- `src-tauri/src/store/schema.rs` — schema initialization and migrations
- `src-tauri/src/store/sessions.rs` — session CRUD with FTS5 search
- `src-tauri/src/store/messages.rs` — message storage and FTS rebuild
