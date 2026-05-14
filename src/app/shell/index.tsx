import { useState, useMemo, useEffect, useRef, lazy, Suspense } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getEventTransport } from "@/lib/events";
import {
  browseSessions,
  searchSessions,
  getSystemStatus,
  softDeleteSessions,
  softDeleteProject,
  destructiveDeleteSessions,
  getDeleteJob,
  getDeletePlan,
} from "@/lib/api";
import type { SessionRecord, SourceDeletePlan } from "@/lib/api";
import { useDebouncedValue, useLocalStorage } from "@/lib/hooks";
import { useZoom } from "./use-zoom";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";
import SessionRow from "@/pages/sessions/session-row";
import { PageToolbar } from "@/components/ui/page-toolbar";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
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
import { SidebarProvider, SidebarInset } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/app-sidebar";
import { SiteHeader } from "@/components/site-header";
import { Check, Minus, Trash2, ChevronRight } from "lucide-react";
import { SESSION_PAGE_SIZE } from "@/lib/constants";
import { useGroupedSessions } from "./use-grouped-sessions";
import { useSessionSelection } from "./use-session-selection";
import { useKeyboardNavigation } from "./use-keyboard-navigation";

const DashboardPage = lazy(() => import("@/pages/dashboard/dashboard-page"));
const SettingsPage = lazy(() => import("@/pages/system/settings-page"));
const MarketplacePage = lazy(() => import("@/pages/marketplace/marketplace-page"));
const ProxyPage = lazy(() => import("@/pages/proxy/proxy-page"));
const SessionDetailPane = lazy(() => import("@/pages/sessions/session-detail-pane"));

export function AppShell() {
  const [section, setSection] = useState<"dashboard" | "sessions" | "marketplace" | "settings" | "proxy">("dashboard");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [agentFilter, setAgentFilter] = useLocalStorage("agent-filter", "claude_code");
  const queryClient = useQueryClient();
  useZoom();

  const { data: status } = useQuery({
    queryKey: ["system-status"],
    queryFn: getSystemStatus,
    refetchInterval: 30_000,
  });

  useEffect(() => {
    const transport = getEventTransport();
    const unlistenCompleted = transport.on("sync-completed", () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["session-detail"] });
      queryClient.invalidateQueries({ queryKey: ["session-preview"] });
      queryClient.invalidateQueries({ queryKey: ["session-transcript"] });
      queryClient.invalidateQueries({ queryKey: ["system-status"] });
      queryClient.invalidateQueries({ queryKey: ["action-log"] });
    });
    return () => {
      unlistenCompleted.then((fn) => fn());
    };
  }, [queryClient]);

  return (
    <div className="app-shell">
      <div className="app-overlay" />
      <SidebarProvider
        className="!min-h-0 h-full"
        style={{
          "--sidebar-width": "184px",
          "--sidebar-width-icon": "48px",
          "--header-height": "32px",
        } as React.CSSProperties}
      >
        <AppSidebar section={section} onSectionChange={setSection} totalSessions={status?.total_sessions} />
        <SidebarInset>
          <SiteHeader section={section} agentFilter={agentFilter} onAgentFilterChange={setAgentFilter} />
          <main data-ai-page={section} className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <Suspense fallback={<PanelFallback />}>
              {section === "dashboard" && <DashboardPage />}
              {section === "sessions" && (
                <SessionsPage selectedId={selectedId} onSelect={setSelectedId} agentFilter={agentFilter} />
              )}
              {section === "marketplace" && <MarketplacePage />}
              {section === "settings" && <SettingsPage />}
              {section === "proxy" && <ProxyPage />}
            </Suspense>
          </main>
        </SidebarInset>
      </SidebarProvider>
    </div>
  );
}

