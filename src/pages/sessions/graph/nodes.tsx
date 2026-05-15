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
        <span className={`rounded-full border px-1.5 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-[0.06em] ${tone.badge}`}>
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
