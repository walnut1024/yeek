# Swimlane Graph View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the linear graph layout with a three-swimlane topology that shows role hierarchy (User/Assistant/Tool) and branch structure.

**Architecture:** Replace Dagre auto-layout with a custom swimlane algorithm that assigns x by role (3 fixed columns) and y by main_path order. Branch nodes expand right from a 4th column. All rendering stays in React Flow — only the layout computation and data filtering change.

**Tech Stack:** React Flow (@xyflow/react), TypeScript, existing node components

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/pages/sessions/graph/build-tree.ts` | **Rewrite** | Swimlane layout algorithm, branch processing, edge creation |
| `src/pages/sessions/graph/nodes.tsx` | **Modify** | Add left/right Handles to all node types |
| `src/pages/sessions/session-graph.tsx` | **Modify** | Remove main_path filter, pass branches + main_path to buildTree, update edge styles |
| `src/pages/sessions/graph/node-detail-panel.tsx` | **Modify** | Show branch info |
| `src/pages/sessions/graph/__tests__/build-tree.test.ts` | **Modify** | Update existing tests for new API, add swimlane + branch tests |
| `src/i18n/locales/en.json` | **Modify** | Add new i18n keys for branches |
| `src/i18n/locales/zh-CN.json` | **Modify** | Add new i18n keys for branches |

---

### Task 1: Rewrite build-tree.ts — Swimlane Layout Algorithm

**Files:**
- Modify: `src/pages/sessions/graph/build-tree.ts`
- Test: `src/pages/sessions/graph/__tests__/build-tree.test.ts`

The core change. Replace Dagre with a custom swimlane layout. The function signature changes to accept `mainPath` and `branches` in addition to messages.

- [ ] **Step 1: Update buildTree signature and write swimlane layout**

Replace the entire `build-tree.ts` with the new implementation. Key changes:

```typescript
import type { Node as RFNode, Edge } from "@xyflow/react";
import type { MessageRecord, BranchPoint } from "@/lib/api";

// ─── Constants ──────────────────────────────────────────────────────

const COL_WIDTH = 220;
const ROW_HEIGHT = 70;
const COMPACT_HEIGHT = 45; // for tool_result紧跟tool_use
const W = 200; // node width

const TOOL_COLORS: Record<string, string> = {
  Bash: "#e07a5f",
  Read: "#f2cc8f",
  Write: "#f2cc8f",
  Edit: "#f2cc8f",
  Grep: "#81b29a",
  Glob: "#81b29a",
  Agent: "#c77dba",
  WebSearch: "#7fb8d8",
  Skill: "#b8a9c9",
  AskUserQuestion: "#6c8ebf",
  TaskCreate: "#7ec8b8",
  TaskUpdate: "#7ec8b8",
  TaskOutput: "#7ec8b8",
  TaskList: "#7ec8b8",
  SendMessage: "#c77dba",
  CronCreate: "#b8a9c9",
};

export function toolColor(name: string): string {
  return TOOL_COLORS[name] || "#888";
}

// ─── Helpers ────────────────────────────────────────────────────────

export function truncate(text: string, len = 48): string {
  if (!text) return "";
  const s = text.replace(/\n/g, " ").trim();
  return s.length > len ? s.slice(0, len) + "…" : s;
}

// ─── Node data types ────────────────────────────────────────────────

export type TreeNodeType = "user" | "assistant" | "toolUse" | "toolResult" | "meta";

export interface TreeNodeData {
  label: string;
  toolName?: string;
  model?: string;
  isBranch?: boolean;
  branchIndex?: number;
  [key: string]: unknown;
}

type GraphNode = RFNode<TreeNodeData>;

// ─── Subtypes to skip ───────────────────────────────────────────────

const SKIP_SUBTYPES = new Set([
  "mcp_instructions_delta",
  "skill_listing",
  "superpowers",
  "claude-md",
  "context",
]);

// ─── Column assignment ──────────────────────────────────────────────

function columnForNode(type: TreeNodeType): number {
  switch (type) {
    case "user": return 0;
    case "assistant": return 1;
    case "toolUse":
    case "toolResult": return 2;
    case "meta": return 1; // meta nodes go in assistant column
  }
}

// ─── buildTree (swimlane version) ───────────────────────────────────

