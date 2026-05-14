import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { FileText, ChevronRight } from "lucide-react";

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
    <section data-ai-region="sessions-summary-message" className="rounded-xl border border-border bg-[var(--editor)] px-3 py-3">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setExpanded(!expanded)}
        className="flex h-auto w-full items-center justify-start gap-2 rounded-lg px-2 py-1.5 hover:bg-element-hover"
      >
        <ChevronRight size={16} className={`text-muted-foreground transition-transform ${expanded ? "rotate-90" : ""}`} />
        <span className="inline-flex size-6 items-center justify-center rounded-full border border-border bg-card">
          <FileText size={14} className="text-muted-foreground" />
        </span>
        <span className="text-[14px] font-medium text-muted-foreground">
          {t("summary.label")}
        </span>
      </Button>
      {expanded && (
        <p className="mt-1 text-[14px] leading-[1.50] text-muted-foreground whitespace-pre-wrap">
          {msg.content_preview}
        </p>
      )}
    </section>
  );
}
