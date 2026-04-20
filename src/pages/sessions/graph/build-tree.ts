import type { Node as RFNode, Edge } from "@xyflow/react";
import dagre from "@dagrejs/dagre";
import type { MessageRecord } from "@/lib/api";

// ─── Tool color map ─────────────────────────────────────────────────

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
  return s.length > len ? s.slice(0, len) + "\u2026" : s;
}

// ─── Node data types ────────────────────────────────────────────────

export type TreeNodeType = "user" | "assistant" | "toolUse" | "toolResult" | "meta";

export interface TreeNodeData {
  label: string;
  toolName?: string;
  model?: string;
  [key: string]: unknown;
}

type GraphNode = RFNode<TreeNodeData>;

// ─── Subtypes to skip (verbose system attachments) ──────────────────

const SKIP_SUBTYPES = new Set([
  "mcp_instructions_delta",
  "skill_listing",
  "superpowers",
  "claude-md",
  "context",
]);

// ─── Dagre layout ───────────────────────────────────────────────────

function computeLayout(
  nodes: GraphNode[],
  edges: Edge[]
): GraphNode[] {
  const g = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 70 });
  for (const n of nodes)
    g.setNode(n.id, { width: n.data.width as number, height: n.data.height as number });
  for (const e of edges) g.setEdge(e.source, e.target);
  dagre.layout(g);
  return nodes.map((n) => {
    const pos = g.node(n.id);
    return {
      ...n,
      position: {
        x: pos.x - (n.data.width as number) / 2,
        y: pos.y - (n.data.height as number) / 2,
      },
    };
  });
}

// ─── buildTree ──────────────────────────────────────────────────────

const W = 200;

export interface BuildTreeResult {
  nodes: GraphNode[];
  edges: Edge[];
  stats: { total: number; users: number; assistants: number; tools: number };
}

export function buildTree(messages: MessageRecord[]): BuildTreeResult {
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
      if (SKIP_SUBTYPES.has(msg.subtype || "")) {
        visible = false;
      }
    } else if (
      msg.role === "human" &&
      msg.kind === "message"
    ) {
      /* keep */
    } else if (
      msg.role === "assistant" &&
      msg.kind === "message"
    ) {
      /* keep */
    } else {
      visible = false;
    }
    if (visible) keep.add(msg.id);
  }

  // Build re-parent map: for skipped nodes, find nearest visible ancestor
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

  // Second pass: create nodes and edges
  let users = 0;
  let assistants = 0;
  let tools = 0;

  for (const msg of messages) {
    if (!keep.has(msg.id)) continue;

    let type: TreeNodeType;
    let label: string;
    let height: number;

    if (msg.kind === "tool_use") {
      const toolName = msg.tool_name || "Tool";
      label = msg.content_preview
        ? msg.content_preview.replace(/^Tool:\s*/, "").split("\n")[0]
        : toolName;
      type = "toolUse";
      height = 40;
      tools++;
    } else if (msg.kind === "tool_result") {
      label = truncate(msg.content_preview || "done", 60);
      type = "toolResult";
      height = 34;
    } else if (
      msg.entry_type === "attachment" ||
      msg.entry_type === "system" ||
      msg.role === "system"
    ) {
      const sub = msg.subtype || "";
      if (sub === "plan_mode") label = "Plan mode";
      else if (sub === "plan_mode_exit") label = "Exit plan";
      else if (sub === "edited_text_file")
        label = "Edited: " + (msg.content_preview || "").split(":")[0];
      else if (sub === "api_error") label = "API Error";
      else if (sub === "compact_boundary") label = "Compacted";
      else if (sub === "scheduled_task_fire") label = "Scheduled task";
      else
        label = msg.content_preview
          ? truncate(msg.content_preview, 35)
          : sub || "system";
      type = "meta";
      height = 28;
    } else if (msg.role === "human" && msg.kind === "message") {
      label = truncate(msg.content_preview, 55);
      type = "user";
      height = 46;
      users++;
    } else if (msg.role === "assistant" && msg.kind === "message") {
      label = msg.content_preview
        ? truncate(msg.content_preview, 55)
        : "(thinking\u2026)";
      type = "assistant";
      height = msg.content_preview ? 48 : 30;
      assistants++;
    } else {
      continue;
    }

    nodes.push({
      id: msg.id,
      type,
      data: {
        label,
        toolName: msg.tool_name || undefined,
        model: msg.model || undefined,
        width: W,
        height,
      },
      position: { x: 0, y: 0 },
    });

    const effectiveParent = parentMap.get(msg.id);
    if (effectiveParent) {
      edges.push({
        id: `e-${effectiveParent}-${msg.id}`,
        source: effectiveParent,
        target: msg.id,
      });
    }
  }

  return {
    nodes: computeLayout(nodes, edges),
    edges,
    stats: { total: nodes.length, users, assistants, tools },
  };
}