export interface BuildTreeResult {
  nodes: GraphNode[];
  edges: Edge[];
  stats: { total: number; users: number; assistants: number; tools: number; branches: number };
}

export interface BuildTreeOptions {
  mainPath: string[];
  branches: BranchPoint[];
}

export function buildTree(
  messages: MessageRecord[],
  options?: BuildTreeOptions,
): BuildTreeResult {
  const mainPath = options?.mainPath;
  const branches = options?.branches ?? [];

  const nodes: GraphNode[] = [];
  const edges: Edge[] = [];
  const msgMap = new Map(messages.map((m) => [m.id, m]));

  // Build a set of branch message IDs for styling
  const branchMsgIds = new Set<string>();
  for (const bp of branches) {
    for (const sib of bp.siblings) {
      branchMsgIds.add(sib.message_id);
    }
  }

  // Build main_path set if provided
  const mainPathSet = mainPath ? new Set(mainPath) : null;

  // First pass: decide which messages to keep
  const keep = new Set<string>();
  for (const msg of messages) {
    let visible = true;
    if (msg.kind === "tool_use" || msg.kind === "tool_result") {
      /* keep */
    } else if (
      msg.entry_type === "attachment" ||
      msg.entry_type === "system" ||
      msg.role === "system"
    ) {
      if (SKIP_SUBTYPES.has(msg.subtype || "")) visible = false;
    } else if (msg.role === "human" && msg.kind === "message") {
      /* keep */
    } else if (msg.role === "assistant" && msg.kind === "message") {
      /* keep */
    } else {
      visible = false;
    }
    if (visible) keep.add(msg.id);
  }

  // Build re-parent map
  const parentMap = new Map<string, string | null>();
  for (const msg of messages) {
    if (!keep.has(msg.id)) continue;
    let p = msg.parent_id;
    while (p && !keep.has(p)) {
      const parent = msgMap.get(p);
      p = parent ? parent.parent_id : null;
    }
    parentMap.set(msg.id, p);
  }

  // Determine ordering: use main_path order if available, otherwise use message list order
  let orderedIds: string[];
  if (mainPath) {
    const mainOrdered = mainPath.filter((id) => keep.has(id));
    // Also include branch messages that aren't on main_path
    const branchOnlyIds = Array.from(keep).filter(
      (id) => !mainPathSet!.has(id) && keep.has(id)
    );
    orderedIds = [...mainOrdered, ...branchOnlyIds];
  } else {
    orderedIds = Array.from(keep);
  }

  // Track y position per column for stacking
  let currentY = 0;
  let lastMainPathIndex = 0;
  const mainPathY = new Map<string, number>(); // y position for each main_path node
  let users = 0, assistants = 0, tools = 0;

  // Process main_path nodes first
  const mainPathOrdered = mainPath
    ? orderedIds.filter((id) => mainPathSet!.has(id))
    : orderedIds;

  const processedIds = new Set<string>();

  for (let i = 0; i < mainPathOrdered.length; i++) {
    const id = mainPathOrdered[i];
    if (processedIds.has(id)) continue;
    const msg = msgMap.get(id);
    if (!msg) continue;

    const nodeResult = createNode(msg, branchMsgIds.has(id));
    if (!nodeResult) continue;

    const { type, label, height, toolName } = nodeResult;
    const col = columnForNode(type);

    nodes.push({
      id: msg.id,
      type,
      data: {
        label,
        toolName,
        model: msg.model || undefined,
        width: W,
        height,
        isBranch: false,
      },
      position: {
        x: col * COL_WIDTH,
        y: currentY,
      },
    });

    mainPathY.set(id, currentY);
    processedIds.add(id);

    // Count stats
    if (type === "user") users++;
    else if (type === "assistant") assistants++;
    else if (type === "toolUse" || type === "toolResult") tools++;

    // Edge from effective parent
    const effectiveParent = parentMap.get(id);
    if (effectiveParent && mainPathY.has(effectiveParent)) {
      edges.push({
        id: `e-${effectiveParent}-${id}`,
        source: effectiveParent,
        target: id,
        type: "smoothstep",
        style: { strokeWidth: 2 },
      });
    }

    currentY += height + ROW_HEIGHT - 30; // tight spacing
  }

  // Process branch nodes: group by branch point, place at branch parent's y
  let branchCol = 3;
  let branchCount = 0;

  for (let bi = 0; bi < branches.length; bi++) {
    const bp = branches[bi];
    const parentY = mainPathY.get(bp.parent_id);
    if (parentY === undefined) continue;

    for (const sib of bp.siblings) {
      const msg = msgMap.get(sib.message_id);
      if (!msg || processedIds.has(sib.message_id)) continue;

      const nodeResult = createNode(msg, true);
      if (!nodeResult) continue;

      const { type, label, height, toolName } = nodeResult;

      nodes.push({
        id: msg.id,
        type,
        data: {
          label,
          toolName,
          model: msg.model || undefined,
          width: W,
          height,
          isBranch: true,
          branchIndex: bi,
        },
        position: {
          x: branchCol * COL_WIDTH,
          y: parentY,
        },
      });

      processedIds.add(msg.id);
      branchCount++;

      // Edge from branch point parent to branch node
      edges.push({
        id: `e-${bp.parent_id}-${msg.id}`,
        source: bp.parent_id,
        target: msg.id,
        type: "smoothstep",
        style: { stroke: "#f59e0b", strokeDasharray: "5,3", strokeWidth: 1.5 },
      });

      // Follow this branch's chain
      let childId = msg.id;
      let childY = parentY;
      let branchChainCol = branchCol;

      // Walk down children of the branch node
      for (const otherMsg of messages) {
        if (processedIds.has(otherMsg.id) || !keep.has(otherMsg.id)) continue;
        if (otherMsg.parent_id !== childId) continue;

        // Check this message is part of the same branch subtree
        const chainResult = createNode(otherMsg, true);
        if (!chainResult) continue;

        const childCol = columnForNode(chainResult.type);
        // Branch chain nodes can be in their native columns OR stay in branch area
        const nodeCol = Math.max(childCol, branchCol);

        childY += chainResult.height + COMPACT_HEIGHT;

        nodes.push({
          id: otherMsg.id,
          type: chainResult.type,
          data: {
            label: chainResult.label,
            toolName: chainResult.toolName,
            model: otherMsg.model || undefined,
            width: W,
            height: chainResult.height,
            isBranch: true,
            branchIndex: bi,
          },
          position: {
            x: nodeCol * COL_WIDTH,
            y: childY,
          },
        });

        processedIds.add(otherMsg.id);
        branchCount++;

        edges.push({
          id: `e-${childId}-${otherMsg.id}`,
          source: childId,
          target: otherMsg.id,
          type: "smoothstep",
          style: { stroke: "#f59e0b", strokeDasharray: "5,3", strokeWidth: 1.5 },
        });

        childId = otherMsg.id;
      }
    }

    branchCol++;
  }

  return {
    nodes,
    edges,
    stats: { total: nodes.length, users, assistants, tools, branches: branchCount },
  };
}

