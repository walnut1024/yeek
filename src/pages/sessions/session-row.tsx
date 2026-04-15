import React from "react";
import { Badge } from "@/components/ui/badge";
import type { SessionRecord } from "@/lib/api";
import { formatRelativeTime } from "@/lib/formatters";

const SessionRow = React.memo(function SessionRow({
  session,
  isSelected,
  onSelect,
}: {
  session: SessionRecord;
  isSelected: boolean;
  onSelect: () => void;
}) {
  const title = session.title || session.id.slice(0, 12);
  return (
    <button
      onClick={onSelect}
      className={`flex w-full items-start gap-3 px-4 py-2.5 text-left transition-colors ${
        isSelected ? "bg-accent" : "hover:bg-accent/50"
      }`}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-sm font-medium leading-tight">
            {title.length > 80 ? title.slice(0, 80) + "..." : title}
          </span>
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[10px] text-muted-foreground">
          <Badge
            variant="outline"
            className="px-1 py-0 text-[9px] font-normal"
          >
            {session.agent === "claude_code" ? "Claude" : session.agent}
          </Badge>
          {session.pinned && (
            <Badge
              variant="secondary"
              className="px-1 py-0 text-[9px] font-normal"
            >
              pinned
            </Badge>
          )}
          {session.visibility === "archived" && (
            <Badge
              variant="secondary"
              className="px-1 py-0 text-[9px] font-normal"
            >
              archived
            </Badge>
          )}
          {session.visibility === "hidden" && (
            <Badge
              variant="secondary"
              className="px-1 py-0 text-[9px] font-normal"
            >
              hidden
            </Badge>
          )}
          {session.model && (
            <span className="text-muted-foreground/50">{session.model}</span>
          )}
          <span className="text-muted-foreground/40">
            {session.message_count} msg{session.message_count !== 1 ? "s" : ""}
          </span>
          {session.project_path && (
            <span className="text-muted-foreground/40">
              {session.project_path.split("/").pop()}
            </span>
          )}
        </div>
      </div>
      <span className="shrink-0 pt-0.5 text-[10px] text-muted-foreground/40">
        {formatRelativeTime(session.updated_at)}
      </span>
    </button>
  );
});

export default SessionRow;
