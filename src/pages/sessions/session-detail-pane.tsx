import React, { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getSessionPreview,
  setPinned,
  setArchived,
  setHidden,
  softDeleteSessions,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
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

const MetaField = React.memo(function MetaField({
  label,
  value,
}: {
  label: string;
  value: string | null;
}) {
  return (
    <div>
      <span className="text-[10px] text-muted-foreground">{label}</span>
      <p className="text-xs font-medium">{value || "N/A"}</p>
    </div>
  );
});

export default function SessionDetailPane({ sessionId }: { sessionId: string }) {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState("overview");
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const { data: preview, isLoading: previewLoading } = useQuery({
    queryKey: ["session-preview", sessionId],
    queryFn: () => getSessionPreview(sessionId),
  });

  const pin = useMutation({
    mutationFn: (value: boolean) => setPinned([sessionId], value),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["sessions"] }),
  });
  const archive = useMutation({
    mutationFn: (value: boolean) => setArchived([sessionId], value),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["sessions"] }),
  });
  const hide = useMutation({
    mutationFn: (value: boolean) => setHidden([sessionId], value),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["sessions"] }),
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
      <div className="space-y-3 p-4">
        <Skeleton className="h-6 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
        <Skeleton className="h-20 w-full" />
      </div>
    );
  }

  const { record } = preview;

  return (
    <ScrollArea className="h-full">
      <div className="p-4 space-y-4">
        {/* Title */}
        <div>
          <h2 className="text-sm font-semibold leading-tight">
            {record.title || record.id.slice(0, 12)}
          </h2>
          {record.project_path && (
            <p className="mt-0.5 text-[10px] text-muted-foreground">
              {record.project_path}
            </p>
          )}
        </div>

        {/* Actions */}
        <div className="flex flex-wrap gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-6 text-[10px]"
            onClick={() => pin.mutate(!record.pinned)}
          >
            {record.pinned ? "Unpin" : "Pin"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-6 text-[10px]"
            onClick={() =>
              archive.mutate(record.visibility !== "archived")
            }
          >
            {record.visibility === "archived" ? "Unarchive" : "Archive"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-6 text-[10px]"
            onClick={() =>
              hide.mutate(record.visibility !== "hidden")
            }
          >
            {record.visibility === "hidden" ? "Unhide" : "Hide"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-[10px] text-destructive hover:text-destructive"
            onClick={() => setShowDeleteConfirm(true)}
          >
            Delete
          </Button>
        </div>

        {/* Delete confirmation dialog */}
        <AlertDialog open={showDeleteConfirm} onOpenChange={setShowDeleteConfirm}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Soft Delete Session</AlertDialogTitle>
              <AlertDialogDescription>
                This will mark the session as deleted. The source files will be
                preserved.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction
                disabled={softDel.isPending}
                onClick={() => softDel.mutate()}
                className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              >
                {softDel.isPending ? "Deleting..." : "Delete"}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>

        <Separator />

        {/* Tabs */}
        <Tabs value={tab} onValueChange={setTab}>
          <TabsList className="h-7">
            <TabsTrigger value="overview" className="text-[10px] px-2">
              Overview
            </TabsTrigger>
            <TabsTrigger value="transcript" className="text-[10px] px-2">
              Transcript
            </TabsTrigger>
            <TabsTrigger value="sources" className="text-[10px] px-2">
              Sources
            </TabsTrigger>
          </TabsList>
        </Tabs>

        {/* Overview tab */}
        {tab === "overview" && (
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
              <MetaField label="Agent" value={record.agent === "claude_code" ? "Claude Code" : record.agent} />
              <MetaField label="Model" value={record.model} />
              <MetaField label="Branch" value={record.git_branch} />
              <MetaField label="Messages" value={String(record.message_count)} />
              <MetaField label="Started" value={formatTime(record.started_at)} />
              <MetaField label="Status" value={record.status} />
              <MetaField label="Sources" value={String(preview.source_count)} />
              <MetaField label="Updated" value={formatRelativeTime(record.updated_at)} />
            </div>

            <Separator />

            <div>
              <p className="mb-1.5 text-[10px] font-medium uppercase text-muted-foreground/60">
                Preview
              </p>
              {preview.preview_messages.length === 0 ? (
                <p className="text-[10px] text-muted-foreground/50">
                  No preview messages
                </p>
              ) : (
                <div className="space-y-1.5">
                  {preview.preview_messages.map(
                    (msg: { role: string; content_preview: string }, i: number) => (
                      <div
                        key={i}
                        className="rounded bg-muted/40 px-2.5 py-1.5"
                      >
                        <span className="text-[9px] font-medium uppercase text-muted-foreground">
                          {msg.role}
                        </span>
                        <p className="mt-0.5 line-clamp-3 text-[11px] leading-snug">
                          {msg.content_preview}
                        </p>
                      </div>
                    )
                  )}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Transcript tab */}
        {tab === "transcript" && (
          <TranscriptView sessionId={sessionId} />
        )}

        {/* Sources tab */}
        {tab === "sources" && (
          <SourcesTab sessionId={sessionId} />
        )}
      </div>
    </ScrollArea>
  );
}
