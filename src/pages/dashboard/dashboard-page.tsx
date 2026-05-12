import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSystemStatus, getActionLog, getProxyMetrics, listPlugins, getProxyErrorEvents } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { UpdateBanner } from "@/components/update-banner";
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
  const [showErrorSheet, setShowErrorSheet] = useState(false);

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

  const { data: errorEvents } = useQuery({
    queryKey: ["proxy-error-events"],
    queryFn: getProxyErrorEvents,
    enabled: showErrorSheet,
    refetchInterval: showErrorSheet ? 5000 : false,
  });

  const allActions = actionLog?.actions ?? [];
  const visibleActions = showAllActions ? allActions : allActions.slice(0, VISIBLE_ACTIONS);
  const hiddenCount = allActions.length - VISIBLE_ACTIONS;

  const pluginHealth = plugins?.health_summary;

  return (
    <div className="flex h-full overflow-hidden">
      {/* Main content */}
      <div className="flex h-full flex-col overflow-hidden flex-1 min-w-0">

        {/* Header */}
        <header data-ai-region="dashboard-header" className="flex flex-col gap-2 border-b border-border px-3 pb-3">
          <h2 className="text-[14px] font-medium leading-none text-foreground">{t("dashboard.title")}</h2>
          <p className="mt-2 max-w-2xl text-[14px] leading-[1.5] text-muted-foreground">{t("dashboard.description")}</p>
        </header>

        {/* Update notification banner */}
        <UpdateBanner />

        <div className="min-h-0 flex-1 overflow-auto p-3">
        {/* Row 1: Hero Stats */}
        <section data-ai-region="dashboard-stats" className="grid grid-cols-4 gap-3">
          <StatCard label={t("dashboard.statSessions")} value={String(status?.total_sessions ?? "-")} sub={t("dashboard.statSessionsSub")} />
          <StatCard label={t("dashboard.statSources")} value={String(status?.total_sources ?? "-")} sub={t("dashboard.statSourcesSub")} />
          <StatCard label={t("dashboard.statProjects")} value={String(status?.total_projects ?? "-")} sub={t("dashboard.statProjectsSub")} />
          <StatCard label={t("dashboard.statMessages")} value={status?.total_messages != null ? formatCount(status.total_messages) : "-"} sub={t("dashboard.statMessagesSub")} accent />
        </section>

        {/* Row 2: Proxy Runtime */}
        {metrics && (
          <section data-ai-region="dashboard-metrics">
            <p className="zed-kicker mt-7 mb-2">{t("dashboard.proxyRuntime")}</p>
            <div className="grid grid-cols-6 gap-3">
              <MetricCard label={t("dashboard.metricUptime")} value={`${Math.floor(metrics.uptime_secs / 60)}m`} sub={`${metrics.uptime_secs}s`} />
              <MetricCard label={t("dashboard.metricRPS")} value={metrics.rps.toFixed(1)} sub="req/s" />
              <MetricCard label={t("dashboard.metricLatency")} value={`${metrics.avg_latency_ms.toFixed(0)}ms`} sub="avg" />
              <MetricCard label={t("dashboard.metricActive")} value={String(metrics.active_connections)} sub="connections" />
              <MetricCard label={t("dashboard.metricRequests")} value={String(metrics.request_count)} sub="total" />
              <MetricCard label={t("dashboard.metricErrors")} value={String(metrics.error_count)} sub={`${((metrics.error_count / Math.max(metrics.request_count, 1)) * 100).toFixed(2)}%`} danger={metrics.error_count > 0} onClick={() => setShowErrorSheet(!showErrorSheet)} />
            </div>
          </section>
        )}

        {/* Row 3: System Health */}
        <section data-ai-region="dashboard-health">
          <p className="zed-kicker mt-7 mb-2">{t("dashboard.systemHealth")}</p>
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

        {/* Activity Timeline (merged with alerts) */}
        <section data-ai-region="dashboard-activity">
        <p className="zed-kicker mt-7 mb-2">{t("dashboard.activity")}</p>
        {allActions.length === 0 ? (
          <div className="border border-border bg-secondary px-2.5 py-2 text-[13px] text-muted-foreground">
            {t("dashboard.noActions")}
          </div>
        ) : (
          <div>
            {visibleActions.map((a, i) => {
              const isError = a.detail?.includes("errors=") && !a.detail.includes("errors=0");
              const isDestructive = a.action.includes("delete") || a.action.includes("destructive");
              const isSync = a.action.includes("sync") || a.action.includes("release");
              const dotColor = isError ? "bg-destructive" : isDestructive ? "bg-destructive" : isSync ? "bg-primary" : "bg-muted-foreground";
              const isLast = i === visibleActions.length - 1 && hiddenCount <= 0;
              return (
                <div key={a.id} className="flex gap-3">
                  <div className="flex flex-col items-center w-3 shrink-0">
                    <span className={`mt-1 size-2 shrink-0 rounded-full ${dotColor}`} />
                    {!isLast && <div className="w-px flex-1 bg-border" />}
                  </div>
                  <div className={`flex-1 min-w-0 ${isLast ? "pb-0" : "pb-3"}`}>
                    <div className="flex items-center gap-2">
                      <span className={`truncate font-mono text-[11px] font-medium ${isError ? "text-destructive" : "text-foreground/50"}`}>
                        {a.action}
                      </span>
                      <span className="ml-auto shrink-0 font-mono text-[11px] text-muted-foreground">
                        {a.created_at ? new Date(a.created_at).toLocaleTimeString(getCurrentLocale(), { hour: "2-digit", minute: "2-digit", hour12: false }) : ""}
                      </span>
                    </div>
                    <p className={`mt-0.5 text-[12px] leading-[1.4] truncate ${isError ? "text-destructive/70" : "text-foreground/60"}`}>
                      {a.detail || a.session_id?.slice(0, 8) || t("dashboard.noDetail")}
                    </p>
                  </div>
                </div>
              );
            })}
            {hiddenCount > 0 && (
              <Button
                variant="ghost" size="sm"
                className="mt-1 h-7 w-full pl-6 text-[12px] text-muted-foreground hover:text-foreground"
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

      {/* Error panel — inline slide-in */}
      <aside data-ai-region="dashboard-errors" className={`shrink-0 transition-[width] duration-200 ease-out overflow-hidden ${showErrorSheet ? "w-[420px] border-l border-border" : "w-0"}`}>
        <div className="w-[420px] h-full flex flex-col">
          <div className="flex items-center justify-between px-3 py-2 border-b border-border">
            <div>
              <h3 className="text-[14px] font-medium">Proxy Errors</h3>
              <p className="text-[12px] text-muted-foreground">Retains last 100 errors, cleared on proxy restart</p>
            </div>
            <Button variant="ghost" size="sm" className="h-6 w-6 p-0 text-[14px]" onClick={() => setShowErrorSheet(false)}>✕</Button>
          </div>
          <div className="flex-1 overflow-auto px-3 pt-2">
            {!errorEvents || errorEvents.length === 0 ? (
              <p className="py-8 text-center text-[13px] text-muted-foreground">{t("dashboard.noActions")}</p>
            ) : (
              errorEvents.map((e, i) => (
                <div key={i} className="border-b border-border py-2.5">
                  <div className="flex items-center gap-2">
                    <span className={`font-mono text-[11px] font-medium px-1.5 py-0.5 rounded ${e.status >= 500 ? "bg-destructive/10 text-destructive" : "bg-chart-3/10 text-chart-3"}`}>
                      {e.status}
                    </span>
                    <span className="font-mono text-[11px] text-foreground/60">{e.provider}</span>
                    <span className="font-mono text-[11px] text-muted-foreground truncate">{e.model}</span>
                    <span className="ml-auto shrink-0 font-mono text-[11px] text-muted-foreground">
                      {new Date(e.timestamp).toLocaleString(getCurrentLocale(), { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false }).replace(/\//g, "-")}
                    </span>
                  </div>
                  <p className="mt-1 text-[12px] text-foreground/60 leading-[1.4] break-all">{e.message}</p>
                </div>
              ))
            )}
          </div>
        </div>
      </aside>
    </div>
  );
}

/* ── Sub-components ── */

function StatCard({ label, value, sub, accent }: { label: string; value: string; sub: string; accent?: boolean }) {
  return (
    <article data-ai-item="stat-card" className={`flex flex-col border p-[18px_20px_16px] ${accent ? "border-primary/20 bg-[linear-gradient(135deg,rgba(94,106,210,0.16),rgba(20,21,22,1))]" : "bg-card border-border"}`}>
      <p className={`text-[12px] uppercase tracking-[0.06em] ${accent ? "text-primary" : "text-muted-foreground"}`}>{label}</p>
      <p className={`mt-2.5 font-mono text-[28px] font-medium leading-none tracking-[-0.03em] ${accent ? "text-primary" : "text-foreground"}`}>{value}</p>
      <p className="mt-auto pt-2 text-[12px] text-muted-foreground">{sub}</p>
    </article>
  );
}

function MetricCard({ label, value, sub, danger, onClick }: { label: string; value: string; sub: string; danger?: boolean; onClick?: () => void }) {
  return (
    <article
      data-ai-item="metric-card"
      className={`border border-border bg-card px-3 py-3.5 text-center ${onClick ? "cursor-pointer hover:bg-card/80 transition-colors" : ""}`}
      onClick={onClick}
    >
      <p className={`font-mono text-[18px] font-medium leading-none tracking-[-0.02em] ${danger ? "text-chart-3" : "text-foreground"}`}>{value}</p>
      <p className="mt-1.5 text-[11px] font-medium uppercase tracking-[0.06em] text-muted-foreground">{label}</p>
      <p className="mt-1 font-mono text-[11px] text-muted-foreground/50">{sub}</p>
    </article>
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
    <article data-ai-item="health-card" className="flex items-center gap-3 border border-border bg-card px-4 py-3.5">
      <div className={`flex size-[34px] shrink-0 items-center justify-center rounded-lg ${iconBg}`}>
        {iconEl}
      </div>
      <div className="min-w-0">
        <p className="font-mono text-[14px] font-medium leading-[1.1]">{value}</p>
        <p className="mt-0.5 text-[12px] text-muted-foreground">{label}</p>
        {health && total > 0 ? (
          <div className="mt-1">
            <div className="flex h-[3px] gap-0.5 overflow-hidden rounded-sm">
              {health.ok > 0 && <span className="h-full rounded-sm bg-emerald-500" style={{ flex: health.ok }} />}
              {health.partial > 0 && <span className="h-full rounded-sm bg-chart-3" style={{ flex: health.partial }} />}
              {health.hook > 0 && <span className="h-full rounded-sm bg-chart-5" style={{ flex: health.hook }} />}
              {health.broken > 0 && <span className="h-full rounded-sm bg-destructive" style={{ flex: health.broken }} />}
            </div>
            <p className="mt-1 font-mono text-[11px] text-muted-foreground/55">
              {health.ok} OK &middot; {health.partial} partial &middot; {health.broken} broken
            </p>
          </div>
        ) : (
          sub && <p className="mt-0.5 font-mono text-[11px] text-muted-foreground/55">{sub}</p>
        )}
      </div>
    </article>
  );
}
