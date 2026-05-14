import React from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { SessionRecord } from "@/lib/api";
import { formatRelativeTime } from "@/lib/formatters";
import { Check, MessageSquareText, FolderTree } from "lucide-react";
import { TITLE_TRUNCATE_LEN } from "@/lib/constants";

function formatAgentLabel(agent: string): string {
  if (agent === "claude_code") return "Claude Code";
  if (agent === "claude_code_subagent") return "Claude Code";
  if (agent === "codex") return "Codex";
  return agent;
}

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
  const projectLabel = session.project_path
    ? session.project_path.split("/").filter(Boolean).pop()
    : session.agent === "claude_code_subagent"
      ? t("sessionRow.subagent")
      : t("sessionRow.noProject");

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
      className={`zed-list-row flex w-full items-start gap-3 rounded-xl border px-3 py-3 text-left transition-all [animation:fadeSlideIn_300ms_ease-out] ${
        isSelected
          ? "border-border bg-accent shadow-[0_0_0_1px_rgba(28,28,28,0.06)]"
          : "border-transparent hover:border-border hover:bg-element-hover"
      }`}
    >
      {manageMode && (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={checked ? t("sessionRow.deselectAria") : t("sessionRow.selectAria")}
          onClick={(e) => { e.stopPropagation(); onCheck?.(); }}
          className={`mt-1 flex size-5 shrink-0 items-center justify-center rounded-md border-2 transition ${
            checked
              ? "border-primary bg-primary"
              : "border-muted-foreground/40 hover:border-primary"
          }`}
        >
          {checked && (
            <Check size={16} color="white" />
          )}
        </Button>
      )}
      <div className={`mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-full border ${isSelected ? "border-primary/20 bg-primary/10" : "border-border bg-secondary"}`}>
        <MessageSquareText size={15} className={isSelected ? "text-primary" : "text-muted-foreground"} />
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[11px] uppercase tracking-[0.06em] text-muted-foreground">
              {formatAgentLabel(session.agent)}
            </p>
            <span className="mt-1 block truncate text-[14px] font-medium leading-[1.35] text-foreground">
              {title.length > TITLE_TRUNCATE_LEN ? `${title.slice(0, TITLE_TRUNCATE_LEN)}...` : title}
            </span>
          </div>
          <span className="shrink-0 rounded-full border border-border bg-secondary px-2 py-1 font-mono text-[11px] text-muted-foreground">
            {formatRelativeTime(session.updated_at)}
          </span>
        </div>

        <div className="mt-2 flex flex-wrap items-center gap-1.5 text-[12px] text-muted-foreground">
          <Badge
            variant="outline"
            className="bg-secondary px-2 py-0.5 text-[11px] text-primary"
          >
            {session.model || t("sessionRow.noModel")}
          </Badge>
          <span className="zed-chip inline-flex items-center gap-1">
            <MessageSquareText size={12} />
            {t("sessionRow.msgCount", { count: session.message_count })}
          </span>
          <span className="zed-chip inline-flex items-center gap-1">
            <FolderTree size={12} />
            {projectLabel}
          </span>
        </div>
      </div>
    </div>
  );
});

export default SessionRow;
