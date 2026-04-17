# Session Visualization Design

## Context

Yeek stores session messages as a tree via `parent_id`. The transcript API returns `main_path` (primary conversation path) and `branches` (divergence points). Branch data is fetched but never rendered — users can only see the linear main path. A graph visualization will surface the full tree structure and tool call patterns.

## Scope

Add a "Visualization" tab to the session detail pane, rendering the message tree as an interactive graph with two views: Conversation Tree and Tool Flow.

## Entry Point

In `session-detail-pane.tsx`, add a third tab alongside History and Sources:

```
[ History ] [ Sources ] [ Visualization ]
```

Tab value: `"visualization"`. i18n key: `detail.tabVisualization` ("Visualization" / "可视化").

The tab renders `<SessionGraph sessionId={sessionId} />`, which fetches transcript data via the existing `getSessionTranscript(sessionId)` API.

## Views

### View 1: Conversation Tree

A top-to-bottom tree showing all messages connected by `parent_id`.

**Node types:**
- **User** — rounded rectangle, accent border, displays first 60 chars of message
- **Assistant** — rounded rectangle, different accent, displays first 60 chars + model badge
- **Tool** — small rectangle, monospace, displays tool name (Read, Bash, Agent…)
- **Meta** — dashed border, compact, displays event type (Plan mode, File edit, Compacted…)

**Edge behavior:**
- Edges connect parent_id → child, straight lines with step breakpoints
- Nodes on `main_path` get solid borders (highlighted)
- Branch nodes get lighter/dashed borders
- Branch points show a count badge (e.g., "3 branches")

**Layout:** dagre TB (top-to-bottom), node spacing horizontal 50px, vertical 80px.

### View 2: Tool Flow

A vertical flow showing only tool_use and tool_result messages in temporal order.

**Node types:**
- **Tool Call** — displays tool name + input summary (e.g., file path, command)
- **Agent** — special styling, expandable to show subagent call chain

**Edges:**
- Sequential order (top to bottom)
- Data dependencies (e.g., Read → Edit on same file) shown as dashed connections

**Layout:** dagre TB, narrower spacing.

### View switching

Two buttons at the top of SessionGraph: "Conversation Tree" / "Tool Flow". Default: Conversation Tree.

## Interaction

- **Pan/zoom** — React Flow built-in (drag canvas, scroll to zoom)
- **Click node** — opens a detail slide-over panel on the right side of the graph area:
  - User: full message text
  - Assistant: full message text + model info
  - Tool: tool name + formatted input JSON + output summary
  - Meta: event description
- **Double-click branch point** — switch active branch at that point, highlighting the selected branch path
- **Fit View button** — resets viewport to show all nodes

## Data Flow

```
SessionGraph
  ├── useQuery(["session-transcript", sessionId]) → TranscriptPayload
  ├── buildConversationTree(messages, mainPath, branches) → { nodes, edges }
  ├── buildToolFlow(messages) → { nodes, edges }
  └── ReactFlow with computed nodes/edges
```

Layout computation runs once after data loads, stored in component state. View switching swaps the nodes/edges set.

## File Structure

```
src/pages/sessions/
  session-graph.tsx              — Main container: loads transcript, view toggle, fit view
  graph/
    conversation-tree.tsx        — Conversation tree view (React Flow canvas)
    tool-flow.tsx                — Tool flow view (React Flow canvas)
    nodes/
      user-node.tsx              — Custom User node component
      assistant-node.tsx         — Custom Assistant node component
      tool-node.tsx              — Custom Tool node component
      meta-node.tsx              — Custom Meta/Event node component
    layout.ts                    — dagre layout computation utility
    node-detail-panel.tsx        — Slide-over panel for node details
```

## Dependencies

- `@xyflow/react` — React Flow graph rendering (~50KB gzipped)
- `@dagrejs/dagre` — DAG layout algorithm (~10KB gzipped)

## Modified Files

- `src/pages/sessions/session-detail-pane.tsx` — add Visualization tab
- `src/i18n/locales/en.json` + `zh-CN.json` — new graph-related translation keys

## Translation Keys (new)

```
detail.tabVisualization       — "Visualization" / "可视化"
graph.conversationTree        — "Conversation Tree" / "对话树"
graph.toolFlow                — "Tool Flow" / "工具流"
graph.fitView                 — "Fit View" / "适配视图"
graph.nodeDetail              — "Node Detail" / "节点详情"
graph.noData                  — "No messages to visualize" / "没有可可视化的消息"
graph.branches                — "{{count}} branches" / "{{count}} 个分支"
```

## Visual Style

Follows the Zed-inspired design system from DESIGN.md:
- Achromatic palette: grays from `#171717` to `#ffffff`
- Node borders: shadow-as-border (`box-shadow: 0px 0px 0px 1px rgba(0,0,0,0.08)`)
- Fonts: Geist Sans, 12-14px for node labels
- Node backgrounds: `bg-[var(--editor)]` for nodes, `bg-secondary` for tool nodes
- Active path: slightly brighter background + solid border
- Detail panel: matches existing surface-card style

## Edge Cases

- **Empty transcript:** show "No messages to visualize" placeholder
- **Single message:** render one node, center it
- **Very wide trees (>10 branches at one level):** dagre handles layout; horizontal scroll covers overflow
- **Subagent nodes:** Tool Flow shows Agent nodes with a "→" indicator; expanding fetches subagent messages via `getSubagentMessages()`
- **Sidechain messages:** included in graph as lighter-colored nodes off the main path
