import { useMemo, useCallback, useState, useLayoutEffect } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { getSessionTranscript } from "@/lib/api";
import { GRAPH_MAX_NODES } from "@/lib/constants";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Maximize2, Minimize2, Clock, User, Bot, GitBranch, Wrench } from "lucide-react";
import { buildMapTree, TOOL_COLLAPSE_THRESHOLD } from "./build-session-tree";
import type { MapNode, Turn } from "./build-session-tree";
import NodeDetailPanel from "./node-detail-panel";

// ── Tool color map ──────────────────────────────────────────────

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

// ── Node card ───────────────────────────────────────────────────

function NodeCard({
  node,
  onClick,
}: {
  node: MapNode;
  onClick: (id: string) => void;
}) {
  const style = NODE_STYLES[node.type];
  const toolColor =
    node.type === "toolUse" && node.toolName
      ? TOOL_COLORS[node.toolName]
      : undefined;

  const bg = toolColor
    ? `color-mix(in srgb, ${toolColor} 14%, var(--card))`
    : style.bg;
  const border = toolColor
    ? `color-mix(in srgb, ${toolColor} 35%, var(--border))`
    : style.border;
  const text = toolColor || style.text;

  return (
    <button
      data-node-card="true"
      data-node-id={node.id}
      type="button"
      onClick={() => onClick(node.id)}
      className={`group relative z-20 w-full rounded-lg border px-2.5 py-1.5 text-left transition-colors ${
        node.isMainPath ? "" : "border-dashed opacity-60"
      } ${node.type === "thinking" ? "border-dashed opacity-70" : ""}`}
      style={{
        background: bg,
        borderColor: border,
        color: text,
        borderRadius: node.type === "user" || node.type === "assistant" ? 10 : 6,
      }}
    >
      <p
        className="line-clamp-2 text-[12px] leading-snug"
        style={{ fontWeight: node.type === "user" || node.type === "assistant" ? 500 : 400 }}
      >
        {node.label}
      </p>
      <div className="mt-0.5 flex items-center justify-between gap-2">
        {node.toolName && (
          <span className="text-[9px] opacity-70">{node.toolName}</span>
        )}
        {node.model && (
          <span className="text-[9px] text-muted-foreground opacity-60">{node.model}</span>
        )}
        {node.type === "subagent" && (
          <span className="text-[9px]" style={{ color: "#c77dba" }}>agent</span>
        )}
      </div>
    </button>
  );
}

const NODE_STYLES: Record<string, { bg: string; border: string; text: string }> = {
  user: { bg: "color-mix(in srgb, #2563eb 9%, var(--card))", border: "rgba(37,99,235,0.2)", text: "#1e40af" },
  assistant: { bg: "color-mix(in srgb, #059669 9%, var(--card))", border: "rgba(5,150,105,0.2)", text: "#065f46" },
  toolUse: { bg: "color-mix(in srgb, #f2cc8f 14%, var(--card))", border: "rgba(242,204,143,0.3)", text: "#8b6914" },
  toolResult: { bg: "color-mix(in srgb, #78716c 7%, var(--card))", border: "rgba(120,113,108,0.15)", text: "#78716c" },
  subagent: { bg: "color-mix(in srgb, #c77dba 10%, var(--card))", border: "rgba(199,125,186,0.25)", text: "#8b3a7d" },
  meta: { bg: "var(--card)", border: "rgba(120,113,108,0.2)", text: "#78716c" },
  thinking: { bg: "color-mix(in srgb, #8b5cf6 6%, var(--card))", border: "rgba(139,92,246,0.2)", text: "#7c3aed" },
};

// ── Turn edges (SVG overlay) ─────────────────────────────────────

const EDGE_COLOR = "rgba(28,28,28,0.28)";
const SWIMLANE_GRID_COLUMNS =
  "60px minmax(240px, 1fr) minmax(280px, 1.18fr) minmax(240px, 1fr) minmax(240px, 1fr)";
