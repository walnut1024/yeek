import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
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
      <p className="text-[10px] text-muted-foreground/50">
        No source files recorded
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
              <Badge variant="outline" className="px-1 py-0 text-[8px]">
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
                className="px-1 py-0 text-[8px]"
              >
                {src.delete_policy}
              </Badge>
            </div>
            <p className="mt-1 break-all font-mono text-[10px] text-muted-foreground">
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
            className="h-6 text-[10px]"
          >
            Destructive Delete
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Confirm Destructive Delete</AlertDialogTitle>
            <AlertDialogDescription>
              {planLoading ? (
                "Analyzing sources..."
              ) : deletePlan ? (
                <span>
                  This will permanently delete source files for this session.
                  <br /><br />
                  <strong>Status:</strong> {deletePlan.allowed ? "Allowed" : "Blocked"} — {deletePlan.reason}
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
                "Loading plan..."
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              disabled={!deletePlan?.allowed || destructiveDelete.isPending}
              onClick={() => destructiveDelete.mutate()}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {destructiveDelete.isPending ? "Deleting..." : "Delete Files"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
