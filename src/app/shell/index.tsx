import { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  browseSessions,
  searchSessions,
  getSystemStatus,
  rescanSources,
  type SessionRecord,
} from "@/lib/api";
import { useDebouncedValue, useLocalStorage } from "@/lib/hooks";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";
import SessionRow from "@/pages/sessions/session-row";
import SessionDetailPane from "@/pages/sessions/session-detail-pane";
import MemoryPage from "@/pages/memory/memory-page";
import SystemPage from "@/pages/system/system-page";
import { formatProjectLabel } from "@/lib/formatters";
import type { SortMode, FilterState } from "./types";
import { DEFAULT_FILTERS } from "./types";

// --- App Shell ---

export function AppShell() {
  const [section, setSection] = useState<"sessions" | "memory" | "system">(
    "sessions"
  );
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const { data: status } = useQuery({
    queryKey: ["system-status"],
    queryFn: getSystemStatus,
    refetchInterval: 30_000,
  });

  const rescan = useMutation({
    mutationFn: rescanSources,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["system-status"] });
      queryClient.invalidateQueries({ queryKey: ["action-log"] });
    },
  });

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex h-11 shrink-0 items-center justify-between border-b border-border px-4">
        <div className="flex items-center gap-6">
          <h1 className="text-sm font-semibold tracking-tight">Yeek</h1>
          <nav className="flex gap-1">
            {(["sessions", "memory", "system"] as const).map((s) => (
              <button
                key={s}
                onClick={() => setSection(s)}
                className={`rounded-md px-3 py-1 text-xs font-medium capitalize transition-colors ${
                  section === s
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                }`}
              >
                {s}
              </button>
            ))}
          </nav>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-muted-foreground">
            {status ? `${status.total_sessions} sessions` : "..."}
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-xs"
            onClick={() => rescan.mutate()}
            disabled={rescan.isPending}
          >
            {rescan.isPending ? "Scanning..." : "Rescan"}
          </Button>
        </div>
      </header>

      <main className="flex-1 overflow-hidden">
        {section === "sessions" && (
          <SessionsPage
            selectedId={selectedId}
            onSelect={setSelectedId}
          />
        )}
        {section === "memory" && <MemoryPage />}
        {section === "system" && <SystemPage />}
      </main>
    </div>
  );
}

// --- Sessions Page ---

