import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { SummaryIcon } from "@/components/icons";

export default function SummarySection({
  msg,
}: {
  msg: MessageRecord;
  indent?: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation();
  if (!msg.content_preview) return null;

  return (
    <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setExpanded(!expanded)}
        className="flex h-auto w-full items-center justify-start gap-1.5 rounded-md px-1 py-0.5 hover:bg-accent/40"
      >
        <SummaryIcon className="text-muted-foreground" />
        <span className="text-[14px] font-medium text-muted-foreground">
          {t("summary.label")}
        </span>
        <span className="text-[14px] text-muted-foreground">{expanded ? "▾" : "▸"}</span>
      </Button>
      {expanded && (
        <p className="mt-1 text-[14px] leading-[1.50] text-muted-foreground whitespace-pre-wrap">
          {msg.content_preview}
        </p>
      )}
    </div>
  );
}
