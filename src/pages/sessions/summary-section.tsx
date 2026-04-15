import { useState } from "react";
import type { MessageRecord } from "@/lib/api";

export default function SummarySection({
  msg,
}: {
  msg: MessageRecord;
  indent?: number; // kept for compat, ignored
}) {
  const [expanded, setExpanded] = useState(false);
  if (!msg.content_preview) return null;

  return (
    <div className="py-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-muted/20 transition-colors"
      >
        <span className="text-[11px] text-muted-foreground/40">{expanded ? "▾" : "▸"}</span>
        <span className="text-[11px] font-medium text-muted-foreground/50">Conversation Summary</span>
      </button>
      {expanded && (
        <p className="ml-6 mt-0.5 text-[12px] leading-relaxed text-muted-foreground/70 whitespace-pre-wrap">
          {msg.content_preview}
        </p>
      )}
    </div>
  );
}
