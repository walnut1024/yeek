import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSystemStatus, getActionLog, getProxyMetrics, listPlugins } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { formatTime, formatRelativeTime, getCurrentLocale } from "@/lib/formatters";

const VISIBLE_ACTIONS = 5;

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
  return String(n);
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const [showAllActions, setShowAllActions] = useState(false);

  const { data: status } = useQuery({
    queryKey: ["system-status"],
    queryFn: getSystemStatus,
  });

  const { data: actionLog } = useQuery({
    queryKey: ["action-log"],
    queryFn: () => getActionLog(50),
  });

  const { data: metrics } = useQuery({
    queryKey: ["proxy-metrics"],
    queryFn: getProxyMetrics,
    refetchInterval: 5000,
  });

  const { data: plugins } = useQuery({
    queryKey: ["plugins", "global"],
    queryFn: () => listPlugins("global"),
    staleTime: 60_000,
  });

  const allActions = actionLog?.actions ?? [];
  const visibleActions = showAllActions ? allActions : allActions.slice(0, VISIBLE_ACTIONS);
  const hiddenCount = allActions.length - VISIBLE_ACTIONS;

  const errorActions =
    allActions.filter(
      (a) => a.detail?.includes("errors=") && !a.detail.includes("errors=0")
    ) ?? [];

  const pluginHealth = plugins?.health_summary;

  return (
    <div className="surface-panel overflow-auto p-4">
      <div className="mx-auto flex max-w-[1056px] flex-col gap-7">

        {/* ═══ Data Overview ═══ */}
        <section>
          <div className="mb-2.5 flex items-baseline gap-2.5">
            <h2 className="text-[12px] font-medium text-foreground">{t("dashboard.dataOverview")}</h2>
            <span className="font-mono text-[11px] text-muted-foreground">{t("dashboard.dataSub")}</span>
          </div>
          <div className="grid grid-cols-4 gap-3">
            <StatCard
              label={t("dashboard.statSessions")}
              value={String(status?.total_sessions ?? "-")}
              sub={t("dashboard.statSessionsSub")}
            />
            <StatCard
              label={t("dashboard.statSources")}
              value={String(status?.total_sources ?? "-")}
              sub={t("dashboard.statSourcesSub")}
            />
            <StatCard
              label={t("dashboard.statProjects")}
              value={String(status?.total_projects ?? "-")}
              sub={t("dashboard.statProjectsSub")}
            />
            <StatCard
              label={t("dashboard.statMessages")}
              value={status?.total_messages != null ? formatCount(status.total_messages) : "-"}
              sub={t("dashboard.statMessagesSub")}
              accent
            />
          </div>
        </section>

        {/* ═══ Proxy Runtime ═══ */}
        {metrics && (
          <section>
            <div className="mb-2.5 flex items-baseline gap-2.5">
              <h2 className="text-[12px] font-medium text-foreground">{t("dashboard.proxyRuntime")}</h2>
              <span className="font-mono text-[11px] text-muted-foreground">{t("dashboard.proxySub")}</span>
              <span className="ml-auto font-mono text-[11px] text-muted-foreground">{metrics.version}</span>
            </div>
            <div className="grid grid-cols-6 gap-3">
              <MetricCard label={t("dashboard.metricUptime")} value={`${Math.floor(metrics.uptime_secs / 60)}m`} sub={`${metrics.uptime_secs}s`} />
              <MetricCard label={t("dashboard.metricRPS")} value={metrics.rps.toFixed(1)} sub="req/s" />
              <MetricCard label={t("dashboard.metricLatency")} value={`${metrics.avg_latency_ms.toFixed(0)}ms`} sub="avg" />
              <MetricCard label={t("dashboard.metricActive")} value={String(metrics.active_connections)} sub="connections" />
              <MetricCard label={t("dashboard.metricRequests")} value={String(metrics.request_count)} sub="total" />
              <MetricCard label={t("dashboard.metricErrors")} value={String(metrics.error_count)} sub={`${((metrics.error_count / Math.max(metrics.request_count, 1)) * 100).toFixed(2)}%`} danger={metrics.error_count > 0} />
            </div>
          </section>
        )}

        {/* ═══ System Health ═══ */}
        <section>
          <div className="mb-2.5 flex items-baseline gap-2.5">
            <h2 className="text-[12px] font-medium text-foreground">{t("dashboard.systemHealth")}</h2>
            <span className="font-mono text-[11px] text-muted-foreground">{t("dashboard.healthSub")}</span>
          </div>
          <div className="grid grid-cols-3 gap-3">
            <HealthCard
              icon="sync"
              value={status?.last_sync_at ? formatRelativeTime(status.last_sync_at) : t("dashboard.never")}
              label={t("dashboard.lastSync")}
              sub={status?.last_sync_at ? formatTime(status.last_sync_at) : undefined}
            />
            <HealthCard
              icon="live"
              value={status ? t("dashboard.activeCount", { count: status.active_sessions }) : "-"}
              label={t("dashboard.activeSessions")}
              sub={status ? t("dashboard.sessionBreakdown", { complete: status.complete_sessions, partial: status.partial_sessions }) : undefined}
            />
            <HealthCard
              icon="plugin"
              value={plugins ? t("dashboard.pluginCount", { count: plugins.total_plugins }) : "-"}
              label={t("dashboard.pluginHealth")}
              health={pluginHealth}
            />
          </div>
        </section>

        {/* ═══ Sync Alerts ═══ */}
        {errorActions.length > 0 && (
          <section>
            <div className="mb-2.5 flex items-baseline gap-2.5">
              <h2 className="text-[12px] font-medium text-destructive">{t("dashboard.syncAlerts")}</h2>
              <span className="font-mono text-[11px] text-destructive">{t("dashboard.alertIssues", { count: errorActions.length })}</span>
            </div>
            <div className="grid grid-cols-2 gap-3">
              {errorActions.slice(0, 4).map((a) => (
                <div key={a.id} className="rounded-[10px] border border-destructive/15 bg-card px-4 py-3">
                  <div className="flex items-center gap-2">
                    <span className="size-1.5 shrink-0 rounded-full bg-destructive" />
                    <span className="font-mono text-[10.5px] font-medium text-destructive">{a.action}</span>
                    <span className="ml-auto font-mono text-[11px] text-muted-foreground">
                      {a.created_at ? new Date(a.created_at).toLocaleTimeString(getCurrentLocale(), { hour: "2-digit", minute: "2-digit", hour12: false }) : ""}
                    </span>
                  </div>
                  <p className="mt-1.5 ml-3.5 text-[12px] leading-[1.45] text-foreground/50">{a.detail}</p>
                </div>
              ))}
            </div>
          </section>
        )}

        {/* ═══ Recent Activity ═══ */}
        <section>
          <div className="mb-2.5 flex items-baseline gap-2.5">
            <h2 className="text-[12px] font-medium text-foreground">{t("dashboard.recentActions")}</h2>
            <span className="ml-auto font-mono text-[11px] text-muted-foreground">{t("dashboard.totalActions", { count: allActions.length })}</span>
          </div>
          {allActions.length === 0 ? (
            <div className="rounded-[10px] border border-border bg-card px-4 py-3 text-[13px] text-muted-foreground">
              {t("dashboard.noActions")}
            </div>
          ) : (
            <div className="divide-y divide-border overflow-hidden rounded-[10px] border border-border bg-card">
              {visibleActions.map((a) => {
                const isDestructive = a.action.includes("delete") || a.action.includes("destructive");
                const isSync = a.action.includes("sync") || a.action.includes("release");
                const dotColor = isDestructive ? "bg-destructive" : isSync ? "bg-primary" : "bg-muted-foreground";
                return (
                  <div key={a.id} className="flex items-center gap-2.5 px-4 py-2.5">
                    <span className={`size-1.5 shrink-0 rounded-full ${dotColor}`} />
                    <span className="w-[100px] shrink-0 rounded px-1.5 py-0.5 text-center font-mono text-[10.5px] font-medium text-foreground/55 bg-foreground/5">
                      {a.action}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-[12.5px] text-foreground/75">
                      {a.detail || a.session_id?.slice(0, 8) || t("dashboard.noDetail")}
                    </span>
                    <span className="shrink-0 font-mono text-[11px] text-muted-foreground">
                      {a.created_at ? new Date(a.created_at).toLocaleTimeString(getCurrentLocale(), { hour: "2-digit", minute: "2-digit", hour12: false }) : ""}
                    </span>
                  </div>
                );
              })}
              {hiddenCount > 0 && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8 w-full text-[12px] text-muted-foreground hover:text-foreground"
                  onClick={() => setShowAllActions(!showAllActions)}
                >
                  {showAllActions
                    ? t("dashboard.collapseActions")
                    : t("dashboard.expandActions", { count: hiddenCount })}
                </Button>
              )}
            </div>
          )}
        </section>

      </div>
    </div>
  );
}

