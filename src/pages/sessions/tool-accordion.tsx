import React, { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import i18n from "@/i18n";
import type { MessageRecord } from "@/lib/api";
import SubagentExpansion from "./subagent-expansion";
import { ToolIcon, getToolIcon, ChevronIcon } from "@/components/icons";

interface ToolPair {
  call: MessageRecord;
  result?: MessageRecord;
}

const ToolAccordion = React.memo(function ToolAccordion({
  tools,
  sessionId,
}: {
  tools: ToolPair[];
  sessionId: string;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [expandedTool, setExpandedTool] = useState<number | null>(null);

  // Expand multi-tool entries
  const flatPairs = useMemo(() => {
    const result: ToolPair[] = [];
    for (const pair of tools) {
      const names = (pair.call.tool_name || "Tool").split(",");
      if (names.length <= 1) {
        result.push(pair);
        continue;
      }

      // Parse all tool inputs from metadata
      let allInputs: string[] = [];
      if (pair.call.metadata) {
        try {
          const meta = JSON.parse(pair.call.metadata);
          allInputs = meta.tool_inputs || [];
        } catch {
          void 0;
        }
      }

      for (let i = 0; i < names.length; i++) {
        const name = names[i].trim();
        const virtualCall: MessageRecord = {
          ...pair.call,
          tool_name: name,
          metadata:
            allInputs[i]
              ? JSON.stringify({ tool_inputs: [allInputs[i]] })
              : null,
          content_preview: `Tool: ${name}`,
        };
        result.push({
          call: virtualCall,
          // Attach the combined result only to the last tool
          result: i === names.length - 1 ? pair.result : undefined,
        });
      }
    }
    return result;
  }, [tools]);

  const toolNames = flatPairs.map((t) => t.call.tool_name || "Tool");
  const uniqueNames = [...new Set(toolNames)];
  const summary =
    uniqueNames.length === 1
      ? uniqueNames[0]
      : `${uniqueNames.length} tools: ${uniqueNames.join(", ")}`;

  return (
    <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-1.5 py-1">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-left transition-colors hover:bg-accent/40"
      >
        <ChevronIcon expanded={expanded} className="text-muted-foreground/50" />
        <ToolIcon className="text-muted-foreground" />
        <span className="text-[12px] font-medium text-muted-foreground">
          {t("tools.callCount", { count: flatPairs.length })} · {summary}
        </span>
      </button>

      {expanded && (
        <div className="relative ml-[7px] mt-0.5 border-l border-border/40 pl-3.5">
          {flatPairs.map((pair, i) => (
            <ToolItem
              key={`${pair.call.id}-${i}`}
              pair={pair}
              index={i}
              expandedTool={expandedTool}
              setExpandedTool={setExpandedTool}
              sessionId={sessionId}
            />
          ))}
        </div>
      )}
    </div>
  );
});

function ToolItem({
  pair,
  index,
  expandedTool,
  setExpandedTool,
  sessionId,
}: {
  pair: ToolPair;
  index: number;
  expandedTool: number | null;
  setExpandedTool: (i: number | null) => void;
  sessionId: string;
}) {
  const { t } = useTranslation();
  const { call, result } = pair;
  const isOpen = expandedTool === index;
  const toolName = call.tool_name || "Tool";
  const isAgent = toolName === "Agent";

  // Parse tool inputs from metadata
  let toolInputObj: Record<string, unknown> | null = null;
  let agentDescription: string | null = null;
  let agentType: string | null = null;
  if (call.metadata) {
    try {
      const meta = JSON.parse(call.metadata);
      const raw = meta.tool_inputs?.[0];
      if (raw) {
        toolInputObj = typeof raw === "string" ? JSON.parse(raw) : raw;
        if (isAgent) {
          agentDescription = (toolInputObj as Record<string, string>).description || (toolInputObj as Record<string, string>).prompt?.slice(0, 120) || null;
          agentType = (toolInputObj as Record<string, string>).subagent_type || null;
        }
      }
    } catch {
      void 0;
    }
  }

  const target = isAgent && agentDescription
    ? agentDescription
    : toolInputObj
      ? summarizeToolInput(toolName, toolInputObj)
      : call.content_preview.replace(/^Tool:\s*/, "").split("\n")[0] || "";

  const toolBody = toolInputObj
    ? JSON.stringify(toolInputObj, null, 2)
    : call.content_preview.split("\n").slice(1).join("\n").trim();

  const ToolSpecificIcon = getToolIcon(toolName);

  return (
    <div className="group/item relative mt-0.5 first:mt-0">
      <button
        type="button"
        onClick={() => setExpandedTool(isOpen ? null : index)}
        className="flex w-full items-center gap-1.5 rounded-md border border-transparent bg-secondary/10 px-1.5 py-0.5 text-left transition-colors hover:border-border/50 hover:bg-accent/30"
      >
        <ChevronIcon expanded={isOpen} className="text-muted-foreground/30" />
        {ToolSpecificIcon ? (
          <ToolSpecificIcon className="text-primary/80" />
        ) : (
          <ToolIcon className="text-primary/80" />
        )}
        <span className="font-mono text-[13px] font-medium text-primary/90">
          {toolName}
        </span>
        {isAgent && agentType && (
          <span className="font-mono text-[12px] text-muted-foreground/60">
            {agentType}
          </span>
        )}
        <span className="truncate font-mono text-[13px] text-muted-foreground/80">
          {target}
        </span>
        <span className="ml-auto shrink-0 font-mono text-[11px] text-muted-foreground/40">
          uuid:{call.id}
        </span>
      </button>

      {isOpen && (
        <div className="relative ml-[7px] mt-1 border-l border-border/40 pl-3.5">
          {!isAgent && toolBody && (
            <PayloadCard label={t("tools.input")} content={toolBody} />
          )}

          {result && result.content_preview && (
            <PayloadCard label={t("tools.response")} content={result.content_preview} />
          )}

          {isAgent && call.subagent_id && (
            <SubagentExpansion
              sessionId={sessionId}
              subagentId={call.subagent_id}
              agentType={agentType}
              description={agentDescription}
            />
          )}
        </div>
      )}
    </div>
  );
}

function PayloadCard({ label, content }: { label: string; content: string }) {
  return (
    <div className="my-1.5 overflow-hidden rounded-md border border-border/60 bg-secondary/30">
      <div className="flex items-center border-b border-border/40 bg-secondary/50 px-2 py-0.5">
        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
          {label}
        </span>
      </div>
      <div className="max-h-[400px] overflow-y-auto custom-scrollbar p-2">
        <pre className="whitespace-pre-wrap font-mono text-[13px] leading-[1.6] text-muted-foreground">
          {content}
        </pre>
      </div>
    </div>
  );
}

export default ToolAccordion;

/** Build a one-line summary of a tool's input arguments. */
function summarizeToolInput(toolName: string, input: Record<string, unknown>): string {
  const get = (key: string): string | undefined => {
    const v = input[key];
    return typeof v === "string" ? v : undefined;
  };
  const first = (...keys: string[]): string | undefined => {
    for (const k of keys) { const v = get(k); if (v) return v; }
    return undefined;
  };

  switch (toolName) {
    case "Read":
    case "Write":
    case "Edit":
      return first("file_path", "path") || "";
    case "Bash":
      return first("command") || "";
    case "Grep":
    case "Glob": {
      const pat = first("pattern") || "";
      const dir = first("path") || "";
      return dir ? `${pat} ${i18n.t("tools.in")} ${dir}` : pat;
    }
    case "WebSearch":
    case "WebFetch":
      return first("query", "url") || "";
    case "Agent":
      return first("description", "prompt") || "";
    default: {
      // Show the first string-valued field
      for (const v of Object.values(input)) {
        if (typeof v === "string" && v.length > 0) return v.slice(0, 120);
      }
      return "";
    }
  }
}
