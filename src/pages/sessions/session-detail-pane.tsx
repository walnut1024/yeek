import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionPreview, softDeleteSessions } from "@/lib/api";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { formatTime, formatRelativeTime } from "@/lib/formatters";
import TranscriptView from "./transcript-view";
import SourcesTab from "./sources-tab";
import SessionGraph from "./session-graph";
import { useLocalStorage } from "@/lib/hooks";

export default function SessionDetailPane({
  sessionId,
}: {
  sessionId: string;
}) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [viewMode, setViewMode] = useLocalStorage<"feed" | "graph">(
    "graph-view",
    "feed",
  );

  const { data: preview, isLoading: previewLoading } = useQuery({
    queryKey: ["session-preview", sessionId],
    queryFn: () => getSessionPreview(sessionId),
  });

  const softDel = useMutation({
    mutationFn: () => softDeleteSessions([sessionId]),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      setShowDeleteConfirm(false);
    },
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
        <section className="surface-card sticky top-0 z-20 bg-card/95 p-3 backdrop-blur-sm">
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
              <div className="mt-2 flex items-center gap-3 text-[12px] font-medium tracking-[0.14em] text-muted-foreground">
                <span>{t("detail.sourceLabel", { path: record.id })}</span>
                <span>{t("detail.sourcePath", { path: record.project_path })}</span>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              <Button
                size="sm"
                className="h-7 rounded-md px-2.5 text-[13px]"
                onClick={async () => {
                  try {
                    await invoke("resume_session", {
                      sessionId: record.id,
                      agent: record.agent,
                      cwd: record.project_path,
                    });
                  } catch (e) {
                    console.error("Failed to resume session:", e);
                  }
                }}
              >
                {t("detail.resume")}
              </Button>
              <Button
                size="sm"
                variant="destructive"
                className="h-7 rounded-md px-2.5 text-[13px]"
                onClick={() => setShowDeleteConfirm(true)}
              >
                {t("detail.delete")}
              </Button>
            </div>
          </div>

          <SourcesTab sessionId={sessionId} />
        </section>

        <AlertDialog
          open={showDeleteConfirm}
          onOpenChange={setShowDeleteConfirm}
        >
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{t("detail.deleteTitle")}</AlertDialogTitle>
              <AlertDialogDescription>
                {t("detail.deleteDescription")}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
              <AlertDialogAction
                disabled={softDel.isPending}
                onClick={() => softDel.mutate()}
                className="border-[#4c2b2c] bg-destructive/10 text-destructive hover:bg-destructive/20"
              >
                {softDel.isPending ? t("detail.deleting") : t("detail.delete")}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>

        <section className="surface-card overflow-hidden p-1">
          {/* Graph/Feed toggle */}
          <div className="flex items-center gap-1 border-b border-border px-2 py-1">
            <button
              className={`pill-tab ${viewMode === "feed" ? "pill-tab-active" : "pill-tab-idle"}`}
              onClick={() => setViewMode("feed")}
            >
              {t("graph.viewFeed")}
            </button>
            <button
              className={`pill-tab ${viewMode === "graph" ? "pill-tab-active" : "pill-tab-idle"}`}
              onClick={() => setViewMode("graph")}
            >
              {t("graph.viewGraph")}
            </button>
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

function MetaPill({ label, value }: { label: string; value: string }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-sm border border-border bg-secondary px-1.5 py-0.5 text-[12px]">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{value}</span>
    </span>
  );
}