/* ── Sub-components ── */

function StatCard({ label, value, sub, accent }: { label: string; value: string; sub: string; accent?: boolean }) {
  return (
    <div className={`rounded-[10px] border border-border bg-card px-5 py-4 ${accent ? "bg-gradient-to-br from-[#0f1729] to-[#14171f] border-primary/15" : ""}`}>
      <p className="text-[10.5px] font-medium uppercase tracking-[0.07em] text-muted-foreground">{label}</p>
      <p className={`mt-2.5 font-mono text-[26px] font-semibold leading-none tracking-[-0.04em] ${accent ? "text-primary" : "text-foreground"}`}>{value}</p>
      <p className="mt-1.5 text-[12px] text-muted-foreground">{sub}</p>
    </div>
  );
}

function MetricCard({ label, value, sub, danger }: { label: string; value: string; sub: string; danger?: boolean }) {
  return (
    <div className="rounded-[10px] border border-border bg-card px-4 py-3.5 text-center">
      <p className={`font-mono text-[19px] font-semibold leading-none tracking-[-0.03em] ${danger ? "text-amber-400" : "text-foreground"}`}>{value}</p>
      <p className="mt-1 text-[10px] font-medium uppercase tracking-[0.06em] text-muted-foreground">{label}</p>
      <p className="mt-0.5 font-mono text-[10px] text-muted-foreground/50">{sub}</p>
    </div>
  );
}

