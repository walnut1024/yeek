import React from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getSystemStatus,
  getActionLog,
  rescanSources,
} from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatTime, formatRelativeTime } from "@/lib/formatters";

const StatCard = React.memo(function StatCard({
  label,
  value,
}: {
  label: string;
  value: number | undefined;
}) {
  return (
    <div className="rounded-lg border border-border p-4">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-2xl font-semibold">{value ?? "-"}</p>
    </div>
  );
});

export default function SystemPage() {
  const queryClient = useQueryClient();
  const { data: status } = useQuery({
    queryKey: ["system-status"],
    queryFn: getSystemStatus,
  });
  const { data: actionLog } = useQuery({
    queryKey: ["action-log"],
    queryFn: () => getActionLog(50),
  });

  const rescan = useMutation({
    mutationFn: rescanSources,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["system-status"] });
      queryClient.invalidateQueries({ queryKey: ["action-log"] });
    },
  });

  const errorActions = actionLog?.actions.filter(
    (a: { detail: string | null }) => a.detail?.includes("errors=") && !a.detail.includes("errors=0")
  ) ?? [];

  return (
    <div className="h-full overflow-auto p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-sm font-semibold">System</h2>
        <Button
          variant="outline"
          size="sm"
          className="h-6 text-[10px]"
          onClick={() => rescan.mutate()}
          disabled={rescan.isPending}
        >
          {rescan.isPending ? "Scanning..." : "Rescan Now"}
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-4 mb-6">
        <StatCard label="Sessions" value={status?.total_sessions} />
        <StatCard label="Sources" value={status?.total_sources} />
        <div className="rounded-lg border border-border p-4">
          <p className="text-xs text-muted-foreground">Last Sync</p>
          <p className="text-sm font-medium">
            {status?.last_sync_at
              ? formatTime(status.last_sync_at)
              : "Never"}
          </p>
        </div>
      </div>

      {errorActions.length > 0 && (
        <>
          <h3 className="mb-2 text-xs font-medium text-destructive">
            Sync Issues
          </h3>
          <div className="mb-6 space-y-1">
            {errorActions.map(
              (a: {
                id: number;
                action: string;
                detail: string | null;
                created_at: string;
              }) => (
                <div
                  key={a.id}
                  className="rounded border border-destructive/30 bg-destructive/5 px-3 py-1.5 text-xs"
                >
                  <span className="text-muted-foreground">
                    {a.detail}
                  </span>
                  <span className="ml-2 text-muted-foreground/40">
                    {formatRelativeTime(a.created_at)}
                  </span>
                </div>
              )
            )}
          </div>
        </>
      )}

      <h3 className="mb-2 text-xs font-medium text-muted-foreground">
        Recent Actions
      </h3>
      <div className="space-y-1">
        {actionLog?.actions.map(
          (a: {
            id: number;
            action: string;
            detail: string | null;
            session_id: string | null;
            created_at: string;
          }) => (
            <div
              key={a.id}
              className="flex items-center justify-between rounded px-3 py-1.5 text-xs"
            >
              <div className="flex items-center gap-2">
                <Badge
                  variant={
                    a.action.includes("delete") || a.action.includes("destructive")
                      ? "destructive"
                      : a.action.includes("sync") || a.action.includes("rescan")
                        ? "secondary"
                        : "outline"
                  }
                  className="px-1 py-0 text-[9px] font-normal"
                >
                  {a.action}
                </Badge>
                <span className="text-muted-foreground">
                  {a.detail || a.session_id?.slice(0, 8) || ""}
                </span>
              </div>
              <span className="text-muted-foreground/40">
                {formatRelativeTime(a.created_at)}
              </span>
            </div>
          )
        )}
        {(!actionLog?.actions || actionLog.actions.length === 0) && (
          <p className="text-xs text-muted-foreground/50">No actions yet</p>
        )}
      </div>
    </div>
  );
}
