import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSystemStatus, getActionLog, getProxyMetrics, listPlugins, listMarketplaces, getProxyErrorEvents } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { X } from "lucide-react";
import { UpdateBanner } from "@/components/update-banner";
import { formatTime, formatRelativeTime, formatDuration, getCurrentLocale } from "@/lib/formatters";

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

  const { data: mktData } = useQuery({
    queryKey: ["marketplaces"],
    queryFn: listMarketplaces,
  });
  const marketplaces = mktData?.marketplaces ?? [];

  const { data: errorEvents } = useQuery({
    queryKey: ["proxy-error-events"],
    queryFn: getProxyErrorEvents,
    enabled: showErrorSheet,
    refetchInterval: showErrorSheet ? 5000 : false,
  });

  const allActions = actionLog?.actions ?? [];
  const visibleActions = showAllActions ? allActions : allActions.slice(0, VISIBLE_ACTIONS);
  const hiddenCount = allActions.length - VISIBLE_ACTIONS;

  return (
    <div className="flex h-full overflow-hidden">
      {/* Main content */}
      <div className="flex h-full flex-col overflow-hidden flex-1 min-w-0">

        {/* Update notification banner */}
        <UpdateBanner />

        <div className="min-h-0 flex-1 overflow-auto p-3">
        {/* Overview */}
        <section data-ai-region="dashboard-overview" className="grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-4">
          <StatCard label={t("dashboard.statSessions")} value={String(status?.total_sessions ?? "-")} sub={t("dashboard.statAllTime")} />
          <StatCard label={t("dashboard.statMessages")} value={status?.total_messages != null ? formatCount(status.total_messages) : "-"} sub={t("dashboard.statProcessed")} accent />
          <StatCard label={t("dashboard.statActive")} value={String(status?.active_sessions ?? "-")} sub={status ? t("dashboard.statActiveSub", { complete: status.complete_sessions, partial: status.partial_sessions }) : "—"} />
          <StatCard label={t("dashboard.statLastSync")} value={status?.last_sync_at ? formatRelativeTime(status.last_sync_at) : t("dashboard.never")} sub={status?.last_sync_at ? formatTime(status.last_sync_at) : "—"} />
        </section>

        {/* Proxy */}
        {metrics && (
          <section data-ai-region="dashboard-proxy">
            <p className="zed-kicker mb-1.5 mt-5">{t("dashboard.proxyRuntime")}</p>
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 2xl:grid-cols-6">
              <MetricCard label={t("dashboard.metricUptime")} value={formatDuration(metrics.uptime_secs)} sub={`${metrics.uptime_secs}s`} />
              <MetricCard label={t("dashboard.metricRPS")} value={metrics.rps.toFixed(1)} sub={t("dashboard.metricReqSec")} />
              <MetricCard label={t("dashboard.metricLatency")} value={`${metrics.avg_latency_ms.toFixed(0)}ms`} sub={t("dashboard.metricAvg")} />
              <MetricCard label={t("dashboard.metricActive")} value={String(metrics.active_connections)} sub={t("dashboard.metricConnections")} />
              <MetricCard label={t("dashboard.metricRequests")} value={String(metrics.request_count)} sub={t("dashboard.metricTotal")} />
              <MetricCard label={t("dashboard.metricErrors")} value={String(metrics.error_count)} sub={`${((metrics.error_count / Math.max(metrics.request_count, 1)) * 100).toFixed(2)}%`} danger={metrics.error_count > 0} onClick={() => setShowErrorSheet(!showErrorSheet)} />
            </div>
          </section>
        )}

        {/* Session */}
        <section data-ai-region="dashboard-session">
          <p className="zed-kicker mb-1.5 mt-5">{t("dashboard.sectionSession")}</p>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-4">
            <StatCard label={t("dashboard.statSources")} value={String(status?.total_sources ?? "-")} sub={t("dashboard.statSourcesSub")} />
            <StatCard label={t("dashboard.statProjects")} value={String(status?.total_projects ?? "-")} sub={t("dashboard.statProjectsSub")} />
            <StatCard label={t("dashboard.statComplete")} value={String(status?.complete_sessions ?? "-")} sub={t("dashboard.statCompleteSub")} />
            <StatCard label={t("dashboard.statPartial")} value={String(status?.partial_sessions ?? "-")} sub={t("dashboard.statPartialSub")} />
          </div>
        </section>

        {/* Marketplace */}
        <section data-ai-region="dashboard-marketplace">
          <p className="zed-kicker mb-1.5 mt-5">{t("dashboard.sectionMarketplace")}</p>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-4">
            <StatCard label={t("dashboard.statRegistries")} value={String(marketplaces.length)}
              sub={marketplaces.length > 0
                ? `${marketplaces.filter(m => m.repo?.startsWith("http")).length} remote, ${marketplaces.filter(m => !m.repo?.startsWith("http")).length} local`
                : "—"} />
            <StatCard label={t("dashboard.statPlugins")} value={String(plugins?.plugins?.length ?? 0)}
              sub={t("dashboard.statInstalled", { count: plugins?.plugins?.length ?? 0 })} />
            <StatCard label={t("dashboard.statSkills")} value={String(plugins?.total_skills ?? 0)}
              sub={t("dashboard.statAvailableCommands")} />
            <StatCard label={t("dashboard.statBroken")} value={String(plugins?.health_summary?.broken ?? 0)}
              sub={(plugins?.health_summary?.broken ?? 0) > 0 ? t("dashboard.statNeedsCleanup") : t("dashboard.statAllHealthy")}
              danger={(plugins?.health_summary?.broken ?? 0) > 0} />
          </div>
        </section>

        {/* Activity Timeline (merged with alerts) */}
        <section data-ai-region="dashboard-activity">
        <p className="zed-kicker mb-1.5 mt-5">{t("dashboard.activity")}</p>
        {allActions.length === 0 ? (
          <div className="rounded-lg border border-border bg-secondary px-2.5 py-2 text-[12px] text-muted-foreground">
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
              <h3 className="text-[14px] font-medium">{t("dashboard.proxyErrors")}</h3>
              <p className="text-[12px] text-muted-foreground">{t("dashboard.proxyErrorsDesc")}</p>
            </div>
            <Button variant="ghost" size="sm" className="h-6 w-6 p-0" onClick={() => setShowErrorSheet(false)} aria-label={t("common.close", { defaultValue: "Close" })}><X size={14} /></Button>
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

function StatCard({ label, value, sub, accent, danger }: { label: string; value: string; sub: string; accent?: boolean; danger?: boolean }) {
  const c = accent ? "accent" : danger ? "danger" : "default";
  return (
    <article data-ai-item="stat-card" className={`metric-tile flex min-h-[112px] flex-col ${
      c === "accent" ? "border-primary/20 bg-[rgba(28,28,28,0.035)]" :
      c === "danger" ? "border-destructive/20 bg-destructive/5" :
      ""
    }`}>
      <p className={`text-[12px] uppercase tracking-[0.06em] ${
        c === "accent" ? "text-primary" : c === "danger" ? "text-destructive" : "text-muted-foreground"
      }`}>{label}</p>
      <p className={`mt-2 font-mono text-[24px] font-medium leading-none tracking-[-0.03em] ${
        c === "accent" ? "text-primary" : c === "danger" ? "text-destructive" : "text-foreground"
      }`}>{value}</p>
      <p className="mt-auto pt-1.5 text-[11px] text-muted-foreground">{sub}</p>
    </article>
  );
}

function MetricCard({ label, value, sub, danger, onClick }: { label: string; value: string; sub: string; danger?: boolean; onClick?: () => void }) {
  return (
    <article
      data-ai-item="metric-card"
      className={`metric-tile text-center ${onClick ? "cursor-pointer transition-colors hover:bg-element-hover" : ""}`}
      onClick={onClick}
    >
      <p className={`font-mono text-[17px] font-medium leading-none tracking-[-0.02em] ${danger ? "text-chart-3" : "text-foreground"}`}>{value}</p>
      <p className="mt-1 text-[10px] font-medium uppercase tracking-[0.06em] text-muted-foreground">{label}</p>
      <p className="mt-0.5 font-mono text-[10px] text-muted-foreground/60">{sub}</p>
    </article>
  );
}