// ─── Node creation helper ───────────────────────────────────────────

function createNode(
  msg: MessageRecord,
  isBranch: boolean,
): { type: TreeNodeType; label: string; height: number; toolName?: string } | null {
  if (msg.kind === "tool_use") {
    const toolName = msg.tool_name || "Tool";
    const label = msg.content_preview
      ? msg.content_preview.replace(/^Tool:\s*/, "").split("\n")[0]
      : toolName;
    return { type: "toolUse", label, height: 40, toolName };
  }
  if (msg.kind === "tool_result") {
    const label = truncate(msg.content_preview || "done", 60);
    return { type: "toolResult", label, height: 34 };
  }
  if (
    msg.entry_type === "attachment" ||
    msg.entry_type === "system" ||
    msg.role === "system"
  ) {
    const sub = msg.subtype || "";
    let label: string;
    if (sub === "plan_mode") label = "Plan mode";
    else if (sub === "plan_mode_exit") label = "Exit plan";
    else if (sub === "edited_text_file") label = "Edited: " + (msg.content_preview || "").split(":")[0];
    else if (sub === "api_error") label = "API Error";
    else if (sub === "compact_boundary") label = "Compacted";
    else if (sub === "scheduled_task_fire") label = "Scheduled task";
    else label = msg.content_preview ? truncate(msg.content_preview, 35) : sub || "system";
    return { type: "meta", label, height: 28 };
  }
  if (msg.role === "human" && msg.kind === "message") {
    return { type: "user", label: truncate(msg.content_preview, 55), height: 46 };
  }
  if (msg.role === "assistant" && msg.kind === "message") {
    const label = msg.content_preview ? truncate(msg.content_preview, 55) : "(thinking…)";
    return { type: "assistant", label, height: msg.content_preview ? 48 : 30 };
  }
  return null;
}
```

- [ ] **Step 2: Update existing tests for new API**

The `buildTree` signature changed — it now takes an optional second argument `{ mainPath, branches }`. Update existing tests to match:

```typescript
import { describe, it, expect } from "vitest";
import { buildTree, truncate } from "../build-tree";
import type { MessageRecord } from "@/lib/api";