const SWIMLANE_MIN_WIDTH = 1060;
const SWIMLANE_GRID_STYLE = {
  gridTemplateColumns: SWIMLANE_GRID_COLUMNS,
  minWidth: SWIMLANE_MIN_WIDTH,
  width: "max(100%, 1060px)",
} as const;

interface CardRect {
  left: number;
  right: number;
  top: number;
  bottom: number;
  cx: number;
  cy: number;
}

interface EdgePath {
  d: string;
  key: string;
}

function getCardRects(
  cell: Element,
  rowRect: DOMRect,
): Map<string, CardRect> {
  const cards = cell.querySelectorAll("[data-node-card='true']");
  const rects = new Map<string, CardRect>();
  Array.from(cards).forEach((card) => {
    const r = card.getBoundingClientRect();
    const id = card.getAttribute("data-node-id");
    if (!id) return;
    rects.set(id, {
      left: r.left - rowRect.left,
      right: r.right - rowRect.left,
      top: r.top - rowRect.top,
      bottom: r.bottom - rowRect.top,
      cx: (r.left + r.right) / 2 - rowRect.left,
      cy: (r.top + r.bottom) / 2 - rowRect.top,
    });
  });
  return rects;
}

function laneEdge(s: CardRect, t: CardRect): string {
  const sx = s.right;
  const sy = s.cy;
  const tx = t.left;
  const ty = t.cy;
  if (Math.abs(sy - ty) < 2) return `M${sx},${sy} H${tx}`;
  const dx = Math.max((tx - sx) * 0.45, 56);
  return `M${sx},${sy} C${sx + dx},${sy} ${tx - dx},${ty} ${tx},${ty}`;
}

function TurnEdges({
  turn,
  row,
  expanded,
}: {
  turn: Turn;
  row: HTMLDivElement | null;
  expanded: boolean;
}) {
  const [paths, setPaths] = useState<EdgePath[]>([]);
  const [size, setSize] = useState({ width: 0, height: 0 });

  const measure = useCallback(() => {
    if (!row) return;

    const rowRect = row.getBoundingClientRect();
    if (rowRect.width === 0 || rowRect.height === 0) return;

    // Measure cards per column
    const userCell = row.querySelector('[data-col="user"]');
    const asstCell = row.querySelector('[data-col="asst"]');
    const subCell = row.querySelector('[data-col="sub"]');
    const toolsCell = row.querySelector('[data-col="tools"]');

    const rects = new Map<string, CardRect>();
    [userCell, asstCell, subCell, toolsCell].forEach((cell) => {
      if (!cell) return;
      getCardRects(cell, rowRect).forEach((rect, id) => rects.set(id, rect));
    });

    const measureNodes = (nodes: MapNode[]) =>
      nodes
      .map((node) => {
        const rect = rects.get(node.id);
        return rect ? { node, rect } : null;
      })
      .filter((item): item is { node: MapNode; rect: CardRect } => item !== null)
      .sort((a, b) => a.node.order - b.node.order);

    const users = measureNodes(turn.user);
    const assistants = measureNodes(turn.assistants);
    const subagents = measureNodes(turn.subagents);
    const tools = measureNodes(turn.tools);
    const nextPaths: EdgePath[] = [];

    const closestTo = (
      nodes: { node: MapNode; rect: CardRect }[],
      y: number,
    ) =>
      nodes.reduce<{ node: MapNode; rect: CardRect } | undefined>(
        (best, node) =>
          !best || Math.abs(node.rect.cy - y) < Math.abs(best.rect.cy - y)
            ? node
            : best,
        undefined,
      );

    const connectOne = (
      source: { node: MapNode; rect: CardRect } | undefined,
      target: { node: MapNode; rect: CardRect } | undefined,
      prefix: string,
    ) => {
      if (!source || !target || target.rect.left <= source.rect.right) return;
      nextPaths.push({
        key: `${prefix}-${source.node.id}-${target.node.id}`,
        d: laneEdge(source.rect, target.rect),
      });
    };

    const firstAssistant = assistants[0];
    const firstSubagent = subagents[0];
    const firstTool = tools[0];
    const userSource = firstAssistant ? closestTo(users, firstAssistant.rect.cy) : users.at(-1);
    const toolSource = firstTool
      ? closestTo(subagents.length > 0 ? subagents : assistants, firstTool.rect.cy)
      : undefined;

    connectOne(userSource, firstAssistant, "user-assistant");
    connectOne(
      firstSubagent ? closestTo(assistants, firstSubagent.rect.cy) : undefined,
      firstSubagent,
      "assistant-subagent",
    );
    connectOne(toolSource, firstTool, "source-tool");

    setSize({ width: rowRect.width, height: rowRect.height });
    setPaths(nextPaths);
  }, [row]);

  useLayoutEffect(() => {
    if (!row) return;

    let frame = requestAnimationFrame(measure);

    const scheduleMeasure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };

    const resizeObserver = new ResizeObserver(scheduleMeasure);
    resizeObserver.observe(row);
    row
      .querySelectorAll("[data-node-card='true']")
      .forEach((card) => resizeObserver.observe(card));

    window.addEventListener("resize", scheduleMeasure);
    document.fonts?.ready.then(scheduleMeasure).catch(() => undefined);

    return () => {
      cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      window.removeEventListener("resize", scheduleMeasure);
    };
  }, [measure, row, expanded, turn.id, turn.user.length, turn.assistants.length, turn.subagents.length, turn.tools.length]);

  return (
    <svg
      className="pointer-events-none absolute left-0 top-0 z-10"
      style={{
        width: `${size.width}px`,
        height: `${size.height}px`,
        overflow: "visible",
      }}
      viewBox={`0 0 ${size.width} ${size.height}`}
    >
      {paths.map((path) => (
        <path
          key={path.key}
          d={path.d}
          fill="none"
          stroke={EDGE_COLOR}
          strokeLinecap="round"
          strokeWidth="2"
        />
      ))}
    </svg>
  );
}

