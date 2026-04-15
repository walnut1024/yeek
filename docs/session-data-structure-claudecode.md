# Claude Code Session Data Structure

## Top-Level Directory Layout

```
~/.claude/projects/-Users-hipnusleo-Documents-Projects-apps-yeek/
├── {sessionId}.jsonl              ← Main session file (one per Claude Code session)
├── {sessionId}/                   ← Session auxiliary directory (active sessions only)
│   ├── subagents/                 ← Sub-agent sessions
│   │   ├── agent-{id}.jsonl       ← Sub-agent conversation content
│   │   └── agent-{id}.meta.json   ← Sub-agent metadata {agentType, description}
│   └── tool-results/              ← Large tool output staging
│       └── {randomId}.txt         ← Tool result content exceeding threshold
└── memory/                        ← Cross-session persistent memory
    ├── MEMORY.md                  ← Memory index
    └── *.md                       ← Individual memory files
```

## Message Chain Structure (`parentUuid` in JSONL)

Each JSONL record forms a **singly-linked list** via `parentUuid` → `uuid`, creating a tree:

```
ROOT (parentUuid=null)
 ├── permission-mode                    ← Session permission mode
 ├── attachment (hook_success)          ← SessionStart hook result
 ├── attachment (hook_additional_ctx)   ← SessionStart additional context
 └── file-history-snapshot              ← Tracked file snapshot
      │
      ▼ (parentUuid chain)
     user (caveat) ──→ user (command) ──→ system (local_command) ──→ ...
      │
      ▼
     user (user input, promptId=P1)     ← A "prompt turn" begins
      │
      ├── attachment (Read result, etc.) ← Attachments follow user message
      │
      ▼
     assistant (thinking)               ← Thinking content
      │
      ▼
     assistant (text + tool_use)        ← Text reply + tool calls
      │
      ├── assistant (tool_use: Bash)    ← Multiple tool calls in parallel
      ├── assistant (tool_use: Read)
      │
      ├── user (tool_result → Bash)     ← Tool results point back to assistant
      ├── user (tool_result → Read)
      │
      ▼
     attachment (system-reminder)       ← System-injected context reminder
      │
      ▼
     assistant (continue reply...)      ← May continue calling tools or end
      │
      ▼
     system (stop_hook_summary)         ← Turn end hook
      │
      ▼
     system (turn_duration)             ← Turn stats (durationMs, messageCount)
      │
      ▼ (next promptId turn)
     user (new user input, promptId=P2)
      └── ...
```

## Key Fields

| Field | Meaning |
|-------|---------|
| `uuid` | Unique identifier for this message |
| `parentUuid` | Points to previous message's uuid; `null` = root node |
| `logicalParentUuid` | Used in `compact_boundary`, points to logical parent before compression |
| `promptId` | Identifies a "user prompt turn"; shared across user/user(tool_result) in same turn |
| `isSidechain` | `true` = sub-agent conversation, `false` = main session |
| `isMeta` | `true` = session metadata (e.g. caveat prefix), not part of conversation logic |
| `sessionId` | Owning session ID |
| `agentId` | Sub-agent only; identifies which agent instance |

## Message Types (`type`)

| type | Description |
|------|-------------|
| `permission-mode` | Session permission mode (default/bypass, etc.) |
| `attachment` | Attachments: hook results, system-reminder, Read tool results, etc. |
| `file-history-snapshot` | File history snapshot (tracks edited files) |
| `user` | User message / tool results (`tool_result` in content) |
| `assistant` | Assistant reply (text / thinking / tool_use in content) |
| `system` | System messages: `local_command`, `stop_hook_summary`, `turn_duration`, `compact_boundary`, `api_error` |
| `compact_boundary` | Context compression marker; `logicalParentUuid` points to pre-compression logical chain |
| `queue-operation` | Async task queue operation (background task notification enqueue) |
| `last-prompt` | Record of the last user input in session |
| `custom-title` | User/system assigned session title |
| `agent-name` | Session display name |

## Tool Call Pattern

```
assistant (content: [{type: "tool_use", id: "call_xxx", name: "Bash", input: {...}}])
    │
    ▼ parentUuid points to the assistant above
user (content: [{type: "tool_result", tool_use_id: "call_xxx", content: "..."}])
```

For parallel tool calls: multiple `assistant(tool_use)` and `user(tool_result)` form a fan-out/fan-in structure via `parentUuid` chains.

## Sub-agents

```
Main session {sessionId}.jsonl
  │
  └── agent spawned → {sessionId}/subagents/agent-{agentId}.jsonl
        │
        ├── isSidechain: true        ← All messages are true
        ├── agentId: "{agentId}"     ← Matches ID in meta.json
        │
        └── Same message structure as main session (user/assistant/system chain)
```

`meta.json` provides quick indexing:
```json
{"agentType": "Explore", "description": "Explore Claude Code session format"}
```

## Context Compression (Compact)

Triggered automatically when token count approaches context window:
```json
{
  "type": "system",
  "subtype": "compact_boundary",
  "logicalParentUuid": "...",
  "compactMetadata": {
    "trigger": "auto",
    "preTokens": 169414
  }
}
```
- `parentUuid` resets to `null` (chain breaks)
- `logicalParentUuid` maintains logical continuity
- Post-compression messages link to pre-compression messages via `logicalParentUuid`

## Memory System

Independent from session JSONL, persists across sessions:
- `MEMORY.md` — Index file, one line per memory entry with title and link
- `*.md` — Memory files with YAML frontmatter (`name`, `description`, `type`)
- `originSessionId` — Session ID where the memory was created

## Hierarchy Summary

```
Project (~/.claude/projects/{project}/)
 └── Session ({sessionId}.jsonl, flat file)
      ├── Metadata layer: permission-mode, file-history-snapshot, custom-title, agent-name
      ├── Conversation layer: user ↔ assistant chain (linked via parentUuid)
      │   ├── Tool calls: assistant(tool_use) → user(tool_result)
      │   ├── Attachments: attachment interspersed between messages
      │   └── System events: system(local_command, stop_hook, turn_duration)
      ├── Compression layer: compact_boundary (parentUuid breaks, logicalParentUuid preserves order)
      └── Sub-agent layer: subagents/agent-{id}.jsonl (isSidechain=true)
           └── Same structure as main session, but in separate JSONL file
```