function SessionsPage({
  selectedId,
  onSelect,
}: {
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  const [searchRaw, setSearchRaw] = useState("");
  const search = useDebouncedValue(searchRaw, 250);
  const [sort, setSort] = useLocalStorage<SortMode>("sort", "updated_at");
  const [filters, setFilters] = useLocalStorage<FilterState>(
    "filters",
    DEFAULT_FILTERS
  );
  const [collapsedProjects, setCollapsedProjects] = useLocalStorage<
    Record<string, boolean>
  >("collapsed-projects", {});

  const isSearching = search.trim().length > 0;

  const browseQuery = useQuery({
    queryKey: ["sessions", "browse", { sort, filters }],
    queryFn: () =>
      browseSessions({
        sort,
        limit: 200,
        visibility: filters.visibility,
        agent: filters.agent ?? undefined,
        project_path: filters.project_path ?? undefined,
        pinned_only: filters.pinned_only,
      }),
    enabled: !isSearching,
  });

  const searchQuery = useQuery({
    queryKey: ["sessions", "search", { query: search, filters }],
    queryFn: () =>
      searchSessions({
        query: search,
        limit: 200,
        visibility: filters.visibility,
        agent: filters.agent ?? undefined,
      }),
    enabled: isSearching,
  });

  const { data, isLoading, error } = isSearching
    ? searchQuery
    : browseQuery;
  const sessions = data?.sessions ?? [];

  // Always group by project (for search results, group flat)
  const grouped = useMemo(() => {
    const map = new Map<string, { label: string; sessions: SessionRecord[] }>();
    for (const s of sessions) {
      const key = isSearching
        ? "Results"
        : (s.project_path ?? "No project");
      if (!map.has(key)) {
        const label = isSearching
          ? "Results"
          : formatProjectLabel(s.project_path);
        map.set(key, { label, sessions: [] });
      }
      map.get(key)!.sessions.push(s);
    }
    // Sort sessions within each group by updated_at descending
    for (const group of map.values()) {
      group.sessions.sort((a, b) =>
        (b.updated_at ?? "").localeCompare(a.updated_at ?? "")
      );
    }
    // Sort groups by most recent session (first after sort) descending
    const sorted = Array.from(map.entries()).sort((a, b) => {
      const aTime = a[1].sessions[0]?.updated_at ?? "";
      const bTime = b[1].sessions[0]?.updated_at ?? "";
      return bTime.localeCompare(aTime);
    });
    return sorted.map(([key, group]) => ({
      key,
      label: group.label,
      sessions: group.sessions,
    }));
  }, [sessions, isSearching]);

  const activeFilterCount = [
    filters.agent,
    filters.project_path,
    filters.visibility !== "visible",
    filters.pinned_only,
  ].filter(Boolean).length;

  const toggleCollapse = (key: string) => {
    setCollapsedProjects((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  // Keyboard shortcuts
  const [showHelp, setShowHelp] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  // Flat list of all visible session IDs for keyboard nav
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
      // Ignore if typing in input/textarea
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
    <div className="flex h-full">
      {/* Left: Session list grouped by project */}
      <div className="flex w-[380px] shrink-0 flex-col overflow-hidden border-r border-border">
        {/* Search + filters header */}
        <div className="shrink-0 space-y-1.5 border-b border-border px-3 py-2">
          <input
            ref={searchRef}
            value={searchRaw}
            onChange={(e) => setSearchRaw(e.target.value)}
            placeholder="Search sessions..."
            className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-xs outline-none focus:ring-1 focus:ring-ring"
          />
          <div className="flex flex-wrap items-center gap-1">
            {/* Visibility pills */}
            {(["visible", "hidden", "archived"] as const).map((v) => (
              <button
                key={v}
                onClick={() => setFilters((f) => ({ ...f, visibility: v }))}
                className={`rounded px-1.5 py-0 text-[10px] leading-5 ${
                  filters.visibility === v
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground hover:bg-accent"
                }`}
              >
                {v}
              </button>
            ))}
            <button
              onClick={() =>
                setFilters((f) => ({ ...f, pinned_only: !f.pinned_only }))
              }
              className={`rounded px-1.5 py-0 text-[10px] leading-5 ${
                filters.pinned_only
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:bg-accent"
              }`}
            >
              pinned
            </button>
            <button
              onClick={() => setSort(sort === "updated_at" ? "started_at" : "updated_at")}
              className="rounded bg-muted px-1.5 py-0 text-[10px] leading-5 text-muted-foreground hover:bg-accent"
            >
              {sort === "updated_at" ? "by updated" : "by started"}
            </button>
            {activeFilterCount > 0 && (
              <button
                className="text-[10px] text-muted-foreground hover:text-foreground"
                onClick={() => setFilters(DEFAULT_FILTERS)}
              >
                clear
              </button>
            )}
          </div>
        </div>

        {/* List */}
        <ScrollArea className="flex-1 min-h-0 overflow-hidden">
          {error ? (
            <div className="flex h-64 items-center justify-center">
              <p className="text-xs text-destructive">Error: {String(error)}</p>
            </div>
          ) : isLoading ? (
            <div className="space-y-1.5 p-3">
              {Array.from({ length: 10 }).map((_, i) => (
                <Skeleton key={i} className="h-14 w-full rounded" />
              ))}
            </div>
          ) : sessions.length === 0 ? (
            <div className="flex h-64 items-center justify-center">
              <div className="text-center">
                <p className="text-xs text-muted-foreground">
                  {isSearching ? "No results" : "No sessions"}
                </p>
                <p className="mt-1 text-[10px] text-muted-foreground/50">
                  {isSearching
                    ? "Try a different search"
                    : "Sessions appear after indexing"}
                </p>
              </div>
            </div>
          ) : (
            <div>
              {grouped.map((g) => {
                const collapsed = collapsedProjects[g.key];
                return (
                  <div key={g.key}>
                    {/* Project header */}
                    <button
                      onClick={() => toggleCollapse(g.key)}
                      className="flex w-full items-center gap-2 border-b border-border bg-muted/30 px-3 py-1.5 text-left hover:bg-muted/50"
                    >
                      <span className="text-[10px] text-muted-foreground/50">
                        {collapsed ? "\u25B6" : "\u25BC"}
                      </span>
                      <span className="text-[11px] font-semibold text-foreground truncate">
                        {g.label}
                      </span>
                      <span className="ml-auto shrink-0 text-[10px] text-muted-foreground/40">
                        {g.sessions.length}
                      </span>
                    </button>
                    {/* Sessions in project */}
                    {!collapsed && (
                      <div className="divide-y divide-border/50">
                        {g.sessions.map((session) => (
                          <SessionRow
                            key={session.id}
                            session={session}
                            isSelected={selectedId === session.id}
                            onSelect={() =>
                              onSelect(
                                selectedId === session.id ? null : session.id
                              )
                            }
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

        {/* Footer count */}
        <div className="shrink-0 border-t border-border px-3 py-1.5">
          <p className="text-[10px] text-muted-foreground/40">
            {data ? `${data.total} sessions` : ""}
          </p>
        </div>
      </div>

      {/* Right: Detail pane */}
      <div className="flex-1 overflow-hidden">
        {selectedId ? (
          <SessionDetailPane sessionId={selectedId} />
        ) : (
          <div className="flex h-full items-center justify-center">
            <div className="text-center">
              <p className="text-xs text-muted-foreground">No session selected</p>
              <p className="mt-1 text-[10px] text-muted-foreground/40">
                Select a session from the list to view details
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Help overlay */}
      {showHelp && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/30"
          onClick={() => setShowHelp(false)}
        >
          <div
            className="rounded-lg border border-border bg-popover p-5 text-popover-foreground shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="mb-3 text-sm font-semibold">Keyboard Shortcuts</h3>
            <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-xs">
              <Shortcut keys={["j", "↓"]} desc="Next session" />
              <Shortcut keys={["k", "↑"]} desc="Previous session" />
              <Shortcut keys={["/"]} desc="Focus search" />
              <Shortcut keys={["Esc"]} desc="Deselect / close" />
              <Shortcut keys={["?"]} desc="Toggle this help" />
            </div>
            <p className="mt-3 text-[10px] text-muted-foreground">
              Shortcuts work when not typing in a search field
            </p>
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
      <span className="font-mono text-muted-foreground/70">
        {keys.join(" / ")}
      </span>
    </>
  );
}
