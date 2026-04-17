import dagre from "@dagrejs/dagre";
import type { Node, Edge } from "@xyflow/react";

// ─── Color map ─────────────────────────────────────────────────────

const TOOL_COLORS: Record<string, string> = {
  Bash: "#e07a5f", Read: "#f2cc8f", Write: "#f2cc8f", Edit: "#f2cc8f",
  Grep: "#81b29a", Glob: "#81b29a", Agent: "#c77dba", WebSearch: "#7fb8d8",
  Skill: "#b8a9c9", AskUserQuestion: "#6c8ebf", TaskCreate: "#7ec8b8",
  TaskUpdate: "#7ec8b8", TaskOutput: "#7ec8b8", TaskList: "#7ec8b8",
  SendMessage: "#c77dba", CronCreate: "#b8a9c9",
};

export function toolColor(name: string): string {
  return TOOL_COLORS[name] || "#888";
}

// ─── Types ──────────────────────────────────────────────────────────

export interface TreeNodeData {
  label: string;
  toolName?: string | null;
  model?: string | null;
  [key: string]: unknown;
}

interface RawMessage {
  id: string;
  parent_id: string | null;
  role: string;
  kind: string;
  content_preview?: string | null;
  timestamp?: string | null;
  entry_type?: string | null;
  subtype?: string | null;
  tool_name?: string | null;
  subagent_id?: string | null;
  model?: string | null;
  is_sidechain?: boolean;
}

// ─── Layout ────────────────────────────────────────────────────────

function computeLayout(nodes: Array<Node<TreeNodeData> & { width: number; height: number }>, edges: Edge[]): Array<Node<TreeNodeData>> {
  const g = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 70 });
  for (const n of nodes) g.setNode(n.id, { width: n.width, height: n.height });
  for (const e of edges) g.setEdge(e.source, e.target);
  dagre.layout(g);
  return nodes.map((n) => {
    const pos = g.node(n.id);
    return { ...n, position: { x: pos.x - n.width / 2, y: pos.y - n.height / 2 } };
  });
}

// ─── Helpers ───────────────────────────────────────────────────────

function truncate(text: string | null | undefined, len = 48): string {
  if (!text) return "";
  const s = text.replace(/\n/g, " ").trim();
  return s.length > len ? s.slice(0, len) + "\u2026" : s;
}

// ─── Build Tree ────────────────────────────────────────────────────

export function buildTree(messages: RawMessage[]) {
  const W = 200;
  const nodes: Array<Node<TreeNodeData> & { width: number; height: number }> = [];
  const edges: Edge[] = [];
  const msgMap = new Map(messages.map((m) => [m.id, m]));

  // First pass: decide which messages to keep
  const keep = new Set<string>();
  for (const msg of messages) {
    if (msg.kind === "tool_use" || msg.kind === "tool_result") {
      /* keep */
    } else if (msg.entry_type === "attachment" || msg.entry_type === "system" || msg.role === "system") {
      const sub = msg.subtype || "";
      if (sub === "mcp_instructions_delta" || sub === "skill_listing" ||
          sub === "superpowers" || sub === "claude-md" || sub === "context") {
        continue;
      }
    } else if (msg.role === "human" && msg.kind === "message") {
      /* keep */
    } else if (msg.role === "assistant" && msg.kind === "message") {
      /* keep */
    } else {
      continue;
    }
    keep.add(msg.id);
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

  // Stats
  let total = 0, users = 0, assistants = 0, tools = 0;

  // Second pass: create nodes
  for (const msg of messages) {
    if (!keep.has(msg.id)) continue;
    let type: string, label: string, height: number;

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
    } else if (msg.entry_type === "attachment" || msg.entry_type === "system" || msg.role === "system") {
      const sub = msg.subtype || "";
      if (sub === "plan_mode") label = "Plan mode";
      else if (sub === "plan_mode_exit") label = "Exit plan";
      else if (sub === "edited_text_file") label = "Edited: " + (msg.content_preview || "").split(":")[0];
      else if (sub === "api_error") label = "API Error";
      else if (sub === "compact_boundary") label = "Compacted";
      else if (sub === "scheduled_task_fire") label = "Scheduled task";
      else label = msg.content_preview ? truncate(msg.content_preview, 35) : (sub || "system");
      type = "meta";
      height = 28;
    } else if (msg.role === "human" && msg.kind === "message") {
      label = truncate(msg.content_preview, 55);
      type = "user";
      height = 46;
      users++;
    } else if (msg.role === "assistant" && msg.kind === "message") {
      label = msg.content_preview ? truncate(msg.content_preview, 55) : "(thinking\u2026)";
      type = "assistant";
      height = msg.content_preview ? 48 : 30;
      assistants++;
    } else {
      continue;
    }

    total++;
    nodes.push({
      id: msg.id,
      type,
      data: { label, toolName: msg.tool_name, model: msg.model },
      position: { x: 0, y: 0 },
      width: W,
      height,
    });

    const effectiveParent = parentMap.get(msg.id);
    if (effectiveParent) {
      edges.push({
        id: `e-${effectiveParent}-${msg.id}`,
        source: effectiveParent,
        target: msg.id,
        style: { stroke: "#555", strokeWidth: 1.2 },
      });
    }
  }

  return {
    nodes: computeLayout(nodes, edges),
    edges,
    stats: { total, users, assistants, tools },
  };
}