function HealthCard({ icon, value, label, sub, health }: {
  icon: "sync" | "live" | "plugin";
  value: string;
  label: string;
  sub?: string;
  health?: { ok: number; partial: number; hook: number; broken: number } | null;
}) {
  const iconEl = {
    sync:   <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>,
    live:   <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>,
    plugin: <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polygon points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5 12 2"/><line x1="12" y1="22" x2="12" y2="15.5"/><polyline points="22 8.5 12 15.5 2 8.5"/></svg>,
  }[icon];
  const iconBg = { sync: "bg-primary/10 text-primary", live: "bg-emerald-500/10 text-emerald-500", plugin: "bg-foreground/5 text-foreground" }[icon];

  const total = health ? health.ok + health.partial + health.hook + health.broken : 0;

  return (
    <div className="flex items-center gap-3 rounded-[10px] border border-border bg-card px-4 py-3.5">
      <div className={`flex size-8 shrink-0 items-center justify-center rounded-lg ${iconBg}`}>
        {iconEl}
      </div>
      <div className="min-w-0">
        <p className="font-mono text-[14px] font-semibold leading-tight">{value}</p>
        <p className="text-[11px] text-muted-foreground">{label}</p>
        {health && total > 0 ? (
          <div className="mt-1">
            <div className="flex h-[3px] gap-0.5 overflow-hidden rounded-sm">
              {health.ok > 0 && <span className="h-full rounded-sm bg-emerald-500" style={{ flex: health.ok }} />}
              {health.partial > 0 && <span className="h-full rounded-sm bg-amber-400" style={{ flex: health.partial }} />}
              {health.hook > 0 && <span className="h-full rounded-sm bg-amber-400" style={{ flex: health.hook }} />}
              {health.broken > 0 && <span className="h-full rounded-sm bg-destructive" style={{ flex: health.broken }} />}
            </div>
            <p className="mt-1 font-mono text-[10px] text-muted-foreground/55">
              {health.ok} OK &middot; {health.partial} partial &middot; {health.broken} broken
            </p>
          </div>
        ) : (
          sub && <p className="mt-0.5 font-mono text-[10px] text-muted-foreground/55">{sub}</p>
        )}
      </div>
    </div>
  );
}
