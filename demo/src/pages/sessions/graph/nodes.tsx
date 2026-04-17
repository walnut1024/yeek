import type { NodeProps } from "@xyflow/react";
import { toolColor, type TreeNodeData } from "./build-tree";

const W = 200;

function UserNode({ data }: NodeProps<Node<TreeNodeData>>) {
  return (
    <div
      style={{
        width: W,
        padding: "7px 10px",
        background: "var(--card, #2f343e)",
        borderRadius: 6,
        border: "1px solid var(--ring, #47679e)",
      }}
    >
      <div
        style={{
          fontSize: 9,
          fontWeight: 600,
          color: "var(--primary, #74ade8)",
          marginBottom: 2,
          textTransform: "uppercase",
          letterSpacing: 0.8,
        }}
      >
        User
      </div>
      <div
        style={{
          fontSize: 11,
          color: "var(--foreground, #dce0e5)",
          lineHeight: 1.35,
        }}
      >
        {data.label}
      </div>
    </div>
  );
}

function AssistantNode({ data }: NodeProps<Node<TreeNodeData>>) {
  return (
    <div
      style={{
        width: W,
        padding: "7px 10px",
        background: "var(--card, #2f343e)",
        borderRadius: 6,
        border: "1px solid #5a9e7a",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 2,
        }}
      >
        <span
          style={{
            fontSize: 9,
            fontWeight: 600,
            color: "#81b29a",
            textTransform: "uppercase",
            letterSpacing: 0.8,
          }}
        >
          Assistant
        </span>
        {data.model && (
          <span
            style={{
              fontSize: 8,
              color: "var(--muted-foreground, #a9afbc)",
              fontFamily: "var(--font-data, monospace)",
            }}
          >
            {data.model}
          </span>
        )}
      </div>
      <div
        style={{
          fontSize: 11,
          color: "var(--foreground, #dce0e5)",
          lineHeight: 1.35,
        }}
      >
        {data.label}
      </div>
    </div>
  );
}

function ToolUseNode({ data }: NodeProps<Node<TreeNodeData>>) {
  const c = toolColor(data.toolName || "");
  return (
    <div
      style={{
        width: W,
        padding: "5px 9px",
        background: "var(--card, #2f343e)",
        borderRadius: 4,
        border: `1px solid ${c}50`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
        <span
          style={{
            fontSize: 10,
            fontWeight: 600,
            color: c,
            fontFamily: "var(--font-data, monospace)",
          }}
        >
          {data.toolName}
        </span>
      </div>
      <div
        style={{
          fontSize: 10,
          color: "var(--muted-foreground, #a9afbc)",
          marginTop: 1,
          fontFamily: "var(--font-data, monospace)",
        }}
      >
        {data.label}
      </div>
    </div>
  );
}

function ToolResultNode({ data }: NodeProps<Node<TreeNodeData>>) {
  return (
    <div
      style={{
        width: W,
        padding: "4px 9px",
        background: "var(--editor, #282c33)",
        borderRadius: 4,
        border: "1px solid var(--border, #464b57)",
      }}
    >
      <div
        style={{
          fontSize: 9,
          color: "var(--muted-foreground, #a9afbc)",
          fontFamily: "var(--font-data, monospace)",
        }}
      >
        result
      </div>
      <div
        style={{
          fontSize: 10,
          color: "var(--muted-foreground, #a9afbc)",
          lineHeight: 1.3,
        }}
      >
        {data.label}
      </div>
    </div>
  );
}

function MetaNode({ data }: NodeProps<Node<TreeNodeData>>) {
  return (
    <div
      style={{
        maxWidth: W,
        padding: "4px 8px",
        background: "transparent",
        borderRadius: 4,
        border: "1px dashed var(--border, #464b57)",
        fontStyle: "italic",
      }}
    >
      <span
        style={{
          fontSize: 10,
          color: "var(--muted-foreground, #a9afbc)",
        }}
      >
        {data.label}
      </span>
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
