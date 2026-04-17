import React from "react";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { Separator } from "@/components/ui/separator";
import {
  FileIcon,
  SystemIcon,
  ErrorIcon,
  PlanIcon,
  CompactIcon,
  ScheduleIcon,
} from "@/components/icons";

const MetaLine = React.memo(function MetaLine({
  msg,
}: {
  msg: MessageRecord;
}) {
  const { t } = useTranslation();
  if (msg.entry_type === "attachment") {
    const subtype = msg.subtype || "";

    if (subtype === "date_change") {
      return (
        <div className="flex items-center gap-3 py-2">
          <Separator className="flex-1 bg-border" />
          <span className="text-[14px] text-muted-foreground">
            {msg.content_preview}
          </span>
          <Separator className="flex-1 bg-border" />
        </div>
      );
    }

    // Plan mode → accent blue
    if (
      subtype === "plan_mode" ||
      subtype === "plan_mode_exit" ||
      subtype === "plan_mode_reentry"
    ) {
      const label =
        subtype === "plan_mode"
          ? t("meta.planMode")
          : subtype === "plan_mode_exit"
            ? t("meta.exitPlan")
            : t("meta.reenterPlan");
      return (
        <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2">
          <div className="flex items-center gap-1.5">
            <PlanIcon className="text-primary" />
            <span className="text-[14px] font-medium text-primary">
              {label}
            </span>
          </div>
          {msg.content_preview && (
            <span className="mt-1 block text-[14px] text-muted-foreground">
              {msg.content_preview}
            </span>
          )}
        </div>
      );
    }

    // File edit
    if (subtype === "file" || subtype === "edited_text_file") {
      const filename = msg.content_preview.split(":")[0];
      const label = subtype === "edited_text_file" ? t("meta.edited") : t("meta.file");
      return (
        <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2">
          <div className="flex items-center gap-1.5">
            <FileIcon className="text-muted-foreground" />
            <span className="text-[14px] text-muted-foreground">{label}</span>
            <span className="font-mono text-[14px] text-muted-foreground">
              {filename}
            </span>
          </div>
        </div>
      );
    }

    if (!msg.content_preview) return null;
    return (
      <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2">
        <div className="flex items-center gap-1.5">
          <FileIcon className="text-muted-foreground" />
          <span className="text-[14px] text-muted-foreground">
            {msg.content_preview}
          </span>
        </div>
      </div>
    );
  }

  if (msg.entry_type === "system") {
    const subtype = msg.subtype || "";

    // Compact boundary → dashed border
    if (subtype === "compact_boundary") {
      return (
        <div className="flex items-center gap-3 py-2">
          <div className="flex-1 h-px border-t border-dashed border-border" />
          <div className="flex items-center gap-1.5">
            <CompactIcon className="text-muted-foreground" />
            <span className="text-[14px] text-muted-foreground">{t("meta.compacted")}</span>
          </div>
          <div className="flex-1 h-px border-t border-dashed border-border" />
        </div>
      );
    }

    // API error → destructive
    if (subtype === "api_error") {
      return (
        <div className="-mx-1 rounded-md border border-destructive/40 bg-[var(--editor)] px-2.5 py-2">
          <div className="flex items-center gap-1.5">
            <ErrorIcon className="text-destructive" />
            <span className="text-[14px] font-medium text-destructive">
              {t("meta.apiError")}
            </span>
          </div>
          {msg.content_preview && (
            <span className="mt-1 block text-[14px] text-destructive/70">
              {msg.content_preview}
            </span>
          )}
        </div>
      );
    }

    // Scheduled task
    if (subtype === "scheduled_task_fire") {
      return (
        <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2">
          <div className="flex items-center gap-1.5">
            <ScheduleIcon className="text-muted-foreground" />
            <span className="text-[14px] font-medium text-muted-foreground">
              {t("meta.scheduledTask")}
            </span>
          </div>
        </div>
      );
    }

    if (!msg.content_preview) return null;
    return (
      <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2">
        <div className="flex items-center gap-1.5">
          <SystemIcon className="text-muted-foreground" />
          <span className="text-[14px] text-muted-foreground">
            {msg.content_preview}
          </span>
        </div>
      </div>
    );
  }

  return null;
});

export default MetaLine;
