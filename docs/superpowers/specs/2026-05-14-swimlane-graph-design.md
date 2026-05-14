# Swimlane Graph View Design

Replace the linear graph layout with a three-swimlane topology view that shows role hierarchy and branch structure.

## Problem

`session-graph.tsx` filters to `main_path` only, discarding all `transcript.branches` data. The result is a flat linear chain — 106 messages with 10 branch points render as a single column with no visible topology.

## Solution: Three-Swimlane Layout

React Flow with constrained column positioning. Three fixed lanes (User / Assistant / Tool), branches expand right, timeline flows downward.

### Column Assignment

| Role | x position |
|------|-----------|
| User messages | 0 |
| Assistant messages | COL_WIDTH (220px) |
| Tool (tool_use + tool_result) | 2 × COL_WIDTH (440px) |
| Branch nodes | 3 × COL_WIDTH + branchOffset |

`COL_WIDTH = 220px`

### Y-Axis Positioning

- Messages ordered by main_path sequence, `ROW_HEIGHT = 70px` per step
- `tool_result` follows its `tool_use` with compact spacing (half row)
- Branch node y aligns to its branch point's y position

### Edges

- Cross-column: horizontal arrows (left→right = call, right→left = return)
- Same-column: implicit via y ordering, no vertical edges needed
- Main path: solid edges
- Branch: dashed edges

### Branch Points

- Marked with numbered badges (①②③)
- Branch siblings rendered in the 4th+ columns
- `transcript.branches` data drives branch node creation

### Node Handles

Current: `Position.Top` (target) + `Position.Bottom` (source) — vertical only.

New: Add `Position.Left` and `Position.Right` handles to support horizontal cross-column edges.

## Files to Change

1. **`build-tree.ts`** — Replace dagre layout with swimlane algorithm. Add branch node processing from `transcript.branches`.
2. **`session-graph.tsx`** — Remove `main_path` filter. Pass full messages + branches to `buildTree`. Update edge options for horizontal flow.
3. **`nodes.tsx`** — Add left/right handles on all node types.
4. **`node-detail-panel.tsx`** — Show branch info (parent_id, sibling index).

## Data Flow

```
TranscriptPayload { messages, main_path, branches }
  ↓
session-graph.tsx passes ALL messages (not just main_path)
  ↓
buildTree(messages, branches, mainPath)
  ↓ assign x by role, y by main_path order
  ↓ create branch nodes from branches[].siblings
  ↓ create edges (horizontal + branch)
  ↓
ReactFlow renders with constrained positions
```

## Constraints

- Max 300 nodes (`GRAPH_MAX_NODES`), same as current
- Must preserve zoom/pan/fitView via React Flow
- Node detail panel on click still works
- Dark theme via CSS variables (not hardcoded colors)
