import React, { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { getSystemStatus, getActionLog, rescanSources, releaseAndResync } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatTime, formatRelativeTime, getCurrentLocale } from "@/lib/formatters";

const StatCard = React.memo(function StatCard({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: string;
  tone?: "neutral" | "brand";
}) {
  return (
    <div
      className={`border p-2.5 ${
        tone === "brand"
          ? "border-border bg-secondary text-foreground"
          : "bg-card border border-border text-foreground"
      }`}
    >
      <p
        className={`text-[11px] uppercase tracking-[0.08em] ${
          tone === "brand" ? "text-primary" : "text-muted-foreground"
        }`}
      >
        {label}
      </p>
      <p className="mt-2 font-mono text-[18px] font-medium leading-none">
        {value}
      </p>
    </div>
  );
});

export default function SystemPage() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [scanProgress, setScanProgress] = useState<{ processed: number; total: number } | null>(null);
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
      // Scan started in background — data refreshes on sync-completed event
    },
  });

  const [confirmRelease, setConfirmRelease] = useState(false);
  const release = useMutation({
    mutationFn: releaseAndResync,
    onSuccess: () => {
      setConfirmRelease(false);
    },
  });

  // Listen for scan progress and completion events
  useEffect(() => {
    const unlistenStarted = listen<{ source_count: number }>("sync-started", (event) => {
      setScanProgress({ processed: 0, total: event.payload.source_count });
    });
    const unlistenProgress = listen<{ processed: number; total: number }>("sync-progress", (event) => {
      setScanProgress(event.payload);
    });
    const unlistenCompleted = listen("sync-completed", () => {
      setScanProgress(null);
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["system-status"] });
      queryClient.invalidateQueries({ queryKey: ["action-log"] });
    });

    return () => {
      unlistenStarted.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenCompleted.then((f) => f());
    };
  }, [queryClient]);

  const isScanning = scanProgress !== null;

  const errorActions =
    actionLog?.actions.filter(
      (a: { detail: string | null }) =>
        a.detail?.includes("errors=") && !a.detail.includes("errors=0")
    ) ?? [];

  return (
    <div className="grid h-full min-h-0 xl:grid-cols-[minmax(0,1.2fr)_320px]">
      <section className="surface-panel overflow-auto p-3">
        <div className="flex flex-col gap-2 border-b border-border pb-3 md:flex-row md:items-end md:justify-between">
          <div>
            <p className="zed-kicker">{t("system.operations")}</p>
            <h2 className="mt-1 text-[14px] font-medium leading-none text-foreground">
              {t("system.title")}
            </h2>
            <p className="mt-2 max-w-2xl text-[14px] leading-[1.5] text-muted-foreground">
              {t("system.description")}
            </p>
          </div>
          <div className="flex items-center gap-2">
            {confirmRelease ? (
              <div className="flex items-center gap-2">
                <span className="text-[13px] text-destructive">{t("system.confirmRelease")}</span>
                <Button
                  size="sm"
                  className="h-7 rounded-md px-2.5 text-[13px]"
                  variant="outline"
                  onClick={() => setConfirmRelease(false)}
                  disabled={isScanning}
                >
                  {t("manage.cancel")}
                </Button>
                <Button
                  size="sm"
                  className="h-7 rounded-md px-2.5 text-[13px] font-medium"
                  variant="destructive"
                  onClick={() => release.mutate()}
                  disabled={isScanning}
                >
                  {t("system.confirmReleaseBtn")}
                </Button>
              </div>
            ) : (
              <Button
                size="sm"
                className="h-7 rounded-md px-2.5 text-[13px] text-muted-foreground hover:border-destructive hover:text-destructive disabled:opacity-60"
                variant="outline"
                onClick={() => setConfirmRelease(true)}
                disabled={isScanning}
              >
                {t("system.release")}
              </Button>
            )}
            <Button
              size="sm"
              className="h-7 rounded-md px-2.5 text-[13px] font-medium disabled:opacity-60"
              onClick={() => rescan.mutate()}
              disabled={isScanning}
            >
              {isScanning ? (
                <span className="flex items-center gap-2">
                  <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-primary-foreground/30 border-t-primary-foreground" />
                  {scanProgress!.total > 0
                    ? `${scanProgress!.processed}/${scanProgress!.total}`
                    : t("system.starting")}
                </span>
              ) : (
                t("system.rescan")
              )}
            </Button>
          </div>
        </div>

        {isScanning && scanProgress!.total > 0 && (
          <div className="mt-3 h-1 overflow-hidden rounded-full bg-secondary">
            <div
              className="h-full rounded-full bg-primary transition-all duration-300 ease-out"
              style={{ width: `${Math.round((scanProgress!.processed / scanProgress!.total) * 100)}%` }}
            />
          </div>
        )}

        <div className="mt-3 grid gap-px border border-border bg-border md:grid-cols-3">
          <StatCard label={t("system.statSessions")} value={String(status?.total_sessions ?? "-")} />
          <StatCard label={t("system.statSources")} value={String(status?.total_sources ?? "-")} />
          <StatCard
            label={t("system.statLastSync")}
            value={status?.last_sync_at ? formatTime(status.last_sync_at) : t("system.never")}
            tone="brand"
          />
        </div>

        {errorActions.length > 0 && (
          <div className="mt-4">
            <p className="zed-kicker text-destructive">{t("system.attention")}</p>
            <h3 className="mt-1 text-[14px] font-medium leading-none text-foreground">{t("system.syncIssues")}</h3>
            <div className="mt-2 space-y-2">
              {errorActions.map(
                (a: {
                  id: number;
                  action: string;
                  detail: string | null;
                  created_at: string;
                }) => (
                  <div
                    key={a.id}
                    className="border border-destructive/30 bg-destructive/10 px-2.5 py-2"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <Badge className="border-[#4c2b2c] bg-destructive/10 px-1.5 py-0.5 text-[12px] text-destructive">
                        {a.action}
                      </Badge>
                      <span className="font-mono text-[12px] text-muted-foreground">
                        {formatRelativeTime(a.created_at)}
                      </span>
                    </div>
                    <p className="mt-2 text-[14px] leading-[1.5] text-destructive/80">
                      {a.detail}
                    </p>
                  </div>
                )
              )}
            </div>
          </div>
        )}

        <div className="mt-4">
          <p className="zed-kicker">{t("system.activity")}</p>
          <h3 className="mt-1 text-[14px] font-medium leading-none text-foreground">{t("system.recentActions")}</h3>
          {!actionLog?.actions || actionLog.actions.length === 0 ? (
            <div className="mt-2 border border-border bg-secondary px-2.5 py-2 text-[14px] text-muted-foreground">
              {t("system.noActions")}
            </div>
          ) : (
            <div className="mt-2 flex">
              {/* Axis column */}
              <div className="flex w-[88px] flex-col items-center border-r border-border pr-0">
                <div className="h-3" />
                <span className="text-[13px] font-medium text-foreground">{t("system.today")}</span>
                <span className="text-[12px] text-muted-foreground">
                  {new Date().toLocaleDateString(getCurrentLocale(), { month: "short", day: "numeric", year: "numeric" })}
                </span>
                <div className="h-2" />
                <div className="flex-1 w-[2px] bg-border" />
              </div>

              {/* Entries */}
              <div className="flex flex-1 flex-col">
                {actionLog.actions.map(
                  (a: {
                    id: number;
                    action: string;
                    detail: string | null;
                    session_id: string | null;
                    created_at: string;
                  }) => {
                    const isDestructive = a.action.includes("delete") || a.action.includes("destructive");
                    const isSync = a.action.includes("sync") || a.action.includes("rescan");
                    const dotColor = isDestructive ? "bg-destructive" : isSync ? "bg-primary" : "bg-muted-foreground";
                    const cardBorder = isDestructive ? "border-destructive/30" : isSync ? "border-primary/30" : "border-border";
                    const cardBg = isDestructive ? "bg-destructive/5" : isSync ? "bg-primary/5" : "bg-card";
                    const badgeBg = isDestructive ? "bg-destructive" : isSync ? "bg-primary" : "bg-muted-foreground";
                    const textColor = isDestructive ? "text-destructive/80" : "text-muted-foreground";

                    return (
                      <div key={a.id} className="flex items-center">
                        {/* Time label */}
                        <div className="flex w-[88px] flex-col items-end justify-center pr-3">
                          <span className="font-mono text-[13px] font-medium text-foreground">
                            {a.created_at ? new Date(a.created_at).toLocaleTimeString(getCurrentLocale(), { hour: "2-digit", minute: "2-digit", hour12: false }) : "--:--"}
                          </span>
                          <span className="text-[12px] text-muted-foreground">
                            {formatRelativeTime(a.created_at)}
                          </span>
                        </div>

                        {/* Dot node */}
                        <span className={`flex size-3.5 shrink-0 items-center justify-center rounded-full ${dotColor}`}>
                          <span className="size-[5px] rounded-full bg-card" />
                        </span>

                        {/* Card */}
                        <div className={`ml-3 mb-3 flex-1 border p-2 ${cardBg} ${cardBorder}`}>
                          <div className="flex items-center gap-2">
                            <span className={`rounded-sm px-1.5 py-0.5 text-[12px] font-medium text-primary-foreground ${badgeBg}`}>
                              {a.action}
                            </span>
                          </div>
                          <p className={`mt-1 text-[14px] leading-[1.5] ${textColor}`}>
                            {a.detail || a.session_id?.slice(0, 8) || t("system.noDetail")}
                          </p>
                        </div>
                      </div>
                    );
                  }
                )}
              </div>
            </div>
          )}
        </div>
      </section>

      <aside className="surface-panel overflow-hidden border-l border-border p-3">
        <p className="zed-kicker">{t("system.opsNotes")}</p>
        <h3 className="mt-2 text-[16px] font-medium leading-[1.2] text-foreground">
          {t("system.opsHeading")}
        </h3>
        <p className="mt-2 text-[14px] leading-[1.5] text-muted-foreground">
          {t("system.opsDescription")}
        </p>
        <div className="mt-3 space-y-2">
          <SystemHint title={t("system.hintRescanTitle")} body={t("system.hintRescanBody")} />
          <SystemHint title={t("system.hintErrorTitle")} body={t("system.hintErrorBody")} />
          <SystemHint title={t("system.hintAuditTitle")} body={t("system.hintAuditBody")} />
        </div>
      </aside>
    </div>
  );
}

function SystemHint({ title, body }: { title: string; body: string }) {
  return (
    <div className="border border-border bg-secondary/50 p-2.5">
      <p className="text-[14px] font-medium text-foreground">
        {title}
      </p>
      <p className="mt-1.5 text-[14px] leading-[1.5] text-muted-foreground">{body}</p>
    </div>
  );
}