function makeMsg(overrides: Partial<MessageRecord> & { id: string }): MessageRecord {
  return {
    session_id: "s1",
    parent_id: null,
    role: "human",
    kind: "message",
    content_preview: "test",
    timestamp: null,
    is_sidechain: false,
    entry_type: "message",
    subtype: null,
    tool_name: null,
    subagent_id: null,
    model: null,
    metadata: null,
    ...overrides,
  };
}

describe("buildTree", () => {
  it("returns empty result for empty input", () => {
    const { nodes, edges, stats } = buildTree([]);
    expect(nodes).toHaveLength(0);
    expect(edges).toHaveLength(0);
    expect(stats.total).toBe(0);
  });

  it("creates a user node for human messages", () => {
    const msg = makeMsg({ id: "m1", role: "human", kind: "message", content_preview: "Hello" });
    const { nodes, stats } = buildTree([msg]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].type).toBe("user");
    expect(stats.users).toBe(1);
  });

  it("creates an assistant node", () => {
    const msg = makeMsg({ id: "m1", role: "assistant", kind: "message", content_preview: "Hi" });
    const { nodes, stats } = buildTree([msg]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].type).toBe("assistant");
    expect(stats.assistants).toBe(1);
  });

  it("creates an edge from parent to child", () => {
    const parent = makeMsg({ id: "m1", role: "human", kind: "message" });
    const child = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const { edges } = buildTree([parent, child]);
    expect(edges).toHaveLength(1);
    expect(edges[0].source).toBe("m1");
    expect(edges[0].target).toBe("m2");
  });

  it("re-parents across skipped nodes", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    const sys = makeMsg({ id: "m2", parent_id: "m1", role: "system", kind: "message", entry_type: "system", subtype: "mcp_instructions_delta" });
    const assistant = makeMsg({ id: "m3", parent_id: "m2", role: "assistant", kind: "message" });
    const { nodes, edges } = buildTree([user, sys, assistant]);
    expect(nodes).toHaveLength(2);
    expect(edges).toHaveLength(1);
    expect(edges[0].source).toBe("m1");
    expect(edges[0].target).toBe("m3");
  });

  it("creates tool_use nodes", () => {
    const msg = makeMsg({ id: "m1", role: "assistant", kind: "tool_use", tool_name: "Read", content_preview: "Tool: Read\nfile.ts" });
    const { nodes, stats } = buildTree([msg]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].type).toBe("toolUse");
    expect(stats.tools).toBe(1);
  });

  it("skips verbose system subtypes", () => {
    const msg = makeMsg({ id: "m1", role: "system", kind: "message", entry_type: "system", subtype: "skill_listing" });
    const { nodes } = buildTree([msg]);
    expect(nodes).toHaveLength(0);
  });

  it("places user nodes in column 0 and assistant in column 1", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    const asst = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const { nodes } = buildTree([user, asst]);
    const userNode = nodes.find((n) => n.type === "user")!;
    const asstNode = nodes.find((n) => n.type === "assistant")!;
    expect(userNode.position.x).toBe(0);
    expect(asstNode.position.x).toBe(220);
  });

  it("places tool nodes in column 2", () => {
    const tool = makeMsg({ id: "m1", role: "assistant", kind: "tool_use", tool_name: "Read" });
    const { nodes } = buildTree([tool]);
    expect(nodes[0].position.x).toBe(440);
  });

  it("places branch nodes in column 3+", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    const asst = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const branchAsst = makeMsg({ id: "m3", role: "assistant", kind: "message", parent_id: "m1", content_preview: "alt path" });

    const { nodes, stats } = buildTree(
      [user, asst, branchAsst],
      {
        mainPath: ["m1", "m2"],
        branches: [{ parent_id: "m1", siblings: [{ message_id: "m3", label: "alt" }], active_index: 0 }],
      },
    );

    expect(stats.branches).toBeGreaterThan(0);
    const branchNode = nodes.find((n) => n.id === "m3");
    expect(branchNode).toBeDefined();
    expect(branchNode!.position.x).toBeGreaterThanOrEqual(3 * 220);
    expect(branchNode!.data.isBranch).toBe(true);
  });

  it("uses main_path ordering for y positions", () => {
    const m1 = makeMsg({ id: "m1", role: "human", kind: "message" });
    const m2 = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const m3 = makeMsg({ id: "m3", role: "assistant", kind: "tool_use", tool_name: "Read", parent_id: "m2" });

    const { nodes } = buildTree([m1, m2, m3], { mainPath: ["m1", "m2", "m3"], branches: [] });

    const n1 = nodes.find((n) => n.id === "m1")!;
    const n2 = nodes.find((n) => n.id === "m2")!;
    const n3 = nodes.find((n) => n.id === "m3")!;
    expect(n2.position.y).toBeGreaterThan(n1.position.y);
    expect(n3.position.y).toBeGreaterThan(n2.position.y);
  });
});

