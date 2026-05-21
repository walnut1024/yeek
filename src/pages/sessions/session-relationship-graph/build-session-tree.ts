import type { MessageRecord } from "@/lib/api";

// ── Node types ──────────────────────────────────────────────────

export interface MapNode {
  id: string;
  label: string;
  type: "user" | "assistant" | "toolUse" | "toolResult" | "subagent" | "meta" | "thinking";
  toolName?: string;
  model?: string;
  isMainPath: boolean;
  order: number;
}

export interface Turn {
  id: string;
  timestamp: string | null;
  user: MapNode[];
  assistants: MapNode[];
  subagents: MapNode[];
  tools: MapNode[];
}

export interface BuildMapTreeResult {
  turns: Turn[];
  stats: { users: number; assistants: number; tools: number; subagents: number };
}

// ── Constants ───────────────────────────────────────────────────

export const TOOL_COLLAPSE_THRESHOLD = 3;

// ── Helpers ─────────────────────────────────────────────────────

export function truncate(text: string, len = 48): string {
  if (!text) return "";
  const s = text.replace(/\n/g, " ").trim();
  return s.length > len ? s.slice(0, len) + "…" : s;
}

// ── Classification ──────────────────────────────────────────────

function classifyMessage(
  msg: MessageRecord,
): { type: MapNode["type"]; label: string } | null {
  if (msg.kind === "tool_use") {
    const toolName = msg.tool_name || "Tool";
    if (toolName === "Agent") {
      const desc = msg.content_preview
        ? msg.content_preview.replace(/^Tool:\s*/, "").split("\n")[0]
        : "sub-agent";
      return { type: "subagent", label: truncate(desc, 55) };
    }
    const label = msg.content_preview
      ? msg.content_preview.replace(/^Tool:\s*/, "").split("\n")[0]
      : toolName;
    return { type: "toolUse", label: truncate(label, 55) };
  }
  if (msg.kind === "tool_result") {
    return {
      type: "toolResult",
      label: truncate(msg.content_preview || "done", 60),
    };
  }
  if (
    msg.entry_type === "attachment" ||
    msg.entry_type === "system" ||
    msg.role === "system"
  ) {
    const sub = msg.subtype || "";
    const skip = new Set([
      "mcp_instructions_delta",
      "skill_listing",
      "superpowers",
      "claude-md",
      "context",
    ]);
    if (skip.has(sub)) return null;
    let label: string;
    if (sub === "plan_mode") label = "Plan mode";
    else if (sub === "plan_mode_exit") label = "Exit plan";
    else if (sub === "edited_text_file")
      label = "Edited: " + (msg.content_preview || "").split(":")[0];
    else if (sub === "api_error") label = "API Error";
    else if (sub === "compact_boundary") label = "Compacted";
    else
      label = msg.content_preview
        ? truncate(msg.content_preview, 35)
        : sub || "system";
    return { type: "meta", label };
  }
  if (msg.role === "human" && msg.kind === "message") {
    return { type: "user", label: truncate(msg.content_preview, 55) };
  }
  if (msg.role === "assistant" && msg.kind === "message") {
    if (msg.entry_type === "reasoning") {
      return { type: "thinking", label: truncate(msg.content_preview || "", 55) || "(thinking…)" };
    }
    const label = msg.content_preview
      ? truncate(msg.content_preview, 55)
      : "(thinking…)";
    return { type: "assistant", label };
  }
  return null;
}

function toMapNode(
  msg: MessageRecord,
  info: { type: MapNode["type"]; label: string },
  mainPathSet: Set<string>,
  order: number,
): MapNode {
  return {
    id: msg.id,
    label: info.label,
    type: info.type,
    toolName:
      msg.kind === "tool_use" && msg.tool_name !== "Agent"
        ? msg.tool_name || undefined
        : undefined,
    model:
      msg.role === "assistant" && msg.kind === "message"
        ? msg.model || undefined
        : undefined,
    isMainPath: mainPathSet.has(msg.id),
    order,
  };
}

// ── Turn building ───────────────────────────────────────────────

export function buildMapTree(
  messages: MessageRecord[],
  mainPath: string[],
): BuildMapTreeResult {
  if (messages.length === 0) {
    return { turns: [], stats: { users: 0, assistants: 0, tools: 0, subagents: 0 } };
  }

  const mainPathSet = new Set(mainPath);

  // Classify all messages, preserving order
  interface Classified {
    msg: MessageRecord;
    info: { type: MapNode["type"]; label: string };
    order: number;
  }
  const classified: Classified[] = [];
  messages.forEach((msg, order) => {
    const info = classifyMessage(msg);
    if (info) classified.push({ msg, info, order });
  });

  if (classified.length === 0) {
    return { turns: [], stats: { users: 0, assistants: 0, tools: 0, subagents: 0 } };
  }

  // Split into turns: new turn starts at each human/user message
  interface RawTurn {
    entries: Classified[];
  }
  const rawTurns: RawTurn[] = [];
  let current: RawTurn = { entries: [] };

  for (const entry of classified) {
    if (entry.info.type === "user") {
      if (current.entries.length > 0) {
        rawTurns.push(current);
      }
      current = { entries: [entry] };
    } else {
      current.entries.push(entry);
    }
  }
  if (current.entries.length > 0) {
    rawTurns.push(current);
  }

  // Build Turn objects
  const turns: Turn[] = [];
  for (const raw of rawTurns) {
    const userEntries = raw.entries.filter((e) => e.info.type === "user");
    const asstEntries = raw.entries.filter((e) => e.info.type === "assistant");
    const subagentEntries = raw.entries.filter((e) => e.info.type === "subagent");
    const toolEntries = raw.entries.filter(
      (e) =>
        e.info.type === "toolUse" ||
        e.info.type === "toolResult" ||
        e.info.type === "meta",
    );

    // Turn ID: first user message ID, or "prelude" / "turn-{firstId}"
    let turnId: string;
    if (userEntries.length > 0) {
      turnId = userEntries[0].msg.id;
    } else if (raw.entries.length > 0) {
      turnId = raw.entries[0].msg.id;
    } else {
      turnId = "prelude";
    }

    // Timestamp from first user message
    const timestamp = userEntries.length > 0 ? userEntries[0].msg.timestamp : null;

    turns.push({
      id: turnId,
      timestamp,
      user: userEntries.map((e) => toMapNode(e.msg, e.info, mainPathSet, e.order)),
      assistants: asstEntries.map((e) => toMapNode(e.msg, e.info, mainPathSet, e.order)),
      subagents: subagentEntries.map((e) => toMapNode(e.msg, e.info, mainPathSet, e.order)),
      tools: toolEntries.map((e) => toMapNode(e.msg, e.info, mainPathSet, e.order)),
    });
  }

  // Stats
  let users = 0, assistants = 0, tools = 0, subagents = 0;
  for (const turn of turns) {
    users += turn.user.length;
    assistants += turn.assistants.length;
    tools += turn.tools.length;
    subagents += turn.subagents.length;
  }

  return { turns, stats: { users, assistants, tools, subagents } };
}