// ── Turn row ────────────────────────────────────────────────────

function formatTime(ts: string | null): string {
  if (!ts) return "--:--";
  const d = new Date(ts);
  if (isNaN(d.getTime())) return "--:--";
  return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", hour12: false });
}

function TurnRow({
  turn,
  prevTimestamp,
  expanded,
  onToggle,
  onNodeClick,
}: {
  turn: Turn;
  prevTimestamp: string | null;
  expanded: boolean;
  onToggle: () => void;
  onNodeClick: (id: string) => void;
}) {
  const timeStr = formatTime(turn.timestamp);
  const showDitto =
    prevTimestamp &&
    turn.timestamp &&
    Math.abs(new Date(turn.timestamp).getTime() - new Date(prevTimestamp).getTime()) < 60_000;

  const visibleTools = expanded
    ? turn.tools
    : turn.tools.slice(0, TOOL_COLLAPSE_THRESHOLD);
  const hiddenCount = turn.tools.length - TOOL_COLLAPSE_THRESHOLD;

  const [rowEl, setRowEl] = useState<HTMLDivElement | null>(null);

  return (
    <div
      ref={setRowEl}
      className="relative grid border-t border-border/40"
      style={SWIMLANE_GRID_STYLE}
    >
      <TurnEdges turn={turn} row={rowEl} expanded={expanded} />

      {/* Time */}
      <div className="flex items-start justify-center pt-2 text-[11px] text-muted-foreground">
        <span className="font-mono">{showDitto ? '"' : timeStr}</span>
      </div>

      {/* User */}
      <div
        data-col="user"
        className="space-y-2 border-l border-border/30 p-2"
        style={{ background: "rgba(37,99,235,0.02)" }}
      >
        {turn.user.map((n) => (
          <NodeCard key={n.id} node={n} onClick={onNodeClick} />
        ))}
      </div>

      {/* Assistant */}
      <div
        data-col="asst"
        className="space-y-2 border-l border-border/30 p-2"
        style={{ background: "rgba(5,150,105,0.02)" }}
      >
        {turn.assistants.map((n) => (
          <NodeCard key={n.id} node={n} onClick={onNodeClick} />
        ))}
      </div>

      {/* Sub-agent */}
      <div
        data-col="sub"
        className="space-y-2 border-l border-border/30 p-2"
        style={{ background: "rgba(199,125,186,0.02)" }}
      >
        {turn.subagents.map((n) => (
          <NodeCard key={n.id} node={n} onClick={onNodeClick} />
        ))}
      </div>

      {/* Tools */}
      <div
        data-col="tools"
        className="space-y-2 border-l border-border/30 p-2"
        style={{ background: "rgba(242,204,143,0.02)" }}
      >
        {visibleTools.map((n) => (
          <NodeCard key={n.id} node={n} onClick={onNodeClick} />
        ))}
        {!expanded && hiddenCount > 0 && (
          <button
            type="button"
            onClick={onToggle}
            className="collapse-btn w-full rounded-md border border-dashed border-border/50 px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-secondary"
          >
            ▸ {hiddenCount} more tools
          </button>
        )}
        {expanded && turn.tools.length > TOOL_COLLAPSE_THRESHOLD && (
          <button
            type="button"
            onClick={onToggle}
            className="collapse-btn w-full rounded-md border border-dashed border-border/50 px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-secondary"
          >
            ▴ Collapse
          </button>
        )}
      </div>
    </div>
  );
}

