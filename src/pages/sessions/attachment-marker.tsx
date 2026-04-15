import React from "react";
import type { MessageRecord } from "@/lib/api";

const AttachmentMarker = React.memo(function AttachmentMarker({
  msg,
}: {
  msg: MessageRecord;
  indent?: number; // kept for compat, ignored
}) {
  const subtype = msg.subtype || "unknown";

  // Date change → centered divider
  if (subtype === "date_change") {
    return (
      <div className="flex items-center gap-3 py-2">
        <div className="flex-1 border-t border-border/30" />
        <span className="text-[11px] text-muted-foreground/50">{msg.content_preview}</span>
        <div className="flex-1 border-t border-border/30" />
      </div>
    );
  }

  // Plan mode → subtle inline text
  if (subtype === "plan_mode" || subtype === "plan_mode_exit" || subtype === "plan_mode_reentry") {
    const label = subtype === "plan_mode" ? "Plan mode" : subtype === "plan_mode_exit" ? "Exit plan" : "Re-enter plan";
    return (
      <div className="py-0.5">
        <span className="text-[11px] font-medium text-blue-500/80">{label}</span>
        {msg.content_preview && (
          <span className="ml-1.5 text-[11px] text-muted-foreground/50">— {msg.content_preview}</span>
        )}
      </div>
    );
  }

  // File edit → mono filename, no badge
  if (subtype === "file" || subtype === "edited_text_file") {
    const filename = msg.content_preview.split(":")[0];
    const label = subtype === "edited_text_file" ? "Edited" : "File";
    return (
      <div className="py-0.5">
        <span className="text-[11px] text-muted-foreground/40">{label}</span>{" "}
        <span className="font-mono text-[12px] text-muted-foreground/60">{filename}</span>
      </div>
    );
  }

  // Task reminder
  if (subtype === "task_reminder") {
    return (
      <div className="py-0.5">
        <span className="text-[11px] font-medium text-amber-600/70">Reminder</span>
        {msg.content_preview && (
          <span className="ml-1.5 text-[11px] text-muted-foreground/50">{msg.content_preview}</span>
        )}
      </div>
    );
  }

  // Hook success / async_hook_response — hidden unless has content
  if (subtype === "hook_success" || subtype === "async_hook_response" || subtype === "hook_additional_context") {
    if (!msg.content_preview) return null;
    return (
      <div className="py-0.5">
        <span className="text-[11px] text-muted-foreground/30">{msg.content_preview}</span>
      </div>
    );
  }

  // Generic attachment — hidden if no content
  if (!msg.content_preview) return null;
  return (
    <div className="py-0.5">
      <span className="text-[11px] text-muted-foreground/30">{msg.content_preview}</span>
    </div>
  );
});

export default AttachmentMarker;
