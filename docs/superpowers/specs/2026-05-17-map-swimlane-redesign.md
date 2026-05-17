# Map Swimlane Redesign — Design Spec

## Context

The session Map view is being redesigned from a relationship-tree visualization into a chronological swimlane grouped by user turns. The current uncommitted refactor under `src/pages/sessions/session-relationship-graph/` still keeps too much of the old pure-SVG/D3 rendering model, which makes the failed refactor hard to finish cleanly.

This refactor should treat the user turn as the primary unit of layout. Parent/child `parent_id` relationships can still inform local ordering inside a turn, but the visual goal is not to preserve a full cross-turn graph.

## Goal

Redesign the Map view as a five-column HTML+SVG hybrid chronological swimlane layout with:
1. Tool-group collapse/expand (show first N tools, fold the rest)
2. Left-aligned timestamp column
3. Visually distinct swimlanes with headers and background tints
4. Native vertical scrolling (no d3.zoom)

## Architecture: HTML + SVG Hybrid

### Why Hybrid over Pure SVG

- **Fixed column headers**: HTML `sticky` positioning keeps headers visible during scroll
- **Native collapse buttons**: HTML `<button>` gives free hover/focus/keyboard accessibility
- **CSS swimlane backgrounds**: `background-color` + `border-right` on `<div>` beats SVG `<rect>`
- **Readable time axis**: HTML text doesn't degrade when zoomed
- **No zoom needed**: Swimlane layout fits naturally in vertical scrolling

### Overall Structure

```
┌──────────────────────────────────────────────────────────────────┐
│ Header Row (sticky top-0, z-10)                                  │
│ ┌──────┬──────────┬───────────┬───────────┬──────────┐          │
│ │ Time │ User     │ Assistant │ Sub-agent │ Tools    │          │
│ └──────┴──────────┴───────────┴───────────┴──────────┘          │
├──────────────────────────────────────────────────────────────────┤
│ Scrollable Content (overflow-y: auto)                            │
│                                                                  │
│ ┌──────┬──────────┬───────────┬───────────┬──────────┐          │
│ │14:02 │ [User]   │ [Asst]    │           │ [Read]   │          │
│ │      │ Fix bug  │ I'll fix  │           │ [Edit]   │          │
│ │      │          │           │           │ ▸ 3 more │ ← fold  │
│ ├──────┼──────────┼───────────┼───────────┼──────────┤          │
│ │14:05 │ [User]   │ [Asst]    │ [Agent]   │ [Write]  │          │
│ │      │ Add test │ Adding    │ Explore   │ [Bash]   │          │
│ └──────┴──────────┴───────────┴───────────┴──────────┘          │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## Column Layout

| Column | Width | Content |
|--------|-------|---------|
| Time | 60px | Timestamp per Turn (only on User row) |
| User | 240px | Human messages, blue tint `rgba(37,99,235,0.03)` |
| Assistant | 280px | Assistant text responses, green tint `rgba(5,150,105,0.03)` |
| Sub-agent | 240px | Agent tool calls, purple tint `rgba(199,125,186,0.03)` |
| Tools | 240px | Tool use/result, amber tint `rgba(242,204,143,0.03)` |

Total width: ~1060px. Centered in the container with horizontal scroll if viewport is narrower.

Column separators: `border-right: 1px solid rgba(0,0,0,0.06)` on each column cell.

## Turn Layout

Each Turn is a horizontal CSS grid row, for example `<div class="turn-row">`, with five column cells:

- **Time cell**: Shows formatted timestamp from the Turn's first user message. Format: `HH:mm`. If the Turn has no user message or timestamp is null, shows `--:--`.
- **User cell**: One node card for the human message.
- **Assistant cell**: One or more node cards for assistant responses (stacked vertically if multiple).
- **Sub-agent cell**: Node cards for `tool_name === "Agent"` messages.
- **Tools cell**: Node cards for tool_use and tool_result messages.

### Turn Grouping

- A new Turn starts at each human/user message.
- Messages before the first user message go into a synthetic prelude Turn with ID `prelude`.
- Messages after a user message belong to that Turn until the next user message.
- Assistant continuations, Agent tool calls, tool uses, and tool results stay in the current Turn.
- Tool results without a visible matching tool call still render in the Tools column of the current Turn.
- `Turn.id` is the first user message ID. For a synthetic Turn, use `prelude`; if a Turn has no user message but has visible messages, use the first visible message ID with a `turn-` prefix.

### Y Alignment Within a Turn

- User and Assistant column stacks start at the same vertical position (top-aligned).
- Sub-agent cards appear in their own column, stacked vertically after the first Assistant card's top edge.
- Tool cards appear in their own column, stacked vertically after the first Assistant or Sub-agent card's top edge.
- Tool cards stack vertically with 8px gap.
- The row height is the maximum content height of the five column cells. Avoid absolute positioning for cards unless the row height is measured and updated.

### Turn Spacing

- Between Turns: 16px gap with a `border-top: 1px solid rgba(0,0,0,0.05)` separator.
- Within a Turn: 8px gap between stacked cards.

## Node Cards

Each node is an HTML `<div>` with:
- Rounded corners: `border-radius: 8px` for user/assistant, `6px` for tools/subagents
- Border: 1px solid with type-specific color
- Background: type-specific tint
- Label: truncated text, max 2 lines with `line-clamp: 2`
- Font: 12px for user/assistant, 11px for tools

### Badges

- **Tool name**: Right-aligned, 9px, in tool-specific color
- **Model**: Bottom-right, 9px, muted, for assistant nodes
- **Agent**: Right-aligned, 9px, purple, for subagent nodes

### Branch Indication

Non-main-path nodes:
- `border-style: dashed`
- `opacity: 0.6`

## Tool Group Collapse

### Rule

Within each Turn's Tools column:
- Show the first **3** tool cards (configurable via `MAP_TOOL_COLLAPSE_THRESHOLD`)
- If there are more than 3 tools, show a collapse button: `"▸ N more tools (click to expand)"`
- Clicking the button reveals all remaining tools
- When expanded, the button changes to `"▴ Collapse"` to re-fold

### Collapse Button

```html
<button class="collapse-btn">
  ▸ 3 more tools