// ── Time gap indicator ──────────────────────────────────────────

function TimeGap({ prev, next }: { prev: string; next: string }) {
  const diffMin = Math.round(
    (new Date(next).getTime() - new Date(prev).getTime()) / 60_000,
  );
  if (diffMin <= 5) return null;
  const label =
    diffMin < 60
      ? `${diffMin} min`
      : diffMin < 1440
        ? `${Math.floor(diffMin / 60)}h ${diffMin % 60}m`
        : `${Math.floor(diffMin / 1440)}d`;
  return (
    <div className="flex items-center gap-3 border-t border-border/20 py-1">
      <div className="h-px flex-1 bg-border/20" />
      <span className="text-[10px] text-muted-foreground/60">{label} gap</span>
      <div className="h-px flex-1 bg-border/20" />
    </div>
  );
}

// ── Main component ──────────────────────────────────────────────

export default function SessionRelationshipGraph({
  sessionId,
  fullscreen,
  onFullscreen,
}: {
  sessionId: string;
  fullscreen?: boolean;
  onFullscreen?: (value: boolean) => void;
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expandedTurns, setExpandedTurns] = useState<Set<string>>(new Set());
  const { t } = useTranslation();

  const { data: transcript, isLoading, error } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  const { turns, stats } = useMemo(() => {
    if (!transcript || transcript.messages.length === 0) {
      return { turns: [], stats: { users: 0, assistants: 0, tools: 0, subagents: 0 } };
    }

    let messages = transcript.messages;
    if (messages.length > GRAPH_MAX_NODES) {
      const mainSet = new Set(transcript.main_path);
      const mainMsgs = messages.filter((m) => mainSet.has(m.id));
      const otherMsgs = messages.filter((m) => !mainSet.has(m.id));
      const remaining = GRAPH_MAX_NODES - mainMsgs.length;
      messages =
        remaining > 0
          ? [...mainMsgs, ...otherMsgs.slice(0, remaining)]
          : mainMsgs;
    }

    return buildMapTree(messages, transcript.main_path);
  }, [transcript]);

  const handleNodeClick = useCallback((id: string) => {
    setSelectedId(id);
  }, []);

  const toggleTurn = useCallback((turnId: string) => {
    setExpandedTurns((prev) => {
      const next = new Set(prev);
      if (next.has(turnId)) next.delete(turnId);
      else next.add(turnId);
      return next;
    });
  }, []);

  if (isLoading) {
    return (
      <div className="flex h-full min-h-[400px] items-center justify-center p-4">
        <p className="text-[14px] text-muted-foreground">
          {t("graph.loading")}
        </p>
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

  if (turns.length === 0) {
    return (
      <p className="px-4 py-3 text-[14px] text-muted-foreground">
        {t("graph.noData")}
      </p>
    );
  }

  return (
    <div className={`flex h-full flex-col p-3 ${fullscreen ? "min-h-0" : "min-h-[400px]"}`}>
      {/* Stats bar */}
      <div className="mb-2.5 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border bg-secondary px-3 py-2">
        <div className="min-w-0">
          <p className="zed-kicker">{t("graph.relationshipsTitle")}</p>
          <p className="mt-0.5 truncate text-[12px] text-muted-foreground">
            {t("graph.mapDescription")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-blue-500">
            {t("graph.statsUsers", { count: stats.users })}
          </Badge>
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-emerald-600">
            {t("graph.statsAssistants", { count: stats.assistants })}
          </Badge>
          <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            {t("graph.statsTools", { count: stats.tools })}
          </Badge>
          {stats.subagents > 0 && (
            <Badge variant="outline" className="bg-card px-1.5 py-0.5 font-mono text-[10px] text-fuchsia-600">
              {t("graph.statsSubagents", { count: stats.subagents })}
            </Badge>
          )}
          {onFullscreen && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFullscreen(!fullscreen)}
              className="ml-1 h-7 rounded-md px-2.5 text-[12px]"
            >
              {fullscreen ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
              {fullscreen ? t("graph.exitFullscreen") : t("graph.fullscreen")}
            </Button>
          )}
        </div>
      </div>

      <div
        className={`grid min-h-0 flex-1 gap-2 ${
          selectedId && transcript ? "lg:grid-cols-[minmax(0,1fr)_clamp(400px,38vw,1200px)]" : ""
        }`}
      >
        {/* Swimlane container */}
        <div className="relative min-h-0 overflow-hidden rounded-xl border border-border bg-card">
          <div className="h-full overflow-auto">
            {/* Header row */}
            <div
              className="sticky top-0 z-20 grid border-b border-border bg-background/95"
              style={SWIMLANE_GRID_STYLE}
            >
              <div className="flex items-center justify-center px-1 py-1.5 text-[10px] font-medium text-muted-foreground">
                <Clock size={12} className="mr-1" />Time
              </div>
              <div className="flex items-center gap-1 border-l border-border/30 px-2 py-1.5 text-[10px] font-medium text-blue-500">
                <User size={12} /> User
              </div>
              <div className="flex items-center gap-1 border-l border-border/30 px-2 py-1.5 text-[10px] font-medium text-emerald-600">
                <Bot size={12} /> Assistant
              </div>
              <div className="flex items-center gap-1 border-l border-border/30 px-2 py-1.5 text-[10px] font-medium text-fuchsia-600">
                <GitBranch size={12} /> Sub-agent
              </div>
              <div className="flex items-center gap-1 border-l border-border/30 px-2 py-1.5 text-[10px] font-medium text-amber-600">
                <Wrench size={12} /> Tools
              </div>
            </div>

            {/* Scrollable content */}
            <div>
              {turns.map((turn, i) => {
                const prevTs = i > 0 ? turns[i - 1].timestamp : null;
                return (
                  <div key={turn.id}>
                    {prevTs && turn.timestamp && (
                      <TimeGap prev={prevTs} next={turn.timestamp} />
                    )}
                    <TurnRow
                      turn={turn}
                      prevTimestamp={prevTs}
                      expanded={expandedTurns.has(turn.id)}
                      onToggle={() => toggleTurn(turn.id)}
                      onNodeClick={handleNodeClick}
                    />
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {selectedId && transcript && (
          <NodeDetailPanel
            nodeId={selectedId}
            messages={transcript.messages}
            onClose={() => setSelectedId(null)}
            className="min-w-0 overflow-auto"
          />
        )}
      </div>
    </div>
  );
}