describe("truncate", () => {
  it("returns empty for empty input", () => {
    expect(truncate("")).toBe("");
  });

  it("truncates at default length", () => {
    expect(truncate("a".repeat(60))).toHaveLength(49);
  });

  it("preserves short text", () => {
    expect(truncate("hello")).toBe("hello");
  });
});
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `npx vitest run src/pages/sessions/graph/__tests__/build-tree.test.ts`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/pages/sessions/graph/build-tree.ts src/pages/sessions/graph/__tests__/build-tree.test.ts
git commit -m "feat: swimlane layout algorithm with branch support"
```

---

### Task 2: Update Node Components — Add Left/Right Handles

**Files:**
- Modify: `src/pages/sessions/graph/nodes.tsx`

Add `Position.Left` and `Position.Right` handles alongside existing Top/Bottom. This enables horizontal cross-column edges from the swimlane layout.

- [ ] **Step 1: Update all node components with left/right handles**

Each node gets 4 handles: Top (target), Bottom (source), Left (target), Right (source). The `id` prop on Handle is required when multiple handles of the same type exist.

```tsx
import type { NodeProps } from "@xyflow/react";
import { Handle, Position } from "@xyflow/react";
import { useTranslation } from "react-i18next";

function toolTone(toolName?: string) {
  switch (toolName) {
    case "Bash":
      return {
        card: "border-orange-400/30 bg-orange-500/10",
        badge: "border-orange-400/20 bg-orange-500/10 text-orange-300",
      };
    case "Read":
    case "Write":
    case "Edit":
      return {
        card: "border-amber-300/30 bg-amber-400/10",
        badge: "border-amber-300/20 bg-amber-400/10 text-amber-200",
      };
    case "Grep":
    case "Glob":
      return {
        card: "border-emerald-400/30 bg-emerald-500/10",
        badge: "border-emerald-400/20 bg-emerald-500/10 text-emerald-300",
      };
    case "Agent":
      return {
        card: "border-fuchsia-400/30 bg-fuchsia-500/10",
        badge: "border-fuchsia-400/20 bg-fuchsia-500/10 text-fuchsia-300",
      };
    case "WebSearch":
      return {
        card: "border-sky-400/30 bg-sky-400/10",
        badge: "border-sky-400/20 bg-sky-400/10 text-sky-300",
      };
    default:
      return {
        card: "border-border/70 bg-card/90",
        badge: "border-border/60 bg-secondary text-muted-foreground",
      };
  }
}

function UserNode({ data }: NodeProps) {
  const d = data as { label: string };
  const { t } = useTranslation();
  return (
    <div className="graph-node rounded-xl border border-primary/30 bg-primary/10 px-[10px] py-[8px] shadow-sm">
      <Handle type="target" position={Position.Top} id="top" style={{ visibility: "hidden" }} />
      <Handle type="target" position={Position.Left} id="left" style={{ visibility: "hidden" }} />
      <div className="graph-node-tag mb-1 text-primary">{t("graph.nodeUser")}</div>
      <div className="graph-node-label">{d.label}</div>
      <Handle type="source" position={Position.Bottom} id="bottom" style={{ visibility: "hidden" }} />
      <Handle type="source" position={Position.Right} id="right" style={{ visibility: "hidden" }} />
    </div>
  );
}

