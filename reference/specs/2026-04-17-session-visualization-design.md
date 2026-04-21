# Session Visualization Design

## Context

Yeek stores session messages as a tree via `parent_id`. The transcript API returns `main_path` (primary conversation path) and `branches` (divergence points). Branch data is fetched but never rendered — users can only see the linear main path. A graph visualization will surface the full tree structure as a unified conversation tree where tool calls are inline (since tools are triggered by assistant messages).

## Scope

Add a Graph/Feed view toggle to the session detail pane's History tab, switching the conversation history between a linear feed (current view) and an interactive graph.

## Entry Point

In `session-detail-pane.tsx`, within the History tab header area, add a Graph/Feed toggle button group:

```
[ Graph ] [ Feed ]
```

- **Feed** (default) — the existing linear transcript view (`transcript-view.tsx`)
- **Graph** — the new interactive graph view (`session-graph.tsx`)

The toggle is a UI preference persisted per-session (or globally) via localStorage. When "Graph" is selected, the History tab content area renders `<SessionGraph sessionId={sessionId} />` instead of the transcript feed. Both views share the same transcript data fetched via `getSessionTranscript(sessionId)`.

## Unified Conversation Tree

A single top-to-bottom tree showing all messages connected by `parent_id`. Tool calls appear inline as children of the assistant messages that triggered them — no separate view needed.

**Node types:**
- **User** — rounded rectangle, accent border, displays first 55 chars of message
- **Assistant** — rounded rectangle, different accent, displays first 55 chars + model badge
- **Tool Use** — small rectangle, tool-specific color, displays tool name (Read, Bash, Agent…)
- **Tool Result** — compact rectangle, muted, displays truncated output
- **Meta** — dashed border, compact, displays event type (Plan mode, File edit, Compacted…)

**Filtering (re-parenting algorithm):**
1. First pass: determine which messages to keep. Skip verbose system attachments (mcp_instructions_delta, skill_listing, superpowers, claude-md, context).
2. Build re-parent map: for each kept node, walk up the parent chain until finding a kept ancestor.
3. Second pass: create nodes and edges using effective parents from the re-parent map.

This ensures skipped nodes don't break the edge chain — their children get re-routed to the nearest visible ancestor.

**Tool color coding:**
Each tool has a distinct color for its node border and label:
- Bash: warm terracotta, Read/Write/Edit: amber, Grep/Glob: sage green
- Agent/SendMessage: purple, WebSearch: light blue, Skill/CronCreate: lavender
- TaskCreate/TaskUpdate/TaskOutput/TaskList: teal

**Edge behavior:**
- Edges connect effective parent → child (after re-parenting)
- Smooth-step edges with arrow markers
- Color: `#555` for visibility on dark backgrounds

**Layout:** dagre TB (top-to-bottom), node spacing horizontal 50px, vertical 70px.

## Interaction

- **Pan/zoom** — React Flow built-in (drag canvas, scroll to zoom), min zoom 0.05, max zoom 2
- **Click node** — opens a detail panel on the right side of the graph area:
  - User: full message text
  - Assistant: full message text + model info + timestamp
  - Tool: tool name (color-coded) + formatted input
  - Meta: event description
- **Fit View button** — resets viewport to show all nodes
- **Stats bar** — shows node count, user/assistant/tool counts in header

## Data Flow

```
SessionGraph
  ├── useQuery(["session-transcript", sessionId]) → TranscriptPayload
  ├── buildTree(messages) → { nodes, edges }
  │     ├── First pass: filter visible messages
  │     ├── Build re-parent map (walk up skipped ancestors)
  │     └── Second pass: create nodes + edges, apply dagre layout
  └── ReactFlow with computed nodes/edges
```

Layout computation runs once after data loads, stored in `useMemo`.

## File Structure

```
src/pages/sessions/
  session-graph.tsx              — Main container: loads transcript, fit view, stats
  graph/
    build-tree.ts                — Tree construction + re-parenting algorithm + dagre layout
    nodes/
      user-node.tsx              — Custom User node component
      assistant-node.tsx         — Custom Assistant node component
      tool-use-node.tsx          — Custom Tool Use node component
      tool-result-node.tsx       — Custom Tool Result node component
      meta-node.tsx              — Custom Meta/Event node component
    node-detail-panel.tsx        — Slide-over panel for node details
```

## Dependencies

- `@xyflow/react` — React Flow graph rendering (~50KB gzipped)
- `@dagrejs/dagre` — DAG layout algorithm (~10KB gzipped)

## Modified Files

- `src/pages/sessions/session-detail-pane.tsx` — add Graph/Feed toggle in History tab header, conditionally render graph vs feed
- `src/i18n/locales/en.json` + `zh-CN.json` — new graph-related translation keys
- `src/lib/api.ts` — if transcript API needs adjustments for graph data

## Translation Keys (new)

```
graph.viewGraph               — "Graph" / "图谱"
graph.viewFeed                — "Feed" / "对话"
graph.fitView                 — "Fit View" / "适配视图"
graph.nodeDetail              — "Node Detail" / "节点详情"
graph.noData                  — "No messages to visualize" / "没有可可视化的消息"
graph.stats                   — "{{count}} nodes" / "{{count}} 个节点"
```

## Visual Style

Follows the Zed-inspired design system from DESIGN.md:
- Achromatic palette: grays from `#171717` to `#ffffff`
- Node backgrounds: `#1e1e2e`, tool results: `#191926`
- Node borders: subtle colored borders (1px) matching node type
- Tool nodes: tool-specific accent colors for borders and labels
- Fonts: system font, 10-11px for node labels, monospace for tool names
- Detail panel: dark surface (`#1a1a2a`) with right-side slide-over

## Edge Cases

- **Empty transcript:** show "No messages to visualize" placeholder
- **Single message:** render one node, center it
- **Very wide trees (>10 branches at one level):** dagre handles layout; horizontal scroll covers overflow
- **Subagent nodes:** Agent tool nodes show subagent name; expanding could fetch subagent messages
- **Sidechain messages:** included in graph as lighter-colored nodes off the main path

## Validated Demo

A working demo exists at `demo/` using real session data (153 messages, 52 tool calls, 4 branch points). The demo validates the unified tree approach, re-parenting algorithm, tool color coding, and interaction patterns described in this spec.
