import React, { useState } from "react";
import type { MessageRecord } from "@/lib/api";

const ToolResultCard = React.memo(function ToolResultCard({
  msg,
}: {
  msg: MessageRecord;
  indent?: number; // kept for compat, ignored
}) {
  const [expanded, setExpanded] = useState(false);
  const preview = msg.content_preview;

  if (!preview) return null;

  // Short content (≤3 lines) → show directly, no collapse
  const lineCount = preview.split("\n").length;
  if (lineCount <= 3) {
    return (
      <div className="ml-5">
        <pre className="whitespace-pre-wrap font-mono text-[13px] leading-relaxed text-muted-foreground/60 px-2 py-0.5">
          {preview}
        </pre>
      </div>
    );
  }

  // Longer content → collapsible
  return (
    <div className="ml-5">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full rounded px-2 py-0.5 text-left hover:bg-muted/20 transition-colors"
      >
        <span className="text-[12px] text-muted-foreground/40">{expanded ? "▾" : "▸"}</span>{" "}
        <span
          className={`font-mono text-[13px] text-muted-foreground/60 ${expanded ? "" : "line-clamp-2"}`}
        >
          {preview}
        </span>
      </button>
    </div>
  );
});

export default ToolResultCard;
