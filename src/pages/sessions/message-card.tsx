import React from "react";
import type { MessageRecord } from "@/lib/api";
import AttachmentMarker from "./attachment-marker";
import SystemEventMarker from "./system-event-marker";
import SummarySection from "./summary-section";
import ToolResultCard from "./tool-result-card";
import ToolCallCard from "./tool-call-card";

const MessageCard = React.memo(function MessageCard({
  msg,
  sessionId,
}: {
  msg: MessageRecord;
  depth?: number; // kept for compat, ignored
  sessionId: string;
}) {
  // Attachment markers
  if (msg.entry_type === "attachment") {
    return <AttachmentMarker msg={msg} />;
  }

  // System events
  if (msg.entry_type === "system") {
    return <SystemEventMarker msg={msg} />;
  }

  // Summary
  if (msg.entry_type === "summary") {
    return <SummarySection msg={msg} />;
  }

  // Tool result
  if (msg.kind === "tool_result") {
    return <ToolResultCard msg={msg} />;
  }

  // Tool use (assistant with tools)
  if (msg.kind === "tool_use") {
    return <ToolCallCard msg={msg} sessionId={sessionId} />;
  }

  // Human message — card with shadow-border
  if (msg.role === "human") {
    return (
      <div
        className="mt-4 rounded-lg bg-[#fafafa] px-4 py-3"
        style={{ boxShadow: "0px 0px 0px 1px rgba(0,0,0,0.06)" }}
      >
        <div className="flex items-center gap-2 mb-1">
          <span className="text-[12px] font-medium text-foreground">You</span>
          {msg.timestamp && (
            <span className="text-[11px] text-muted-foreground/40">
              {new Date(msg.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
            </span>
          )}
        </div>
        <p className="text-[14px] leading-[1.6] text-foreground whitespace-pre-wrap">
          {msg.content_preview}
        </p>
      </div>
    );
  }

  // Assistant text — plain with label header
  return (
    <div className="mt-3">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-[12px] font-medium text-muted-foreground">Assistant</span>
        {msg.model && (
          <span className="font-mono text-[11px] text-muted-foreground/40">{msg.model}</span>
        )}
        {msg.timestamp && (
          <span className="text-[11px] text-muted-foreground/30">
            {new Date(msg.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </span>
        )}
      </div>
      <p className="text-[14px] leading-[1.6] text-foreground whitespace-pre-wrap">
        {msg.content_preview}
      </p>
    </div>
  );
});

export default MessageCard;
