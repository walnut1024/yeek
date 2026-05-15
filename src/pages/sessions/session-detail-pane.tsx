import { useState, useCallback, lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionPreview, resumeSession } from "@/lib/api";
import { useLocalStorage } from "@/lib/hooks";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Copy, CircleCheck, Play, FolderTree, Rows3, GitBranch } from "lucide-react";
import { formatTime, formatRelativeTime } from "@/lib/formatters";
import { getSessionTranscript } from "@/lib/api";
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
  const [fullscreen, setFullscreen] = useState(false);

  const { data: preview, isLoading: previewLoading } = useQuery({
    queryKey: ["session-preview", sessionId],
    queryFn: () => getSessionPreview(sessionId),
  });

  const { data: transcript } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  const mainCount = transcript?.main_path.length ?? 0;
  const branchCount = transcript?.branches.length ?? 0;

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
    <>
      {fullscreen && viewMode === "graph" ? (
        <div className="fixed inset-0 z-50 flex flex-col bg-card">
          <div className="flex items-center gap-2 border-b border-border px-3 py-1.5">
            <p className="text-[13px] font-medium text-foreground">{sessionTitle}</p>
            <span className="font-mono text-[10px] text-muted-foreground">{record.id.slice(0, 8)}</span>
          </div>
          <div className="flex-1 min-h-0">
            <Suspense fallback={<DetailContentFallback graph />}>
              <SessionGraph sessionId={sessionId} fullscreen={fullscreen} onFullscreen={setFullscreen} />
            </Suspense>
          </div>
        </div>
      ) : (
      <ScrollArea className="h-full">
        <div className="space-y-3 p-3">
          {/* Header */}
          <section data-ai-region="sessions-summary" className="surface-card sticky top-0 z-20 bg-card/95 p-3 backdrop-blur-sm">
            <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
              <div className="min-w-0 flex-1">
                <div className="mt-1.5 flex items-baseline gap-2">
                  <p className="shrink-0 text-[16px] font-medium uppercase tracking-[0.04em] text-muted-foreground">{t("detail.sessionLabelWithColon")}</p>
                  <h3 className="min-w-0 truncate text-[14px] font-semibold leading-[1.3] text-foreground">
                    {sessionTitle}
                  </h3>
                  <Button
                    variant="primary"
                    size="sm"
                    className="ml-auto h-7 shrink-0 rounded-md px-2.5 text-[12px]"
                    onClick={async () => {
                      try {
                        await resumeSession(record.id, record.agent, record.project_path, defaultTerminal || null);
                      } catch (e) {
                        console.error("Failed to resume session:", e);
                      }
                    }}
                  >
                    <Play size={14} />
                    {t("detail.resume")}
                  </Button>
                </div>
                <div className="mt-2 flex flex-wrap items-center gap-1.5">
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
                <div className="mt-2 grid gap-2 md:grid-cols-2">
                  <CopyableDetailValue label={t("detail.projectLabel")} value={projectPath} icon={<FolderTree size={14} />} />
                  <CopyableDetailValue label={t("detail.sessionIdLabel")} value={record.id} />
                </div>
                <div className="mt-2">
                  <SourcesTab sessionId={sessionId} />
                </div>
              </div>
            </div>
            <div className="mt-3 border-t border-border pt-2">
              <div className="flex flex-wrap items-center justify-between gap-2 px-0.5">
                <div className="flex flex-wrap items-center gap-1.5">
                  <p className="zed-kicker">{t("detail.tabHistory")}</p>
                  {mainCount > 0 && (
                    <span className="inline-flex items-center gap-1 rounded-full border border-border bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                      <Rows3 size={12} />
                      {t("transcript.messageCount", { count: mainCount })}
                    </span>
                  )}
                  {branchCount > 0 && (
                    <span className="inline-flex items-center gap-1 rounded-full border border-border bg-card px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                      <GitBranch size={12} />
                      {t("transcript.branchCount", { count: branchCount })}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1.5">
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
              </div>
            </div>

          </section>

          <section data-ai-region="sessions-transcript" className="surface-card">
            {/* Conditional content */}
            <div className={viewMode === "graph" ? "h-[70vh]" : ""}>
              <Suspense fallback={<DetailContentFallback graph={viewMode === "graph"} />}>
                {viewMode === "graph" ? (
                  <SessionGraph sessionId={sessionId} fullscreen={fullscreen} onFullscreen={setFullscreen} />
                ) : (
                  <TranscriptView sessionId={sessionId} />
                )}
              </Suspense>
            </div>
          </section>
        </div>
      </ScrollArea>
      )}
    </>
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

function CopyableDetailValue({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [value]);

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="flex w-full flex-col items-start gap-0 rounded-lg border border-border bg-secondary px-2.5 py-2 text-left transition-colors hover:border-primary/30 hover:bg-element-hover"
    >
      <p className="zed-kicker flex items-center gap-1.5">
        {icon}
        <span>{label}</span>
        {copied ? (
          <CircleCheck size={12} className="text-primary" />
        ) : (
          <Copy size={12} className="text-muted-foreground" />
        )}
      </p>
      <p className="mt-0.5 break-all font-mono text-[11px] leading-[1.45] text-foreground">
        {value}
      </p>
    </button>
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