function DeletePlanTable({
  sessionIds,
  sessions,
}: {
  sessionIds: Set<string>;
  sessions: SessionRecord[];
}) {
  const { t } = useTranslation();
  const [plans, setPlans] = useState<Map<string, SourceDeletePlan[]>>(new Map());
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const ids = Array.from(sessionIds);
    setLoading(true);
    Promise.all(ids.map((id) => getDeletePlan(id)))
      .then((results) => {
        if (cancelled) return;
        const map = new Map<string, SourceDeletePlan[]>();
        results.forEach((plan) => {
          if (plan.sources.length > 0) {
            map.set(plan.session_id, plan.sources);
          }
        });
        setPlans(map);
      })
      .catch(() => {
        if (cancelled) return;
        setPlans(new Map());
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [sessionIds]);

  const allFiles = useMemo(() => {
    const files: { path: string; sessionId: string; sessionTitle: string }[] = [];
    const titleMap = new Map(sessions.map((s) => [s.id, s.title || s.id.slice(0, 12)]));
    plans.forEach((sources, sessionId) => {
      for (const s of sources) {
        files.push({ path: s.target_path, sessionId, sessionTitle: titleMap.get(sessionId) || sessionId.slice(0, 8) });
      }
    });
    return files;
  }, [plans, sessions]);

  if (loading) {
    return (
      <div className="flex items-center justify-center rounded-md border border-border bg-secondary/40 px-3 py-6">
        <span className="text-[13px] text-muted-foreground">{t("manage.loadingPlan")}</span>
      </div>
    );
  }

  if (allFiles.length === 0) return null;

  return (
    <div className="max-h-64 overflow-y-auto rounded-md border border-border">
      <table className="w-full text-left text-[12px]">
        <thead className="sticky top-0 bg-secondary/80 backdrop-blur-sm">
          <tr>
            <th className="px-3 py-1.5 font-medium text-muted-foreground">{t("manage.columnFile")}</th>
            <th className="px-3 py-1.5 font-medium text-muted-foreground">{t("manage.columnSession")}</th>
          </tr>
        </thead>
        <tbody>
          {allFiles.map((f, i) => (
            <tr key={i} className="border-t border-border">
              <td className="max-w-[280px] truncate px-3 py-1 font-mono text-foreground">{f.path}</td>
              <td className="max-w-[120px] truncate px-3 py-1 text-muted-foreground">{f.sessionTitle}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function SessionsPage({
  selectedId,
  onSelect,
  agentFilter,
}: {
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  agentFilter: string;
}) {
  const queryClient = useQueryClient();
  const [searchRaw, setSearchRaw] = useState("");
  const search = useDebouncedValue(searchRaw, 250);
  const [sortDesc] = useLocalStorage("sort-desc", true);
  const [collapsedProjects, setCollapsedProjects] = useLocalStorage<
    Record<string, boolean>
  >("collapsed-projects", {});
  const [ctxMenu, setCtxMenu] = useState<
    { x: number; y: number; sessionId: string; projectPath?: string } | null
  >(null);
  const ctxRef = useRef<HTMLDivElement>(null);
  const [deleteMode, _setDeleteMode] = useState<"soft" | "destructive">("soft");
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [_deleteProgress, setDeleteProgress] = useState<{ processed: number; total: number } | null>(null);
  const [_deleteError, setDeleteError] = useState<string | null>(null);
  const { t } = useTranslation();

  const isSearching = search.trim().length > 0;
  const sort = sortDesc ? "updated_at" : "updated_at_asc";

  const browseQuery = useQuery({
    queryKey: ["sessions", "browse", { sort, agent: agentFilter }],
    queryFn: () =>
      browseSessions({
        sort,
        limit: SESSION_PAGE_SIZE,
        agent: agentFilter,
      }),
    enabled: !isSearching,
    refetchInterval: 30_000,
    staleTime: 10_000,
  });

  const searchQuery = useQuery({
    queryKey: ["sessions", "search", { query: search }],
    queryFn: () =>
      searchSessions({
        query: search,
        limit: SESSION_PAGE_SIZE,
      }),
    enabled: isSearching,
    refetchInterval: 30_000,
    staleTime: 10_000,
  });

  const { data, isLoading, error } = isSearching ? searchQuery : browseQuery;
  const sessions = useMemo(() => data?.sessions ?? [], [data?.sessions]);

  const grouped = useGroupedSessions(sessions, isSearching);
  const {
    manageMode, setManageMode, selectedIds,
    toggleSession, toggleProject, exitManageMode, allSelected,
    toggleAll, flatSessionIds,
  } = useSessionSelection(sessions, grouped, collapsedProjects, selectedId, onSelect);
  const { showHelp, setShowHelp, searchRef } = useKeyboardNavigation(flatSessionIds, selectedId, onSelect);

  const deleteBatch = useMutation({
    mutationFn: (ids: string[]) => softDeleteSessions(ids),
    onSuccess: () => {
      setDeleteDialogOpen(false);
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      exitManageMode();
    },
    onError: (err) => {
      console.error("Soft delete failed:", err);
      setDeleteError(err instanceof Error ? err.message : String(err));
      setDeleteDialogOpen(false);
    },
  });

  const [deleteJobId, setDeleteJobId] = useState<string | null>(null);

  const destructiveBatch = useMutation({
    mutationFn: (ids: string[]) => destructiveDeleteSessions(ids),
    onSuccess: (data) => {
      setDeleteDialogOpen(false);
      setDeleteJobId(data.job_id);
      setDeleteProgress({ processed: 0, total: selectedIds.size });
    },
    onError: (err) => {
      console.error("Destructive delete failed:", err);
      setDeleteError(err instanceof Error ? err.message : String(err));
      setDeleteDialogOpen(false);
    },
  });

  // Poll delete job progress
  useEffect(() => {
    if (!deleteJobId) return;
    const interval = setInterval(async () => {
      try {
        const job = await getDeleteJob(deleteJobId);
        setDeleteProgress((prev) => prev ? { ...prev, processed: job.processed, total: job.total } : prev);
        if (job.status === "completed" || job.status === "failed") {
          setDeleteJobId(null);
          setDeleteProgress(null);
          queryClient.invalidateQueries({ queryKey: ["sessions"] });
          queryClient.invalidateQueries({ queryKey: ["system-status"] });
          exitManageMode();
        }
      } catch {
        // job not found or other error — stop polling
        setDeleteJobId(null);
        setDeleteProgress(null);
      }
    }, 500);
    return () => clearInterval(interval);
  }, [deleteJobId, queryClient, exitManageMode]);

  const ctxDelete = useMutation({
    mutationFn: () => {
      if (!ctxMenu) throw new Error("no context menu");
      if (ctxMenu.projectPath) return softDeleteProject(ctxMenu.projectPath);
      return softDeleteSessions([ctxMenu.sessionId]);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      setCtxMenu(null);
    },
  });

  // Close context menu on outside click
  useEffect(() => {
    if (!ctxMenu) return;
    const handler = (e: MouseEvent) => {
      if (ctxRef.current && !ctxRef.current.contains(e.target as Node)) {
        setCtxMenu(null);
      }
    };
    window.addEventListener("mousedown", handler);
    return () => window.removeEventListener("mousedown", handler);
  }, [ctxMenu]);

  const toggleCollapse = (key: string) => {
    setCollapsedProjects((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Search + Manage */}
      <PageToolbar region="sessions-toolbar">
        <div className="flex min-w-0 flex-1 flex-col gap-2 lg:flex-row lg:items-center">
          <label className="block flex-1 lg:max-w-xl">
            <span className="sr-only">{t("sessions.searchSrLabel")}</span>
            <input
              ref={searchRef}
              value={searchRaw}
              onChange={(e) => setSearchRaw(e.target.value)}
              placeholder={t("sessions.searchPlaceholder")}
              className="zed-input"
            />
          </label>
          <div className="flex items-center gap-1.5">
            {!manageMode ? (
              <Button
                variant="outline"
                size="sm"
                className="h-8 rounded-md px-3 text-[12px]"
                onClick={() => setManageMode(true)}
              >
                {t("sessions.manage")}
              </Button>
            ) : (
              <>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 rounded-md px-3 text-[12px]"
                  onClick={exitManageMode}
                >
                  {t("manage.cancel")}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 rounded-md px-3 text-[12px]"
                  onClick={toggleAll}
                >
                  {allSelected ? t("manage.cancel") : t("sessions.selectAll")}
                </Button>
                {selectedIds.size > 0 && (
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-8 rounded-md px-3 text-[12px]"
                    onClick={() => setDeleteDialogOpen(true)}
                  >
                    <Trash2 size={14} />
                    {t("manage.deleteSelected")}
                  </Button>
                )}
              </>
            )}
          </div>
        </div>
      </PageToolbar>

      <div className="grid min-h-0 flex-1 xl:grid-cols-[360px_minmax(0,1fr)] [grid-template-rows:1fr]">
      <section data-ai-region="sessions-list" className="flex min-h-0 flex-1 flex-col overflow-hidden border-r border-border">
        <ScrollArea className="min-h-0 flex-1">
          {error ? (
            <div className="flex h-72 items-center justify-center px-6">
              <div className="max-w-sm text-center">
                <p className="text-xl text-foreground">
                  {t("sessions.loadError")}
                </p>
                <p className="mt-2 text-sm text-muted-foreground">{String(error)}</p>
              </div>
            </div>
          ) : isLoading ? (
            <div className="space-y-1 p-2">
              {Array.from({ length: 8 }).map((_, i) => (
                <Skeleton key={i} className="h-18 w-full rounded-md" />
              ))}
            </div>
          ) : sessions.length === 0 ? (
            <div className="flex h-72 items-center justify-center px-6">
              <div className="max-w-sm text-center">
                <div className="mx-auto mb-3 size-10 rounded-sm border border-border bg-secondary" />
                <p className="text-[16px] font-medium text-foreground">
                  {isSearching ? t("sessions.emptySearch") : t("sessions.emptyBrowse")}
                </p>
                <p className="mt-2 text-[14px] leading-[1.5] text-muted-foreground">
                  {isSearching
                    ? t("sessions.emptySearchHint")
                    : t("sessions.emptyBrowseHint")}
                </p>
              </div>
            </div>
          ) : (
            <div className="space-y-2.5 p-2.5">
              {grouped.map((g) => {
                const collapsed = collapsedProjects[g.key];
                return (
                  <Collapsible
                    key={g.key}
                    open={!collapsed}
                    onOpenChange={() => toggleCollapse(g.key)}
                    className="space-y-1"
                  >
                    <div className="flex items-center gap-1">
                      {manageMode && (() => {
                        const selCount = g.sessions.filter((s) => selectedIds.has(s.id)).length;
                        const allSel = selCount === g.sessions.length;
                        const someSel = selCount > 0 && !allSel;
                        return (
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon-xs"
                            onClick={() => toggleProject(g.sessions)}
                            className={`flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-sm border-2 transition ${
                              allSel
                                ? "border-primary bg-primary"
                                : someSel
                                  ? "border-primary bg-primary/20"
                                  : "border-muted-foreground/40 hover:border-primary"
                            }`}
                          >
                            {allSel && (
                              <Check size={16} color="white" />
                            )}
                            {someSel && !allSel && (
                              <Minus size={16} className="text-primary" />
                            )}
                          </Button>
                        );
                      })()}
                      <CollapsibleTrigger
                        render={<Button type="button" variant="ghost" size="sm" className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-border bg-secondary/40 px-2.5 py-2 text-left transition-colors hover:bg-accent" />}
                        onContextMenu={(e) => {
                          if (isSearching) return;
                          e.preventDefault();
                          setCtxMenu({ x: e.clientX, y: e.clientY, sessionId: "", projectPath: g.key });
                        }}
                      >
                        <ChevronRight size={14} className={`shrink-0 text-muted-foreground transition-transform duration-200 ${collapsed ? "" : "rotate-90"}`} />
                        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-foreground">
                          {g.label}
                        </span>
                        <span className="shrink-0 rounded-sm border border-border bg-[var(--editor)] px-1.5 py-0.5 font-mono text-[12px] text-muted-foreground">
                          {g.sessions.length}
                        </span>
                      </CollapsibleTrigger>
                    </div>

                    <CollapsibleContent className="space-y-1">
                      {g.sessions.map((session) => (
                        <SessionRow
                          key={session.id}
                          session={session}
                          isSelected={selectedId === session.id}
                          onSelect={() =>
                            onSelect(selectedId === session.id ? null : session.id)
                          }
                          manageMode={manageMode}
                          checked={selectedIds.has(session.id)}
                          onCheck={() => toggleSession(session.id)}
                          onContextMenu={(e, id) => {
                            setCtxMenu({ x: e.clientX, y: e.clientY, sessionId: id });
                          }}
                        />
                      ))}
                    </CollapsibleContent>
                  </Collapsible>
                );
              })}
            </div>
          )}
        </ScrollArea>
      </section>

      <section data-ai-region="sessions-detail" className="min-h-0 flex-1 overflow-hidden">
        {selectedId ? (
          <Suspense fallback={<PanelFallback />}>
            <SessionDetailPane sessionId={selectedId} />
          </Suspense>
        ) : (
          <div className="flex h-full items-center justify-center p-6">
            <div className="max-w-xl rounded-xl border border-border bg-card p-5">
              <p className="zed-kicker">{t("sessions.selectPrompt")}</p>
              <h3 className="mt-2 max-w-md text-[14px] font-semibold leading-none text-foreground">
                {t("sessions.selectHeading")}
              </h3>
              <p className="mt-2 max-w-lg text-[14px] leading-[1.5] text-muted-foreground">
                {t("sessions.selectDescription")}
              </p>
            </div>
          </div>
        )}
      </section>
      </div>

      {ctxMenu && (
        <div
          ref={ctxRef}
          className="fixed z-50 min-w-[120px] rounded-md border border-border bg-card p-1 shadow-lg"
          style={{ left: ctxMenu.x, top: ctxMenu.y }}
        >
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-[13px] text-destructive hover:bg-accent"
            onClick={() => ctxDelete.mutate()}
          >
            <Trash2 size={16} />
            {ctxMenu.projectPath ? t("contextMenu.deleteProject") : t("contextMenu.delete")}
          </Button>
        </div>
      )}

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent className="sm:max-w-[90vw]">
          <AlertDialogHeader>
            <AlertDialogTitle>{t("manage.deleteConfirm", { count: selectedIds.size })}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("manage.deleteWarning")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <DeletePlanTable sessionIds={selectedIds} sessions={sessions} />
          <AlertDialogFooter>
            <AlertDialogCancel>{t("manage.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleteBatch.isPending || destructiveBatch.isPending}
              onClick={() => {
                const ids = Array.from(selectedIds);
                if (deleteMode === "destructive") {
                  destructiveBatch.mutate(ids);
                } else {
                  deleteBatch.mutate(ids);
                }
              }}
            >
              {deleteBatch.isPending || destructiveBatch.isPending ? t("manage.deleting") : t("manage.confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {showHelp && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => setShowHelp(false)}
        >
          <div
            className="surface-panel w-full max-w-md p-3"
            onClick={(e) => e.stopPropagation()}
          >
            <p className="zed-kicker">{t("shortcuts.keyboard")}</p>
            <h3 className="mt-2 text-[14px] font-medium leading-none text-foreground">{t("shortcuts.title")}</h3>
            <div className="mt-3 grid grid-cols-2 gap-x-4 gap-y-2 text-[14px]">
              <Shortcut keys={["j", "↓"]} desc={t("shortcuts.next")} />
              <Shortcut keys={["k", "↑"]} desc={t("shortcuts.previous")} />
              <Shortcut keys={["/"]} desc={t("shortcuts.focusSearch")} />
              <Shortcut keys={["Esc"]} desc={t("shortcuts.closeSelection")} />
              <Shortcut keys={["?"]} desc={t("shortcuts.toggleHelp")} />
              <Shortcut keys={["⌘", "="]} desc={t("shortcuts.zoomIn")} />
              <Shortcut keys={["⌘", "-"]} desc={t("shortcuts.zoomOut")} />
              <Shortcut keys={["⌘", "0"]} desc={t("shortcuts.zoomReset")} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Shortcut({ keys, desc }: { keys: string[]; desc: string }) {
  return (
    <>
      <span className="text-muted-foreground">{desc}</span>
      <span className="rounded-sm border border-border bg-secondary px-2 py-0.5 font-mono text-[12px] text-muted-foreground">
        {keys.join(" / ")}
      </span>
    </>
  );
}

function PanelFallback() {
  return (
    <div className="space-y-3 p-4">
      <Skeleton className="h-36 w-full rounded-xl" />
      <Skeleton className="h-72 w-full rounded-xl" />
    </div>
  );
}
