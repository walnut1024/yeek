import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { toolColor } from "./build-tree";

interface NodeDetailPanelProps {
  nodeId: string;
  messages: MessageRecord[];
  onClose: () => void;
}

export default function NodeDetailPanel({
  nodeId,
  messages,
  onClose,
}: NodeDetailPanelProps) {
  const { t } = useTranslation();
  const msg = messages.find((m) => m.id === nodeId);
  if (!msg) return null;

  const roleLabel =
    msg.role === "human"
      ? "User"
      : msg.kind === "tool_use"
        ? `Tool: ${msg.tool_name || "unknown"}`
        : msg.kind === "tool_result"
          ? "Tool Result"
          : "Assistant";

  return (
    <div
      style={{
        position: "absolute",
        right: 0,
        top: 0,
        bottom: 0,
        width: 340,
        background: "var(--card, #2f343e)",
        borderLeft: "1px solid var(--border, #464b57)",
        padding: 16,
        overflowY: "auto",
        zIndex: 10,
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 10,
        }}
      >
        <span
          style={{
            fontSize: 10,
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: 1,
            color: "var(--muted-foreground, #a9afbc)",
          }}
        >
          {roleLabel}
        </span>
        <button
          onClick={onClose}
          style={{
            background: "none",
            border: "none",
            color: "var(--muted-foreground, #a9afbc)",
            cursor: "pointer",
            fontSize: 18,
            lineHeight: 1,
          }}
        >
          ×
        </button>
      </div>
      {msg.model && (
        <div
          style={{
            fontSize: 9,
            color: "var(--muted-foreground, #a9afbc)",
            fontFamily: "var(--font-data, monospace)",
            marginBottom: 6,
          }}
        >
          {msg.model}
        </div>
      )}
      {msg.timestamp && (
        <div
          style={{
            fontSize: 9,
            color: "var(--muted-foreground, #a9afbc)",
            marginBottom: 8,
          }}
        >
          {new Date(msg.timestamp).toLocaleString()}
        </div>
      )}
      <div
        style={{
          fontSize: 12,
          color: "var(--foreground, #dce0e5)",
          lineHeight: 1.55,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {msg.content_preview || "(empty)"}
      </div>
      {msg.tool_name && (
        <div
          style={{
            marginTop: 10,
            padding: "6px 8px",
            background: "var(--editor, #282c33)",
            borderRadius: 4,
          }}
        >
          <span
            style={{
              fontSize: 10,
              fontFamily: "var(--font-data, monospace)",
              color: toolColor(msg.tool_name),
            }}
          >
            {msg.tool_name}
          </span>
        </div>
      )}
    </div>
  );
}
