import { useState, useCallback, lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionPreview, resumeSession } from "@/lib/api";
import { useLocalStorage } from "@/lib/hooks";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Copy, CircleCheck, Play, FolderTree } from "lucide-react";
import { formatTime, formatRelativeTime } from "@/lib/formatters";
import SourcesTab from "./sources-tab";

const TranscriptView = lazy(() => import("./transcript-view"));
const SessionGraph = lazy(() => import("./session-graph"));

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
  const sessionTitle = record.title || record.id.slice(0, 12);
  const projectPath = record.project_path || t("format.noProject");

  return (
    <ScrollArea className="h-full">
      <div className="space-y-3 p-3">
        {/* Header */}
        <section data-ai-region="sessions-summary" className="surface-card sticky top-0 z-20 bg-card/95 p-3 backdrop-blur-sm">
          <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
            <div className="min-w-0 flex-1">
              <p className="zed-kicker">{t("detail.sessionLabel")}</p>
              <h3 className="mt-1.5 truncate text-[20px] font-semibold leading-[1.1] tracking-[-0.02em] text-foreground">
                {sessionTitle}
              </h3>
              <div className="mt-2.5 grid gap-2 md:grid-cols-2">
                <DetailValue label={t("detail.projectLabel")} value={projectPath} icon={<FolderTree size={14} />} />
                <DetailValue label={t("detail.sessionIdLabel")} value={record.id} />
              </div>
              <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
                <CopyableAction label={t("detail.copySessionId")} value={record.id} />
                {record.project_path && (
                  <CopyableAction label={t("detail.copyProjectPath")} value={record.project_path} />
                )}
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Button
                variant="primary"
                size="sm"
                className="h-8 rounded-md px-3 text-[13px]"
                onClick={async () => {
                  try {
                    await resumeSession(record.id, record.agent, record.project_path, defaultTerminal || null);
                  } catch (e) {
                    console.error("Failed to resume session:", e);
                  }
                }}
              >
                <Play size={16} />
                {t("detail.resume")}
              </Button>
            </div>
          </div>

          <div className="mt-3 border-t border-border pt-3">
            <p className="zed-kicker">{t("detail.summaryTitle")}</p>
            <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
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
          </div>
          <div className="mt-3 border-t border-border pt-3">
            <SourcesTab sessionId={sessionId} />
          </div>
        </section>

        <section data-ai-region="sessions-transcript" className="surface-card overflow-hidden">
          {/* Graph/Feed toggle */}
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-2.5">
            <div className="min-w-0">
              <p className="zed-kicker">{t("detail.tabHistory")}</p>
              <p className="mt-0.5 truncate text-[12px] text-muted-foreground">{t("detail.historyDescription")}</p>
            </div>
            <div className="segmented-control">
              <Button
                type="button"
                variant="secondary"
                size="sm"
                className={`segmented-control-item ${viewMode === "feed" ? "segmented-control-item-active" : ""}`}
                onClick={() => setViewMode("feed")}
              >
                {t("graph.viewFeed")}
              </Button>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                className={`segmented-control-item ${viewMode === "graph" ? "segmented-control-item-active" : ""}`}
                onClick={() => setViewMode("graph")}
              >
                {t("graph.viewGraph")}
              </Button>
            </div>
          </div>
          {/* Conditional content */}
          <div className={viewMode === "graph" ? "h-[70vh]" : ""}>
            <Suspense fallback={<DetailContentFallback graph={viewMode === "graph"} />}>
              {viewMode === "graph" ? (
                <SessionGraph sessionId={sessionId} />
              ) : (
                <TranscriptView sessionId={sessionId} />
              )}
            </Suspense>
          </div>
        </section>
      </div>
    </ScrollArea>
  );
}

function CopyableAction({ label, value }: { label: string; value: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [value]);

  return (
    <Button
      type="button"
      variant="outline"
      size="xs"
      className="text-muted-foreground hover:text-foreground"
      onClick={handleCopy}
    >
      <span className="truncate">{label}</span>
      {copied ? (
        <CircleCheck size={16} className="text-primary" />
      ) : (
        <Copy size={16} className="shrink-0" />
      )}
    </Button>
  );
}

function MetaPill({ label, value }: { label: string; value: string }) {
  return (
    <span className="meta-pill">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{value}</span>
    </span>
  );
}

function DetailValue({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-border bg-secondary px-2.5 py-2">
      <p className="zed-kicker flex items-center gap-1.5">
        {icon}
        <span>{label}</span>
      </p>
      <p className="mt-0.5 break-all text-[12px] leading-[1.45] text-foreground">
        {value}
      </p>
    </div>
  );
}

function DetailContentFallback({ graph }: { graph: boolean }) {
  return (
    <div className="space-y-3 p-4">
      <Skeleton className="h-16 w-full rounded-xl" />
      <Skeleton className={`${graph ? "h-[520px]" : "h-40"} w-full rounded-xl`} />
    </div>
  );
}
