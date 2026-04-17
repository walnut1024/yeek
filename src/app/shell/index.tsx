import { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import {
  browseSessions,
  searchSessions,
  getSystemStatus,
  softDeleteSessions,
  type SessionRecord,
} from "@/lib/api";
import { useDebouncedValue, useLocalStorage } from "@/lib/hooks";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";
import SessionRow from "@/pages/sessions/session-row";
import SessionDetailPane from "@/pages/sessions/session-detail-pane";
import SystemPage from "@/pages/system/system-page";
import { formatProjectLabel } from "@/lib/formatters";

export function AppShell() {
  const [section, setSection] = useState<"sessions" | "system">("sessions");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const { t, i18n } = useTranslation();

  const { data: status } = useQuery({
    queryKey: ["system-status"],
    queryFn: getSystemStatus,
    refetchInterval: 30_000,
  });

  useEffect(() => {
    const unlistenCompleted = listen("sync-completed", () => {
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
      <div className="relative z-10 flex h-screen flex-col overflow-hidden">
        <header className="flex h-11 items-center justify-between border-b border-border bg-background px-3">
          <div className="flex items-center gap-2.5">
            <span className="size-2 rounded-full bg-primary" />
            <span className="text-[14px] font-medium text-foreground">
              {t("app.title")}
            </span>
            <span className="zed-chip font-mono uppercase tracking-[0.08em] text-primary">
              {t("app.sessionBrowser")}
            </span>
          </div>
          <div className="flex items-center gap-2">
            {(["sessions", "system"] as const).map((s) => (
              <button
                type="button"
                key={s}
                onClick={() => setSection(s)}
                className={`rounded-md px-2.5 py-1.5 text-[13px] font-medium transition-colors ${
                  section === s
                    ? "border border-border bg-[var(--editor)] text-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground"
                }`}
              >
                {t(`nav.${s}`)}
              </button>
            ))}
            <span className="flex items-center gap-1.5 border border-border bg-secondary px-2 py-1">
              <span className="size-1.5 rounded-full bg-[#a1c181]" />
              <span className="font-mono text-[12px] text-muted-foreground">
                {status ? t("nav.sessionCount", { count: status.total_sessions }) : "..."}
              </span>
            </span>
            <button
              type="button"
              onClick={() => i18n.changeLanguage(i18n.language === "zh-CN" ? "en" : "zh-CN")}
              className="rounded-md px-2.5 py-1.5 text-[13px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              {i18n.language === "zh-CN" ? "EN" : "中"}
            </button>
          </div>
        </header>

        <main className="min-h-0 flex-1">
          {section === "sessions" && (
            <SessionsPage selectedId={selectedId} onSelect={setSelectedId} />
          )}
          {section === "system" && <SystemPage />}
        </main>
      </div>
    </div>
  );
}

function SessionsPage({
  selectedId,
  onSelect,
}: {
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  const queryClient = useQueryClient();
  const [searchRaw, setSearchRaw] = useState("");
  const search = useDebouncedValue(searchRaw, 250);
  const [sortDesc, setSortDesc] = useLocalStorage("sort-desc", true);
  const [collapsedProjects, setCollapsedProjects] = useLocalStorage<
    Record<string, boolean>
  >("collapsed-projects", {});
  const [manageMode, setManageMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [confirmDelete, setConfirmDelete] = useState(false);
  const { t } = useTranslation();

  const deleteBatch = useMutation({
    mutationFn: (ids: string[]) => softDeleteSessions(ids),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      exitManageMode();
    },
  });

  const toggleSession = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleProject = (sessions: SessionRecord[]) => {
    const ids = sessions.map((s) => s.id);
    const allSelected = ids.every((id) => selectedIds.has(id));
    setSelectedIds((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => (allSelected ? next.delete(id) : next.add(id)));
      return next;
    });
  };

  const exitManageMode = () => {
    setManageMode(false);
    setSelectedIds(new Set());
    setConfirmDelete(false);
  };

  const isSearching = search.trim().length > 0;
  const sort = sortDesc ? "updated_at" : "updated_at_asc";

  const browseQuery = useQuery({
    queryKey: ["sessions", "browse", { sort }],
    queryFn: () =>
      browseSessions({
        sort,
        limit: 200,
      }),
    enabled: !isSearching,
  });

  const searchQuery = useQuery({
    queryKey: ["sessions", "search", { query: search }],
    queryFn: () =>
      searchSessions({
        query: search,
        limit: 200,
      }),
    enabled: isSearching,
  });

  const { data, isLoading, error } = isSearching ? searchQuery : browseQuery;
  const sessions = data?.sessions ?? [];

  const grouped = useMemo(() => {
    const map = new Map<string, { label: string; sessions: SessionRecord[] }>();
    for (const s of sessions) {
      const key = isSearching ? "Results" : (s.project_path ?? t("sessions.noProject"));
      if (!map.has(key)) {
        map.set(key, {
          label: isSearching ? t("sessions.searchResults") : formatProjectLabel(s.project_path),
          sessions: [],
        });
      }
      map.get(key)?.sessions.push(s);
    }

    for (const group of map.values()) {
      group.sessions.sort((a, b) =>
        (b.updated_at ?? "").localeCompare(a.updated_at ?? "")
      );
    }

    return Array.from(map.entries())
      .sort((a, b) => {
        const aTime = a[1].sessions[0]?.updated_at ?? "";
        const bTime = b[1].sessions[0]?.updated_at ?? "";
        return bTime.localeCompare(aTime);
      })
      .map(([key, group]) => ({
        key,
        label: group.label,
        sessions: group.sessions,
      }));
  }, [sessions, isSearching]);

  const toggleCollapse = (key: string) => {
    setCollapsedProjects((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  const allSelected = sessions.length > 0 && sessions.every((s) => selectedIds.has(s.id));
  const someSelected = selectedIds.size > 0 && !allSelected;

  const toggleAll = () => {
    if (allSelected) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(sessions.map((s) => s.id)));
    }
  };

  const [showHelp, setShowHelp] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const hasAutoSelectedRef = useRef(false);

  const flatSessionIds = useMemo(() => {
    const ids: string[] = [];
    for (const g of grouped) {
      if (!collapsedProjects[g.key]) {
        for (const s of g.sessions) {
          ids.push(s.id);
        }
      }
    }
    return ids;
  }, [grouped, collapsedProjects]);

  const topSessionId = flatSessionIds[0] ?? grouped[0]?.sessions[0]?.id ?? null;

  useEffect(() => {
    if (!topSessionId) {
      hasAutoSelectedRef.current = false;
      return;
    }

    if (selectedId && sessions.some((session) => session.id === selectedId)) {
      hasAutoSelectedRef.current = true;
      return;
    }

    if (!hasAutoSelectedRef.current && selectedId === null) {
      hasAutoSelectedRef.current = true;
      onSelect(topSessionId);
      return;
    }

    if (selectedId && !sessions.some((session) => session.id === selectedId)) {
      onSelect(topSessionId);
    }
  }, [topSessionId, selectedId, sessions, onSelect]);

  const navigateList = useCallback(
    (direction: "up" | "down") => {
      if (flatSessionIds.length === 0) return;
      const idx = selectedId ? flatSessionIds.indexOf(selectedId) : -1;
      const next =
        direction === "down"
          ? Math.min(idx + 1, flatSessionIds.length - 1)
          : Math.max(idx - 1, 0);
      onSelect(flatSessionIds[next]);
    },
    [flatSessionIds, selectedId, onSelect]
  );

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      switch (e.key) {
        case "?":
          setShowHelp((v) => !v);
          break;
        case "/":
          e.preventDefault();
          searchRef.current?.focus();
          break;
        case "j":
        case "ArrowDown":
          navigateList("down");
          break;
        case "k":
        case "ArrowUp":
          navigateList("up");
          break;
        case "Escape":
          if (showHelp) setShowHelp(false);
          else onSelect(null);
          break;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigateList, showHelp, onSelect]);

  return (
    <div className="grid h-full min-h-0 xl:grid-cols-[360px_minmax(0,1fr)]">
      <section className="surface-panel flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="border-b border-border px-3 py-2">
          <div className="mb-2 flex items-start justify-between gap-3">
            <div>
              <p className="zed-kicker">{t("sessions.searchGroup")}</p>
              <h2 className="mt-1 text-[14px] font-medium leading-none text-foreground">{t("sessions.title")}</h2>
            </div>
            <div className="flex items-center gap-2">
              <div className="zed-chip px-2 py-1 text-right font-mono">
                {data ? t("sessions.total", { count: data.total }) : "..."}
              </div>
              <button
                type="button"
                onClick={() => setSortDesc(!sortDesc)}
                className={`rounded-md px-2.5 py-1.5 text-[13px] font-medium transition-colors ${
                  !sortDesc
                    ? "border border-border bg-[var(--editor)] text-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground"
                }`}
              >
                {sortDesc ? t("sessions.sortNewest") : t("sessions.sortOldest")}
              </button>
              <button
                type="button"
                onClick={() => {
                  if (manageMode) exitManageMode();
                  else setManageMode(true);
                }}
                className={`rounded-md px-2.5 py-1.5 text-[13px] font-medium transition-colors ${
                  manageMode
                    ? "border border-border bg-[var(--editor)] text-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground"
                }`}
              >
                {manageMode ? t("sessions.done") : t("sessions.manage")}
              </button>
            </div>
          </div>

          <label className="block">
            <span className="sr-only">{t("sessions.searchSrLabel")}</span>
            <input
              ref={searchRef}
              value={searchRaw}
              onChange={(e) => setSearchRaw(e.target.value)}
              placeholder={t("sessions.searchPlaceholder")}
              className="zed-input"
            />
          </label>
        </div>

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
            <div className="space-y-2 p-2">
              {manageMode && sessions.length > 0 && (
                <div
                  className="flex cursor-pointer items-center gap-2 rounded-md border border-border bg-secondary px-2.5 py-2 hover:bg-accent transition-colors"
                  onClick={toggleAll}
                >
                  <button
                    type="button"
                    className={`flex size-5 shrink-0 items-center justify-center rounded-sm border-2 transition ${
                      allSelected
                        ? "border-primary bg-primary"
                        : someSelected
                          ? "border-primary bg-primary/20"
                          : "border-muted-foreground/40"
                    }`}
                  >
                    {allSelected && (
                      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    )}
                    {someSelected && !allSelected && (
                      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" className="text-primary" strokeWidth="3.5" strokeLinecap="round">
                        <line x1="5" y1="12" x2="19" y2="12" />
                      </svg>
                    )}
                  </button>
                  <span className="text-[13px] text-muted-foreground">
                  <span
                    className="text-[13px] text-muted-foreground"
                    dangerouslySetInnerHTML={{ __html: t("sessions.selectAll", { count: sessions.length }) }}
                  />
                  </span>
                </div>
              )}
              {grouped.map((g) => {
                const collapsed = collapsedProjects[g.key];
                return (
                  <div key={g.key} className="space-y-1">
                    <div className="flex items-center gap-1">
                      {manageMode && (() => {
                        const selCount = g.sessions.filter((s) => selectedIds.has(s.id)).length;
                        const allSel = selCount === g.sessions.length;
                        const someSel = selCount > 0 && !allSel;
                        return (
                          <button
                            type="button"
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
                              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
                                <polyline points="20 6 9 17 4 12" />
                              </svg>
                            )}
                            {someSel && !allSel && (
                              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" className="text-primary" strokeWidth="3.5" strokeLinecap="round">
                                <line x1="5" y1="12" x2="19" y2="12" />
                              </svg>
                            )}
                          </button>
                        );
                      })()}
                      <button
                        type="button"
                        onClick={() => toggleCollapse(g.key)}
                        className="flex min-w-0 flex-1 items-center gap-2 rounded-md border border-border bg-secondary px-2.5 py-1.5 text-left transition-colors hover:bg-accent"
                      >
                        <span className="grid size-4 shrink-0 place-items-center rounded-sm bg-[var(--editor)] text-[10px] text-primary">
                          {collapsed ? "\u25B6" : "\u25BC"}
                        </span>
                        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-foreground">
                          {g.label}
                        </span>
                        <span className="shrink-0 rounded-sm border border-border bg-[var(--editor)] px-1.5 py-0.5 font-mono text-[12px] text-muted-foreground">
                          {g.sessions.length}
                        </span>
                      </button>
                    </div>

                    {!collapsed && (
                      <div className="space-y-1">
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
                          />
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </ScrollArea>

        {manageMode && (
          <div className="border-t border-border bg-card px-3 py-2">
            {confirmDelete ? (
              <div className="flex items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                  <span className="flex size-8 shrink-0 items-center justify-center rounded-sm bg-destructive/10">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" className="text-destructive" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
                      <line x1="12" y1="9" x2="12" y2="13" />
                      <line x1="12" y1="17" x2="12.01" y2="17" />
                    </svg>
                  </span>
                  <div>
                    <p className="text-[13px] font-medium text-foreground">
                      {t("manage.deleteConfirm", { count: selectedIds.size })}
                    </p>
                    <p className="text-[12px] text-muted-foreground">
                      {t("manage.deleteHint")}
                    </p>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  <Button variant="outline" size="sm" className="h-8 rounded-md px-3 text-[13px]" onClick={() => setConfirmDelete(false)}>
                    {t("manage.back")}
                  </Button>
                  <Button variant="destructive" size="sm" className="h-8 rounded-md px-3 text-[13px]" onClick={() => deleteBatch.mutate(Array.from(selectedIds))} disabled={deleteBatch.isPending}>
                    {deleteBatch.isPending ? t("manage.deleting") : t("manage.confirm")}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="flex items-center justify-between gap-4">
                <span
                  className="text-[13px] text-muted-foreground"
                  dangerouslySetInnerHTML={{ __html: t("manage.selected", { count: selectedIds.size }) }}
                />
                <div className="flex shrink-0 items-center gap-2">
                  <Button variant="outline" size="sm" className="h-8 rounded-md px-3 text-[13px]" onClick={exitManageMode}>
                    {t("manage.cancel")}
                  </Button>
                  <Button variant="destructive" size="sm" className="h-8 rounded-md px-3 text-[13px]" onClick={() => setConfirmDelete(true)} disabled={selectedIds.size === 0}>
                    {t("manage.delete")}
                  </Button>
                </div>
              </div>
            )}
          </div>
        )}
      </section>

      <section className="surface-panel min-h-0 flex-1 overflow-hidden">
        {selectedId ? (
          <SessionDetailPane sessionId={selectedId} />
        ) : (
          <div className="flex h-full items-center justify-center p-6">
            <div className="max-w-xl border border-border bg-card p-4">
              <p className="zed-kicker">{t("sessions.selectPrompt")}</p>
              <h3 className="mt-2 max-w-md text-[16px] font-medium leading-[1.2] text-foreground">
                {t("sessions.selectHeading")}
              </h3>
              <p className="mt-2 max-w-lg text-[14px] leading-[1.5] text-muted-foreground">
                {t("sessions.selectDescription")}
              </p>
            </div>
          </div>
        )}
      </section>

      {showHelp && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
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
