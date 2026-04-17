import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  getSessionDetail,
  getDeletePlan,
  destructiveDeleteSession,
} from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";

export default function SourcesTab({ sessionId }: { sessionId: string }) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [planOpen, setPlanOpen] = useState(false);

  const { data: detail } = useQuery({
    queryKey: ["session-detail", sessionId],
    queryFn: () => getSessionDetail(sessionId),
  });

  const { data: deletePlan, isLoading: planLoading } = useQuery({
    queryKey: ["delete-plan", sessionId],
    queryFn: () => getDeletePlan(sessionId),
    enabled: planOpen,
  });

  const destructiveDelete = useMutation({
    mutationFn: () => destructiveDeleteSession(sessionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["session-detail", sessionId] });
      setPlanOpen(false);
    },
  });

  if (!detail) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 2 }).map((_, i) => (
          <Skeleton key={i} className="h-10 w-full" />
        ))}
      </div>
    );
  }

  if (detail.sources.length === 0) {
    return (
      <p className="text-[13px] text-muted-foreground/50">
        {t("sources.empty")}
      </p>
    );
  }

  return (
    <div className="space-y-3">
      <div className="space-y-2">
        {detail.sources.map((src) => (
          <div
            key={src.source_id}
            className="rounded border border-border p-2.5"
          >
            <div className="flex items-center gap-1.5">
              <Badge variant="outline" className="px-1.5 py-0.5 text-[12px]">
                {src.source_type}
              </Badge>
              <Badge
                variant={
                  src.delete_policy === "file_safe"
                    ? "secondary"
                    : src.delete_policy === "not_allowed"
                      ? "outline"
                      : "outline"
                }
                className="px-1.5 py-0.5 text-[12px]"
              >
                {src.delete_policy}
              </Badge>
            </div>
            <p className="mt-1 break-all font-mono text-[13px] text-muted-foreground">
              {src.path}
            </p>
          </div>
        ))}
      </div>

      {/* Delete plan section */}
      <Separator />

      <AlertDialog open={planOpen} onOpenChange={setPlanOpen}>
        <AlertDialogTrigger>
          <Button
            variant="destructive"
            size="sm"
            className="h-7 text-[13px]"
          >
            {t("sources.destructiveDelete")}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("sources.deleteTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {planLoading ? (
                t("sources.analyzing")
              ) : deletePlan ? (
                <span>
                  {t("sources.deleteDescription")}
                  <br /><br />
                  <strong>{t("sources.statusLabel")}</strong> {deletePlan.allowed ? t("sources.allowed") : t("sources.blocked")} — {deletePlan.reason}
                  <br /><br />
                  {deletePlan.sources.map((sp, i) => (
                    <span key={i}>
                      {sp.can_delete ? "✓" : "✗"} {sp.source.path.split("/").pop()}
                      {" — "}{sp.reason}
                      <br />
                    </span>
                  ))}
                </span>
              ) : (
                t("sources.loadingPlan")
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("sources.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              disabled={!deletePlan?.allowed || destructiveDelete.isPending}
              onClick={() => destructiveDelete.mutate()}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {destructiveDelete.isPending ? t("sources.deleting") : t("sources.deleteFiles")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
