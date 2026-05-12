import { useState, useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionPreview, resumeSession } from "@/lib/api";
import { useLocalStorage } from "@/lib/hooks";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Copy, CircleCheck } from "lucide-react";
import { formatTime, formatRelativeTime } from "@/lib/formatters";
import TranscriptView from "./transcript-view";
import SourcesTab from "./sources-tab";
import SessionGraph from "./session-graph";

export default function SessionDetailPane({
  sessionId,
}: {
  sessionId: string;
}) {
  const { t } = useTranslation();
  const [viewMode, setViewMode] = useLocalStorage<"feed" | "graph">(
    "graph-view",
    "feed",
  );
  const [defaultTerminal] = useLocalStorage<string>("default-terminal", "");

  const { data: preview, isLoading: previewLoading } = useQuery({
    queryKey: ["session-preview", sessionId],
    queryFn: () => getSessionPreview(sessionId),
  });

  if (previewLoading || !preview) {
    return (
      <div className="space-y-2 p-3">
        <Skeleton className="h-32 w-full rounded-md" />
        <Skeleton className="h-56 w-full rounded-md" />
      </div>
    );
  }

  const { record } = preview;

  return (
    <ScrollArea className="h-full">
      <div className="space-y-3 p-3">
        {/* Header */}
        <section data-ai-region="sessions-summary" className="surface-card sticky top-0 z-20 bg-card/95 p-3 backdrop-blur-sm">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              {/* Meta pills */}
              <div className="flex flex-wrap items-center gap-1">
                <MetaPill
                  label={t("detail.model")}
                  value={record.model || t("format.notAvailable")}
                />
                <MetaPill
                  label={t("detail.branch")}
                  value={record.git_branch || t("format.notAvailable")}
                />
                <MetaPill label={t("detail.status")} value={record.status} />
                <MetaPill
                  label={t("detail.messages")}
                  value={String(record.message_count)}
                />
                <MetaPill
                  label={t("detail.sources")}
                  value={String(preview.source_count)}
                />
                <MetaPill
                  label={t("detail.started")}
                  value={formatTime(record.started_at)}
                />
                <MetaPill
                  label={t("detail.updated")}
                  value={formatRelativeTime(record.updated_at)}
                />
              </div>
              <div className="mt-2 flex items-center gap-3 text-[12px] font-medium tracking-normal text-muted-foreground">
                <CopyableText label={t("detail.sourceLabel", { path: record.id })} value={record.id} />
                <CopyableText label={t("detail.sourcePath", { path: record.project_path ?? "" })} value={record.project_path ?? ""} />
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              <Button
                size="sm"
                className="h-7 rounded-md px-2.5 text-[13px]"
                onClick={async () => {
                  try {
                    await resumeSession(record.id, record.agent, record.project_path, defaultTerminal || null);
                  } catch (e) {
                    console.error("Failed to resume session:", e);
                  }
                }}
              >
                {t("detail.resume")}
              </Button>
            </div>
          </div>

          <SourcesTab sessionId={sessionId} />
        </section>

        <section data-ai-region="sessions-transcript" className="surface-card overflow-hidden p-1">
          {/* Graph/Feed toggle */}
          <div className="flex items-center gap-1 border-b border-border px-2 py-1">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className={`pill-tab ${viewMode === "feed" ? "pill-tab-active" : "pill-tab-idle"}`}
              onClick={() => setViewMode("feed")}
            >
              {t("graph.viewFeed")}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className={`pill-tab ${viewMode === "graph" ? "pill-tab-active" : "pill-tab-idle"}`}
              onClick={() => setViewMode("graph")}
            >
              {t("graph.viewGraph")}
            </Button>
          </div>
          {/* Conditional content */}
          <div className={viewMode === "graph" ? "h-[70vh]" : ""}>
            {viewMode === "graph" ? (
              <SessionGraph sessionId={sessionId} />
            ) : (
              <TranscriptView sessionId={sessionId} />
            )}
          </div>
        </section>
      </div>
    </ScrollArea>
  );
}

function CopyableText({ label, value }: { label: string; value: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [value]);

  return (
    <span
      className="group inline-flex cursor-pointer items-center gap-1 transition-colors hover:text-foreground"
      onClick={handleCopy}
    >
      {label}
      {copied ? (
        <CircleCheck size={16} className="text-primary" />
      ) : (
        <Copy size={16} className="shrink-0" />
      )}
    </span>
  );
}

function MetaPill({ label, value }: { label: string; value: string }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-sm border border-border bg-secondary px-1.5 py-0.5 text-[12px]">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{value}</span>
    </span>
  );
}