function AssistantNode({ data }: NodeProps) {
  const d = data as { label: string; model?: string; isBranch?: boolean };
  const { t } = useTranslation();
  const branchClass = d.isBranch ? "border-dashed opacity-70" : "";
  return (
    <div className={`graph-node rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-[10px] py-[8px] shadow-sm ${branchClass}`}>
      <Handle type="target" position={Position.Top} id="top" style={{ visibility: "hidden" }} />
      <Handle type="target" position={Position.Left} id="left" style={{ visibility: "hidden" }} />
      <div className="mb-1 flex items-center justify-between">
        <span className="graph-node-tag text-emerald-300">{t("graph.nodeAssistant")}</span>
        {d.model && (
          <span className="rounded-full border border-border/60 bg-card/70 px-1.5 py-0.5 text-[8px] text-muted-foreground">
            {d.model}
          </span>
        )}
      </div>
      <div className="graph-node-label">{d.label}</div>
      <Handle type="source" position={Position.Bottom} id="bottom" style={{ visibility: "hidden" }} />
      <Handle type="source" position={Position.Right} id="right" style={{ visibility: "hidden" }} />
    </div>
  );
}

function ToolUseNode({ data }: NodeProps) {
  const d = data as { label: string; toolName?: string; isBranch?: boolean };
  const tone = toolTone(d.toolName);
  const branchClass = d.isBranch ? "border-dashed opacity-70" : "";
  return (
    <div className={`graph-node rounded-xl border px-[9px] py-[7px] shadow-sm ${tone.card} ${branchClass}`}>
      <Handle type="target" position={Position.Top} id="top" style={{ visibility: "hidden" }} />
      <Handle type="target" position={Position.Left} id="left" style={{ visibility: "hidden" }} />
      <div className="mb-1 flex items-center gap-1.5">
        <span className={`rounded-full border px-1.5 py-0.5 font-mono text-[9px] font-semibold uppercase tracking-[0.06em] ${tone.badge}`}>
          {d.toolName}
        </span>
      </div>
      <div className="graph-node-detail mt-px">{d.label}</div>
      <Handle type="source" position={Position.Bottom} id="bottom" style={{ visibility: "hidden" }} />
      <Handle type="source" position={Position.Right} id="right" style={{ visibility: "hidden" }} />
    </div>
  );
}

function ToolResultNode({ data }: NodeProps) {
  const d = data as { label: string; isBranch?: boolean };
  const { t } = useTranslation();
  const branchClass = d.isBranch ? "border-dashed opacity-70" : "";
  return (
    <div className={`graph-node rounded-xl border border-border/70 bg-secondary/40 px-[9px] py-[7px] shadow-sm ${branchClass}`}>
      <Handle type="target" position={Position.Top} id="top" style={{ visibility: "hidden" }} />
      <Handle type="target" position={Position.Left} id="left" style={{ visibility: "hidden" }} />
      <div className="graph-node-hint mb-1">{t("graph.nodeResult")}</div>
      <div className="graph-node-detail leading-[1.3]">{d.label}</div>
      <Handle type="source" position={Position.Bottom} id="bottom" style={{ visibility: "hidden" }} />
      <Handle type="source" position={Position.Right} id="right" style={{ visibility: "hidden" }} />
    </div>
  );
}

function MetaNode({ data }: NodeProps) {
  const d = data as { label: string };
  return (
    <div className="graph-node rounded-xl border border-dashed border-border bg-transparent px-[8px] py-[6px] italic">
      <Handle type="target" position={Position.Top} id="top" style={{ visibility: "hidden" }} />
      <Handle type="target" position={Position.Left} id="left" style={{ visibility: "hidden" }} />
      <span className="graph-node-detail">{d.label}</span>
      <Handle type="source" position={Position.Bottom} id="bottom" style={{ visibility: "hidden" }} />
      <Handle type="source" position={Position.Right} id="right" style={{ visibility: "hidden" }} />
    </div>
  );
}

