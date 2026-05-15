import type { Node as RFNode, Edge } from "@xyflow/react";
import type { MessageRecord, BranchPoint } from "@/lib/api";

// ─── Constants ──────────────────────────────────────────────────────

const COL_WIDTH = 220;
const GAP_TURN = 18;       // gap between turns (between user messages)
const GAP_INLINE = 10;     // gap between elements within a turn
const GAP_STACK = 4;       // gap between tool_use → tool_result
const GAP_BRANCH_CHAIN = 6;
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

export const COL_USER = 0;
export const COL_ASSISTANT = 1;
export const COL_TOOL = 2;

function columnForNode(type: TreeNodeType): number {
  switch (type) {
    case "user": return COL_USER;
    case "assistant": return COL_ASSISTANT;
    case "toolUse":
    case "toolResult": return COL_TOOL;
    case "meta": return COL_ASSISTANT;
  }
}

// ─── Node creation helper ───────────────────────────────────────────

function createNodeFromMsg(
  msg: MessageRecord,
): { type: TreeNodeType; label: string; height: number; toolName?: string } | null {
  if (msg.kind === "tool_use") {
    const toolName = msg.tool_name || "Tool";
    const label = msg.content_preview
      ? msg.content_preview.replace(/^Tool:\s*/, "").split("\n")[0]
      : toolName;
    return { type: "toolUse", label, height: 36, toolName };
  }
  if (msg.kind === "tool_result") {
    const label = truncate(msg.content_preview || "done", 60);
    return { type: "toolResult", label, height: 30 };
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
    return { type: "meta", label, height: 24 };
  }
  if (msg.role === "human" && msg.kind === "message") {
    return { type: "user", label: truncate(msg.content_preview, 55), height: 40 };
  }
  if (msg.role === "assistant" && msg.kind === "message") {
    const label = msg.content_preview ? truncate(msg.content_preview, 55) : "(thinking…)";
    return { type: "assistant", label, height: msg.content_preview ? 42 : 26 };
  }
  return null;
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

  // Process main_path nodes with turn-based compact layout
  let turnY = 0;       // Y of current turn's main row
  let stackY = 0;      // Y for next node in tool stack
  let firstInTurn = true; // whether we haven't placed any non-user node yet in this turn
  const mainPathY = new Map<string, number>();
  const processedIds = new Set<string>();
  let users = 0, assistants = 0, tools = 0;

  const mainPathOrdered = mainPath
    ? mainPath.filter((id) => keep.has(id))
    : Array.from(keep);

  for (const id of mainPathOrdered) {
    if (processedIds.has(id)) continue;
    const msg = msgMap.get(id);
    if (!msg) continue;

    const nodeResult = createNodeFromMsg(msg);
    if (!nodeResult) continue;

    const { type, label, height, toolName } = nodeResult;
    const col = columnForNode(type);
    let y: number;

    if (type === "user") {
      // New turn: advance past previous turn's stacked tools
      if (processedIds.size > 0) {
        turnY = stackY + GAP_TURN;
      }
      y = turnY;
      stackY = y + height + GAP_INLINE;
      firstInTurn = true;
    } else if (type === "assistant") {
      if (firstInTurn) {
        // First assistant in this turn — share Y with the user
        y = turnY;
      } else {
        // Continuation after tools — start a new row
        turnY = stackY;
        y = turnY;
      }
      stackY = y + height + GAP_INLINE;
      firstInTurn = false;
    } else {
      // tool/meta — stack below
      y = stackY;
      stackY += height + GAP_STACK;
    }

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
      position: { x: col * COL_WIDTH, y },
    });

    mainPathY.set(id, y);
    processedIds.add(id);

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
  }

  // Process branch nodes
  let branchCol = 3;
  let branchCount = 0;

  for (let bi = 0; bi < branches.length; bi++) {
    const bp = branches[bi];
    const parentY = mainPathY.get(bp.parent_id);
    if (parentY === undefined) continue;

    for (const sib of bp.siblings) {
      const msg = msgMap.get(sib.message_id);
      if (!msg || processedIds.has(sib.message_id)) continue;

      const nodeResult = createNodeFromMsg(msg);
      if (!nodeResult) continue;

      const { type, label, height, toolName } = nodeResult;
      const col = Math.max(columnForNode(type), branchCol);

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
        position: { x: col * COL_WIDTH, y: parentY },
      });

      processedIds.add(msg.id);
      branchCount++;

      edges.push({
        id: `e-${bp.parent_id}-${msg.id}`,
        source: bp.parent_id,
        target: msg.id,
        type: "smoothstep",
        style: { stroke: "#f59e0b", strokeDasharray: "5,3", strokeWidth: 1.5 },
      });

      // Follow branch chain
      let childId = msg.id;
      let childY = parentY;

      for (const otherMsg of messages) {
        if (processedIds.has(otherMsg.id) || !keep.has(otherMsg.id)) continue;
        if (otherMsg.parent_id !== childId) continue;

        const chainResult = createNodeFromMsg(otherMsg);
        if (!chainResult) continue;

        childY += chainResult.height + GAP_BRANCH_CHAIN;
        const nodeCol = Math.max(columnForNode(chainResult.type), branchCol);

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
          position: { x: nodeCol * COL_WIDTH, y: childY },
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
