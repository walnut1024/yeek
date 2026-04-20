import type { NodeProps } from "@xyflow/react";
import { Handle, Position } from "@xyflow/react";
import { useTranslation } from "react-i18next";
import { toolColor } from "./build-tree";

function UserNode({ data }: NodeProps) {
  const d = data as { label: string };
  const { t } = useTranslation();
  return (
    <div className="graph-node" style={{ padding: "7px 10px", borderColor: "var(--ring, #47679e)", borderRadius: 6 }}>
      <Handle type="target" position={Position.Top} />
      <div className="graph-node-tag mb-0.5" style={{ color: "var(--primary, #74ade8)" }}>
        {t("graph.nodeUser")}
      </div>
      <div className="graph-node-label">
        {d.label}
      </div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

function AssistantNode({ data }: NodeProps) {
  const d = data as { label: string; model?: string };
  const { t } = useTranslation();
  return (
    <div className="graph-node" style={{ padding: "7px 10px", border: "1px solid #5a9e7a", borderRadius: 6 }}>
      <Handle type="target" position={Position.Top} />
      <div className="flex items-center justify-between mb-0.5">
        <span className="graph-node-tag" style={{ color: "#81b29a" }}>
          {t("graph.nodeAssistant")}
        </span>
        {d.model && (
          <span className="graph-node-hint text-[8px]">
            {d.model}
          </span>
        )}
      </div>
      <div className="graph-node-label">
        {d.label}
      </div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

function ToolUseNode({ data }: NodeProps) {
  const d = data as { label: string; toolName?: string };
  const c = toolColor(d.toolName || "");
  return (
    <div className="graph-node" style={{ padding: "5px 9px", borderRadius: 4, border: `1px solid ${c}50` }}>
      <Handle type="target" position={Position.Top} />
      <div className="flex items-center gap-1.5">
        <span className="graph-node-detail font-semibold" style={{ color: c }}>
          {d.toolName}
        </span>
      </div>
      <div className="graph-node-detail mt-px">
        {d.label}
      </div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

function ToolResultNode({ data }: NodeProps) {
  const d = data as { label: string };
  const { t } = useTranslation();
  return (
    <div className="graph-node" style={{ padding: "4px 9px", borderRadius: 4, background: "var(--editor, #282c33)" }}>
      <Handle type="target" position={Position.Top} />
      <div className="graph-node-hint">
        {t("graph.nodeResult")}
      </div>
      <div className="graph-node-detail leading-[1.3]">
        {d.label}
      </div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

function MetaNode({ data }: NodeProps) {
  const d = data as { label: string };
  return (
    <div className="graph-node" style={{ padding: "4px 8px", borderRadius: 4, background: "transparent", border: "1px dashed var(--border, #464b57)", fontStyle: "italic" }}>
      <Handle type="target" position={Position.Top} />
      <span className="graph-node-detail">
        {d.label}
      </span>
      <Handle type="source" position={Position.Bottom} />
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