export const nodeTypes = {
  user: UserNode,
  assistant: AssistantNode,
  toolUse: ToolUseNode,
  toolResult: ToolResultNode,
  meta: MetaNode,
} as const;
```

- [ ] **Step 2: Commit**

```bash
git add src/pages/sessions/graph/nodes.tsx
git commit -m "feat: add left/right handles and branch styling to graph nodes"
```

---

### Task 3: Update session-graph.tsx — Remove main_path Filter, Pass Branches

**Files:**
- Modify: `src/pages/sessions/session-graph.tsx`

The main integration change. Remove the `main_path` filter so branch messages are included, and pass `mainPath` + `branches` to the new `buildTree`.

- [ ] **Step 1: Update the useMemo block and edge options**

The key changes in `session-graph.tsx`:
1. Remove the `mainSet` filter — pass ALL messages to `buildTree`
2. Pass `mainPath` and `branches` via options
3. Update `defaultEdgeOptions` for `smoothstep` edge type
4. Add branches count badge to stats bar

```tsx
import { useState, useMemo, useCallback } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Controls,
  Background,
  MarkerType,
  Panel,
  useReactFlow,
  useNodesState,
  useEdgesState,
} from "@xyflow/react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { getSessionTranscript } from "@/lib/api";
import { GRAPH_MAX_NODES } from "@/lib/constants";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { buildTree } from "./graph/build-tree";
import { nodeTypes } from "./graph/nodes";
import NodeDetailPanel from "./graph/node-detail-panel";

import "@xyflow/react/dist/style.css";

function GraphCanvas({
  nodes: layoutedNodes,
  edges: layoutedEdges,
  onSelectNode,
}: {
  nodes: ReturnType<typeof buildTree>["nodes"];
  edges: ReturnType<typeof buildTree>["edges"];
  onSelectNode: (id: string) => void;
}) {
  const { fitView } = useReactFlow();
  const { t } = useTranslation();
  const [nodes, , onNodesChange] = useNodesState(layoutedNodes);
  const [edges, , onEdgesChange] = useEdgesState(layoutedEdges);

  const onNodeClick = useCallback((_: React.MouseEvent, node: { id: string }) => {
    onSelectNode(node.id);
  }, [onSelectNode]);

  const handleFitView = useCallback(() => {
    fitView({ padding: 0.15 });
  }, [fitView]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const onInit = useCallback((instance: any) => {
    instance.fitView({ padding: 0.15 });
  }, []);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onNodeClick={onNodeClick}
      onInit={onInit}
      nodeTypes={nodeTypes}
      defaultEdgeOptions={{
        type: "smoothstep",
        markerEnd: {
          type: MarkerType.ArrowClosed,
          width: 12,
          height: 12,
          color: "#8b9bb4",
        },
        style: { stroke: "#8b9bb4", strokeWidth: 2 },
      }}
      fitView
      minZoom={0.05}
      maxZoom={2}
      proOptions={{ hideAttribution: true }}
      style={{ background: "var(--background, #3b414d)" }}
    >
      <Controls
        position="bottom-left"
        style={{
          background: "var(--card, #2f343e)",
          border: "1px solid var(--border, #464b57)",
          borderRadius: 12,
        }}
      />
      <Background color="var(--border, #464b57)" gap={24} size={1} />
      <Panel position="top-right" style={{ padding: 12 }}>
        <Button
          variant="outline"
          size="sm"
          onClick={handleFitView}
          className="h-8 rounded-md px-3 text-[12px]"
        >
          {t("graph.fitView")}
        </Button>
      </Panel>
    </ReactFlow>
  );
}

