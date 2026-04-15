import React, { useState } from "react";
import type { MessageRecord } from "@/lib/api";
import SubagentExpansion from "./subagent-expansion";

const ToolCallCard = React.memo(function ToolCallCard({
  msg,
  sessionId,
}: {
  msg: MessageRecord;
  sessionId: string;
  indent?: number; // kept for compat, ignored
}) {
  const [expanded, setExpanded] = useState(false);
  const toolName = msg.tool_name || "Tool";
  const isAgent = toolName === "Agent";

  // Try to extract tool input from metadata
  let toolInput: string | null = null;
  try {
    if (msg.metadata) {
      const meta = JSON.parse(msg.metadata);
      if (meta.tool_inputs && meta.tool_inputs.length > 0) {
        toolInput = meta.tool_inputs[0];
      }
    }
  } catch {}

  // Parse agent description from tool input
  let agentDescription: string | null = null;
  let agentType: string | null = null;
  if (isAgent && toolInput) {
    try {
      const input = JSON.parse(toolInput);
      agentDescription = input.description || input.prompt?.slice(0, 120) || null;
      agentType = input.subagent_type || null;
    } catch {}
  }

  // Extract target from content_preview
  const previewLines = msg.content_preview.split("\n");
  const toolHeader = previewLines[0]?.replace(/^Tool:\s*/, "") || "";
  const toolBody = previewLines.slice(1).join("\n").trim();

  const target = isAgent && agentDescription ? agentDescription : toolHeader;

  // Short content (≤5 lines) → show directly, no collapse
  const bodyLines = toolBody ? toolBody.split("\n").length : 0;
  const isShortBody = !isAgent && toolBody && bodyLines <= 5;

  if (isShortBody) {
    return (
      <div className="mt-1.5">
        <div className="flex items-center gap-2 px-2 py-0.5">
          <span className="font-mono text-[13px] font-medium text-foreground/90">{toolName}</span>
          <span className="truncate font-mono text-[13px] text-muted-foreground/60">{target}</span>
        </div>
        <div
          className="ml-5 mt-0.5 rounded-lg p-3"
          style={{ boxShadow: "0px 0px 0px 1px rgba(0,0,0,0.06)" }}
        >
          <pre className="whitespace-pre-wrap font-mono text-[13px] leading-relaxed text-muted-foreground">
            {toolBody}
          </pre>
        </div>
      </div>
    );
  }

  // Agent tools always collapsible (subagent content can be large)
  return (
    <div className="mt-1.5">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-muted/20 transition-colors"
      >
        <span className="text-[12px] text-muted-foreground/40">{expanded ? "▾" : "▸"}</span>
        <span className="font-mono text-[13px] font-medium text-foreground/90">{toolName}</span>
        {isAgent && agentType && (
          <span className="font-mono text-[11px] text-muted-foreground/50">{agentType}</span>
        )}
        <span className="truncate font-mono text-[13px] text-muted-foreground/60">
          {target}
        </span>
      </button>

      {expanded && !isAgent && toolBody && (
        <div
          className="ml-5 mt-1 rounded-lg p-3"
          style={{ boxShadow: "0px 0px 0px 1px rgba(0,0,0,0.06)" }}
        >
          <pre className="whitespace-pre-wrap font-mono text-[13px] leading-relaxed text-muted-foreground">
            {toolBody}
          </pre>
        </div>
      )}

      {expanded && isAgent && msg.subagent_id && (
        <SubagentExpansion
          sessionId={sessionId}
          subagentId={msg.subagent_id}
          agentType={agentType}
          description={agentDescription}
        />
      )}
    </div>
  );
});

export default ToolCallCard;