</button>
```

Styled as a minimal text button with muted color, full-width of the tools column, with `cursor: pointer` and `:hover` background change.

### State Management

Collapse state is tracked in React component state as a `Set<string>` of Turn IDs that are expanded. Default: empty set (all collapsed).

## Edges (Connections)

### Approach: Inline SVG per Turn

Each Turn row may contain a small SVG overlay that draws edges between nodes within the same Turn. This avoids the complexity of a single large SVG spanning all Turns. Edges are a local readability aid, not the source of truth for cross-turn hierarchy.

### Edge Types

1. **User → Assistant**: Horizontal arrow from User cell right edge to Assistant cell left edge.
2. **Assistant → Sub-agent**: Horizontal arrow from Assistant to Sub-agent cell.
3. **Assistant → Tool**: Horizontal arrow from Assistant to Tools cell (when no Sub-agent).
4. **Sub-agent → Tool**: Horizontal arrow from Sub-agent to Tools cell.
5. **Tool → Tool**: Vertical arrow between stacked tools in Tools column.

Edges are drawn as simple SVG `<line>` or `<path>` elements with arrowhead markers.

### Cross-Turn Edges

Not drawn. This is intentional because the Map is now a chronological swimlane grouped by user turns, not a full relationship graph. The Turn separator and time gap make the sequential flow obvious without explicit edges.

## Timeline

### Rendering

The Time column shows timestamps from `MessageRecord.timestamp`. Each Turn displays the timestamp of its first user message, formatted as `HH:mm`. Synthetic Turns without a user timestamp display `--:--`.

If consecutive Turns have timestamps within 1 minute of each other, the later one shows `"` (ditto mark) instead of repeating the time.

### Time Gaps

If the gap between consecutive Turns exceeds 5 minutes, show a time gap indicator:

```
  ... 5 min gap ...
```

Rendered as a centered, muted text between the two Turn rows.

## Header Row

Fixed at the top of the scroll container using `position: sticky; top: 0; z-index: 10`.

Five column headers matching the content columns:
- **Time**: Clock icon + "Time"
- **User**: User icon + "User"
- **Assistant**: Bot icon + "Assistant"
- **Sub-agent**: Git-branch icon + "Sub-agent"
- **Tools**: Wrench icon + "Tools"

Background: `var(--background)` with `border-bottom: 1px solid var(--border)`.

## Stats Bar

Keep the existing stats badges (Users, Assistants, Tools, Sub-agents) in the header bar above the swimlane container. No changes needed.

## Scrollable Container

```css
.map-scroll-container {
  overflow-y: auto;
  overflow-x: auto;
  max-height: 100%; /* fill available space */
}
```

Remove `d3.zoom()` entirely. The fit-view button can scroll to top instead.

## Files to Modify

### `src/pages/sessions/session-relationship-graph/build-session-tree.ts`
- Change output from flat `{ nodes, edges }` to `{ turns: Turn[] }` where each Turn contains its classified entries grouped by column
- Keep all tool entries in the Turn data. Do not remove hidden tools from the model; collapse is only a render concern.
- Add `timestamp` to Turn from the first user message
- Add stable `Turn.id` using the rules in "Turn Grouping"
- Remove `d3` dependency entirely
- Remove `column`, `x`, `y` from MapNode (layout is now CSS-driven)

### `src/pages/sessions/session-relationship-graph/session-relationship-graph.tsx`
- Replace SVG rendering with HTML+CSS layout
- Add collapse state management
- Add Turn iteration rendering
- Keep existing data fetching and stats

### `src/pages/sessions/session-relationship-graph/svg-renderer.ts`
- Prefer removing the old full-canvas renderer entirely
- If inline per-Turn edges are kept, replace this with a small edge helper that has no D3 dependency and no zoom/fit-view behavior

### `src/lib/d3-utils.ts`
- Remove `setupZoom` and `fitView` if no longer used elsewhere
- Check for other consumers first

### `src/pages/sessions/session-relationship-graph/__tests__/build-session-tree.test.ts`
- Update to test new Turn-based output
- Verify Turn grouping starts at user messages and handles prelude/orphan records
- Verify tool collapse threshold
- Verify timestamp extraction

### `src/pages/sessions/graph/node-detail-panel.tsx`
- May remain in place if imports still resolve
- If `src/pages/sessions/graph/` is otherwise deleted, move this file into `src/pages/sessions/session-relationship-graph/` and update imports
- Node click behavior must keep working for visible and expanded tool cards

## Verification

1. `npm run build` passes
2. `npx vitest run src/pages/sessions/session-relationship-graph/__tests__/` passes
3. `cargo tauri dev` → select session → Map tab:
   - Five columns visible with headers and distinct background tints
   - Timeline shows timestamps in left column
   - Tools beyond threshold 3 are collapsed with expand button
   - Clicking expand shows all tools
   - Scrolling works naturally
   - Node click opens detail panel