export default function SessionGraph({ sessionId }: { sessionId: string }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { t } = useTranslation();

  const { data: transcript, isLoading, error } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  const { nodes, edges, stats, truncated } = useMemo(() => {
    if (!transcript || transcript.main_path.length === 0)
      return { nodes: [], edges: [], stats: { total: 0, users: 0, assistants: 0, tools: 0, branches: 0 }, truncated: false };

    let messages = transcript.messages;
    let wasTruncated = false;
    if (messages.length > GRAPH_MAX_NODES) {
      // Keep all main_path messages + as many others as fit
      const mainSet = new Set(transcript.main_path);
      const mainMsgs = messages.filter((m) => mainSet.has(m.id));
      const otherMsgs = messages.filter((m) => !mainSet.has(m.id));
      const remaining = GRAPH_MAX_NODES - mainMsgs.length;
      messages = remaining > 0 ? [...mainMsgs, ...otherMsgs.slice(0, remaining)] : mainMsgs;
      wasTruncated = messages.length < transcript.messages.length;
    }

    const result = buildTree(messages, {
      mainPath: transcript.main_path,
      branches: transcript.branches,
    });
    return { ...result, truncated: wasTruncated };
  }, [transcript]);

  if (isLoading) {
    return (
      <div className="space-y-3 p-4">
        <Skeleton className="h-20 w-full rounded-xl" />
        <Skeleton className="h-[360px] w-full rounded-xl" />
      </div>
    );
  }

  if (error) {
    return (
      <p className="px-4 py-3 text-[14px] text-destructive">
        {t("transcript.error", { message: String(error) })}
      </p>
    );
  }

  if (nodes.length === 0) {
    return (
      <p className="px-4 py-3 text-[14px] text-muted-foreground">
        {t("graph.noData")}
      </p>
    );
  }

  return (
    <div className="flex h-full min-h-[400px] flex-col p-3">
      <div className="mb-2.5 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border bg-secondary px-3 py-2">
        <div className="min-w-0">
          <p className="zed-kicker">{t("graph.title")}</p>
          <p className="mt-0.5 truncate text-[12px] text-muted-foreground">{t("graph.description")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            {t("graph.statsNodes", { count: stats.total })}
          </Badge>
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            {t("graph.statsUsers", { count: stats.users })}
          </Badge>
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            {t("graph.statsAssistants", { count: stats.assistants })}
          </Badge>
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            {t("graph.statsTools", { count: stats.tools })}
          </Badge>
          {stats.branches > 0 && (
            <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-orange-400">
              {t("graph.statsBranches", { count: stats.branches })}
            </Badge>
          )}
        </div>
      </div>
      <div className="relative min-h-0 flex-1 overflow-hidden rounded-xl border border-border bg-card">
        {truncated && (
          <div className="absolute left-1/2 top-3 z-10 -translate-x-1/2 rounded-full border border-border bg-card/95 px-3 py-1.5 text-[12px] text-muted-foreground backdrop-blur-sm">
            {t("graph.truncated", { max: GRAPH_MAX_NODES, total: transcript!.messages.length })}
          </div>
        )}
        <ReactFlowProvider>
          <GraphCanvas nodes={nodes} edges={edges} onSelectNode={setSelectedId} />
        </ReactFlowProvider>
      </div>

      {selectedId && transcript && (
        <NodeDetailPanel
          nodeId={selectedId}
          messages={transcript.messages}
          onClose={() => setSelectedId(null)}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/pages/sessions/session-graph.tsx
git commit -m "feat: wire swimlane layout with full branch data"
```

---

### Task 4: Add i18n Keys

**Files:**
- Modify: `src/i18n/locales/en.json`
- Modify: `src/i18n/locales/zh-CN.json`

- [ ] **Step 1: Add new keys to en.json**

Add after the existing `graph.nodeEmpty` key:

```json
"graph.statsBranches": "{{count}} branches",
"graph.branchPoint": "Branch point",
"graph.branchAlt": "alt",
"graph.branchMain": "main"
```

- [ ] **Step 2: Add new keys to zh-CN.json**

Add the matching Chinese translations in the graph section:

```json
"graph.statsBranches": "{{count}} 个分支",
"graph.branchPoint": "分支点",
"graph.branchAlt": "备选",
"graph.branchMain": "主线"
```

- [ ] **Step 3: Commit**

```bash
git add src/i18n/locales/en.json src/i18n/locales/zh-CN.json
git commit -m "feat: add branch-related i18n keys"
```

---

### Task 5: Verify and Polish

**Files:** All modified files

- [ ] **Step 1: Run frontend build to check for type errors**

Run: `npm run build`
Expected: Build succeeds with no type errors

- [ ] **Step 2: Run tests**

Run: `npx vitest run src/pages/sessions/graph/__tests__/build-tree.test.ts`
Expected: All tests pass

- [ ] **Step 3: Visual verification with `cargo tauri dev`**

Launch the app, open a session with branch points, switch to Graph view, verify:
- Three columns visible (User / Assistant / Tool)
- Branch nodes appear to the right of the main path
- Dashed edges distinguish branches from main path
- Zoom/pan/fitView still works
- Click on node opens detail panel
- Stats bar shows branch count

- [ ] **Step 4: Final commit if any adjustments needed**

```bash
git add -u
git commit -m "fix: polish swimlane graph layout"
```
