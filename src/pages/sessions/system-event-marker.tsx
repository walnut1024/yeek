import React from "react";
import type { MessageRecord } from "@/lib/api";

const SystemEventMarker = React.memo(function SystemEventMarker({
  msg,
}: {
  msg: MessageRecord;
  indent?: number; // kept for compat, ignored
}) {
  const subtype = msg.subtype || "unknown";

  // Compact boundary → dashed divider
  if (subtype === "compact_boundary") {
    return (
      <div className="flex items-center gap-3 py-2">
        <div className="flex-1 border-t border-dashed border-border/30" />
        <span className="text-[11px] text-muted-foreground/40">Compacted</span>
        <div className="flex-1 border-t border-dashed border-border/30" />
      </div>
    );
  }

  // API error → red accent
  if (subtype === "api_error") {
    return (
      <div className="py-0.5">
        <span className="text-[11px] font-medium text-destructive/80">API Error</span>
        {msg.content_preview && (
          <span className="ml-1.5 text-[11px] text-destructive/50">{msg.content_preview}</span>
        )}
      </div>
    );
  }

  // Scheduled task fire
  if (subtype === "scheduled_task_fire") {
    return (
      <div className="py-0.5">
        <span className="text-[11px] font-medium text-green-600/60">Scheduled task</span>
      </div>
    );
  }

  // Hide noisy events: turn_duration, stop_hook_summary, and others with no content
  if (!msg.content_preview) return null;

  // Only show remaining if they have meaningful content
  return (
    <div className="py-0.5">
      <span className="text-[11px] text-muted-foreground/30">{msg.content_preview}</span>
    </div>
  );
});

export default SystemEventMarker;
