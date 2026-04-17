import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  getSessionPreview,
  softDeleteSessions,
} from "@/lib/api";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
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

export default function SessionDetailPane({ sessionId }: { sessionId: string }) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [tab, setTab] = useState("transcript");
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

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
        <section className="surface-card p-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              {/* Meta pills */}
              <div className="flex flex-wrap items-center gap-1">
                <MetaPill label={t("detail.model")} value={record.model || t("format.notAvailable")} />
                <MetaPill label={t("detail.branch")} value={record.git_branch || t("format.notAvailable")} />
                <MetaPill label={t("detail.status")} value={record.status} />
                <MetaPill label={t("detail.messages")} value={String(record.message_count)} />
                <MetaPill label={t("detail.sources")} value={String(preview.source_count)} />
                <MetaPill label={t("detail.started")} value={formatTime(record.started_at)} />
                <MetaPill label={t("detail.updated")} value={formatRelativeTime(record.updated_at)} />
              </div>
              <p className="mt-2 zed-kicker">{t("detail.sourceLabel", { path: record.project_path })}</p>
              {/* Session ID */}
              <div className="mt-2 flex items-center gap-2 rounded-md bg-accent px-2.5 py-1.5">
                <code className="min-w-0 flex-1 truncate font-mono text-[12px] text-muted-foreground">
                  {record.agent} --resume {record.id}
                </code>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 shrink-0 rounded-md px-1.5 text-[12px] text-muted-foreground hover:text-foreground"
                  onClick={() => navigator.clipboard.writeText(record.id)}
                >
                  {t("detail.copy")}
                </Button>
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




        </section>

        <AlertDialog open={showDeleteConfirm} onOpenChange={setShowDeleteConfirm}>
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

        {/* Tabs: Transcript & Sources */}
        <section className="surface-card p-2">
          <Tabs value={tab} onValueChange={setTab}>
            <TabsList className="h-auto p-0.5">
              <TabsTrigger
                value="transcript"
                className="rounded-md px-3 py-1.5 text-[13px]"
              >
                {t("detail.tabHistory")}
              </TabsTrigger>
              <TabsTrigger
                value="sources"
                className="rounded-md px-3 py-1.5 text-[13px]"
              >
                {t("detail.tabSources")}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </section>

        {tab === "transcript" && (
          <section className="surface-card overflow-hidden p-1">
            <TranscriptView sessionId={sessionId} />
          </section>
        )}

        {tab === "sources" && (
          <section className="surface-card overflow-hidden p-1">
            <SourcesTab sessionId={sessionId} />
          </section>
        )}
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
