import React from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { SessionRecord } from "@/lib/api";
import { formatRelativeTime } from "@/lib/formatters";
import { Check } from "iconsax-reactjs";
import { TITLE_TRUNCATE_LEN } from "@/lib/constants";

const SessionRow = React.memo(function SessionRow({
  session,
  isSelected,
  onSelect,
  manageMode,
  checked,
  onCheck,
  onContextMenu,
}: {
  session: SessionRecord;
  isSelected: boolean;
  onSelect: () => void;
  manageMode?: boolean;
  checked?: boolean;
  onCheck?: () => void;
  onContextMenu?: (e: React.MouseEvent, sessionId: string) => void;
}) {
  const title = session.title || session.id.slice(0, 12);
  const { t } = useTranslation();
  return (
    <div
      data-ai-item="session-row"
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu?.(e, session.id);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      aria-label={t("sessionRow.openAria", { title })}
      className={`zed-list-row flex w-full items-start gap-2.5 border px-2.5 py-2 text-left transition-colors [animation:fadeSlideIn_300ms_ease-out] ${
        isSelected
          ? "border-border bg-accent"
          : "border-transparent hover:border-border hover:bg-accent/50"
      }`}
    >
      {manageMode && (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={checked ? t("sessionRow.deselectAria") : t("sessionRow.selectAria")}
          onClick={(e) => { e.stopPropagation(); onCheck?.(); }}
          className={`mt-1 flex size-5 shrink-0 items-center justify-center rounded-sm border-2 transition ${
            checked
              ? "border-primary bg-primary"
              : "border-muted-foreground/40 hover:border-primary"
          }`}
        >
          {checked && (
            <Check size={10} color="white" />
          )}
        </Button>
      )}
      {/* <div className="mt-0.5 size-7 shrink-0 rounded-sm border border-border bg-secondary" /> */}

      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[11px] uppercase tracking-[0.06em] text-muted-foreground">
              {session.agent === "claude_code" ? "Claude Code" : session.agent}
            </p>
            <span className="mt-0.5 block truncate text-[14px] leading-[1.3] text-foreground">
              {title.length > TITLE_TRUNCATE_LEN ? `${title.slice(0, TITLE_TRUNCATE_LEN)}...` : title}
            </span>
          </div>
          <span className="shrink-0 rounded-sm border border-border bg-secondary px-1.5 py-0.5 font-mono text-[12px] text-muted-foreground">
            {formatRelativeTime(session.updated_at)}
          </span>
        </div>

        <div className="mt-1.5 flex flex-wrap items-center gap-1 text-[12px] text-muted-foreground">
          <Badge
            variant="outline"
            className="bg-secondary px-1.5 py-0.5 text-[12px] text-primary"
          >
            {session.model || t("sessionRow.noModel")}
          </Badge>
          <span className="zed-chip">
            {t("sessionRow.msgCount", { count: session.message_count })}
          </span>
          <span className="zed-chip">
            {session.project_path
              ? session.project_path.split("/").filter(Boolean).pop()
              : session.agent === "claude_code_subagent" ? t("sessionRow.subagent") : "—"}
          </span>
        </div>
      </div>
    </div>
  );
});

export default SessionRow;
