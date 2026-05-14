import { useState, useCallback, useEffect, useMemo } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getEventTransport } from "@/lib/events";
import { Trash2, RefreshCcw, SquarePlus, ChevronRight } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import {
  listMarketplaces,
  addMarketplace,
  updateMarketplace,
  removeMarketplace,
  listMarketplacePlugins,
  installMarketplacePlugin,
  listPlugins,
  togglePlugin,
  uninstallPlugin,
  cleanPlugin,
  reinstallPlugin,
  type MarketplaceEntry,
  type MarketplacePlugin,
  type PluginInfo,
  type SkillInfo,
} from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

import { PageToolbar } from "@/components/ui/page-toolbar";
import { ScrollArea } from "@/components/ui/scroll-area";
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
} from "@/components/ui/alert-dialog";

// Health color mapping
const HEALTH_COLORS: Record<string, { dot: string; text: string; bg: string; border: string }> = {
  ok: { dot: "bg-chart-2", text: "text-chart-2", bg: "bg-chart-2/15", border: "border-chart-2/30" },
  partial: { dot: "bg-chart-3", text: "text-chart-3", bg: "bg-chart-3/15", border: "border-chart-3/30" },
  hook: { dot: "bg-chart-5", text: "text-chart-5", bg: "bg-chart-5/15", border: "border-chart-5/30" },
  broken: { dot: "bg-destructive", text: "text-destructive", bg: "bg-destructive/15", border: "border-destructive/30" },
};

const HEALTH_LABELS: Record<string, string> = { ok: "OK", partial: "PARTIAL", hook: "HOOK", broken: "BROKEN" };

const AGENTS = [
  { key: "claude-code", label: "Claude Code" },
  { key: "codex", label: "Codex" },
] as const;
type AgentKey = (typeof AGENTS)[number]["key"];

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export default function MarketplacePage() {
  const [agent, setAgent] = useState<AgentKey>("claude-code");
  const [addOpen, setAddOpen] = useState(false);
  const [addName, setAddName] = useState("");
  const [addRepo, setAddRepo] = useState("");
  const [removeTarget, setRemoveTarget] = useState<MarketplaceEntry | null>(null);
  const [removePlugins, setRemovePlugins] = useState(false);
  const [updatingNames, setUpdatingNames] = useState<Set<string>>(new Set());
  const [expandedName, setExpandedName] = useState<string | null>(null);

  // Plugin management state
  const [uninstallTarget, setUninstallTarget] = useState<PluginInfo | null>(null);
  const [cleanTarget, setCleanTarget] = useState<PluginInfo | null>(null);
  const [reinstallTarget, setReinstallTarget] = useState<PluginInfo | null>(null);
  const [reinstallError, setReinstallError] = useState<string | null>(null);

  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const isClaudeCode = agent === "claude-code";

  // Fetch marketplaces
  const { data: mktData, isLoading: mktLoading } = useQuery({
    queryKey: ["marketplaces"],
    queryFn: listMarketplaces,
    enabled: isClaudeCode,
  });

  // Fetch installed plugins for cross-reference
  const { data: pluginsData } = useQuery({
    queryKey: ["plugins", "global"],
    queryFn: () => listPlugins("global"),
    enabled: isClaudeCode,
  });

  // Build lookup: pluginName@marketplaceName -> PluginInfo
  const pluginByKey = useMemo(() => {
    const map = new Map<string, PluginInfo>();
    for (const p of pluginsData?.plugins ?? []) {
      map.set(p.key, p);
    }
    return map;
  }, [pluginsData]);

  // Count installed plugins per marketplace
  const installedPerMarketplace = useMemo(() => {
    const counts = new Map<string, number>();
    for (const p of pluginsData?.plugins ?? []) {
      if (p.marketplace?.name) {
        counts.set(p.marketplace.name, (counts.get(p.marketplace.name) ?? 0) + 1);
      }
    }
    return counts;
  }, [pluginsData]);

  // SSE listener
  useEffect(() => {
    const transport = getEventTransport();
    const unlisten = transport.on("plugin-config-changed", () => {
      queryClient.invalidateQueries({ queryKey: ["marketplace-plugins"] });
      queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
    });
    return () => { unlisten.then(f => f()); };
  }, [queryClient]);

  const marketplaces = useMemo(() => mktData?.marketplaces ?? [], [mktData?.marketplaces]);
  const isUpdatingAny = updatingNames.size > 0;

  // Mutations
  const toggleMut = useMutation({
    mutationFn: (key: string) => togglePlugin(key),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["plugins"] }),
  });

  const uninstallMut = useMutation({
    mutationFn: (key: string) => uninstallPlugin(key),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["marketplace-plugins"] });
      setUninstallTarget(null);
    },
  });

  const cleanMut = useMutation({
    mutationFn: (key: string) => cleanPlugin(key),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["marketplace-plugins"] });
      setCleanTarget(null);
    },
  });

  const reinstallMut = useMutation({
    mutationFn: (key: string) => reinstallPlugin(key),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["marketplace-plugins"] });
      setReinstallTarget(null);
      setReinstallError(null);
    },
    onError: (err: unknown) => setReinstallError(errorMessage(err)),
  });

  const handleUpdateOne = useCallback(async (name: string) => {
    setUpdatingNames((prev) => new Set(prev).add(name));
    try {
      await updateMarketplace(name);
      queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
    } catch (err: unknown) {
      console.error("Update failed:", name, err);
    } finally {
      setUpdatingNames((prev) => { const n = new Set(prev); n.delete(name); return n; });
    }
  }, [queryClient]);

  const handleUpdateAll = useCallback(async () => {
    for (const m of marketplaces) await handleUpdateOne(m.name);
  }, [marketplaces, handleUpdateOne]);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Toolbar: agent tabs */}
      <PageToolbar region="marketplace-toolbar">
        <div className="segmented-control">
          {AGENTS.map((a) => (
            <Button key={a.key} type="button" variant="secondary" size="sm" onClick={() => setAgent(a.key)}
              className={`segmented-control-item ${agent === a.key ? "segmented-control-item-active" : ""}`}>
              {a.label}
            </Button>
          ))}
        </div>
      </PageToolbar>

      {/* Content */}
      {!isClaudeCode ? (
        <div className="flex h-72 items-center justify-center px-6">
          <div className="max-w-sm text-center">
            <p className="text-[16px] font-medium text-foreground">{AGENTS.find(a => a.key === agent)?.label}</p>
            <p className="mt-2 text-[14px] leading-[1.5] text-muted-foreground">{t("marketplace.pluginSupportSoon")}</p>
          </div>
        </div>
      ) : mktLoading ? (
        <div className="space-y-1 p-2">
          {Array.from({ length: 4 }).map((_, i) => (<Skeleton key={i} className="h-16 w-full rounded-md" />))}
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <div className="proxy-config-cards">
            <div className="proxy-panel-head">
              <span className="zed-kicker">{t("marketplace.sectionMarketplaces")}</span>
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="font-mono text-[11px] text-muted-foreground">{t("marketplace.registries", { count: marketplaces.length })}</span>
                {marketplaces.length > 0 && (
                  <Button type="button" variant="outline" size="sm" disabled={isUpdatingAny} onClick={handleUpdateAll}>
                    <RefreshCcw size={16} className={isUpdatingAny ? "animate-spin" : ""} />
                    {t("marketplace.updateAll")}
                  </Button>
                )}
                <Button type="button" variant="primary" size="sm" onClick={() => setAddOpen(true)}>
                  <SquarePlus size={16} />
                  {t("marketplace.add")}
                </Button>
              </div>
            </div>

            <div className="toml-card-groups">
              <section className="toml-group">
                {marketplaces.length === 0 ? (
                  <div className="px-3 py-4 text-center text-[12px] text-muted-foreground">{t("marketplace.empty")}</div>
                ) : (
                  marketplaces.map((m) => (
                    <MarketplaceCard
                      key={m.name}
                      marketplace={m}
                      installedCount={installedPerMarketplace.get(m.name) ?? 0}
                      isUpdating={updatingNames.has(m.name)}
                      expanded={expandedName === m.name}
                      onToggleExpand={() => setExpandedName(expandedName === m.name ? null : m.name)}
                      onUpdate={() => handleUpdateOne(m.name)}
                      onRemove={() => { setRemovePlugins(false); setRemoveTarget(m); }}
                      pluginByKey={pluginByKey}
                      onToggle={toggleMut.mutate}
                      onUninstall={setUninstallTarget}
                      onClean={setCleanTarget}
                      onReinstall={(p) => { setReinstallTarget(p); setReinstallError(null); }}
                    />
                  ))
                )}
              </section>
            </div>

          </div>
        </ScrollArea>
      )}

      {/* Add dialog */}
      <AlertDialog open={addOpen} onOpenChange={setAddOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("marketplace.addTitle")}</AlertDialogTitle>
            <AlertDialogDescription>{t("marketplace.addDesc")}</AlertDialogDescription>
          </AlertDialogHeader>
          <div className="space-y-2">
            <input className="zed-input w-full" placeholder={t("marketplace.addNamePlaceholder")}
              value={addName} onChange={(e) => setAddName(e.target.value)} />
            <input className="zed-input w-full" placeholder={t("marketplace.addRepoPlaceholder")}
              value={addRepo} onChange={(e) => setAddRepo(e.target.value)} />
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction disabled={!addName.trim() || !addRepo.trim()}
              onClick={async () => {
                await addMarketplace(addName.trim(), addRepo.trim());
                queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
                setAddOpen(false); setAddName(""); setAddRepo("");
              }}>
              {t("marketplace.add")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Remove marketplace dialog */}
      <AlertDialog open={!!removeTarget} onOpenChange={(open) => !open && setRemoveTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("marketplace.removeTitle", { name: removeTarget?.name })}</AlertDialogTitle>
            <AlertDialogDescription>{t("marketplace.removeDesc")}</AlertDialogDescription>
          </AlertDialogHeader>
          <label className="flex items-center gap-2 text-[13px] text-foreground">
            <input type="checkbox" checked={removePlugins} onChange={(e) => setRemovePlugins(e.target.checked)} className="rounded border-border" />
            {t("marketplace.removePlugins")}
          </label>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={async () => {
                if (removeTarget) {
                  await removeMarketplace(removeTarget.name, removePlugins);
                  queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
                  queryClient.invalidateQueries({ queryKey: ["plugins"] });
                  setRemoveTarget(null);
                }
              }}
              className="border-destructive/30 bg-destructive/10 text-destructive hover:bg-destructive/20">
              {t("marketplace.remove")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Uninstall dialog */}
      <AlertDialog open={!!uninstallTarget} onOpenChange={(open) => !open && setUninstallTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("skills.uninstallTitle", { name: uninstallTarget?.name })}</AlertDialogTitle>
            <AlertDialogDescription>
              <span className="mb-2 block rounded-sm border border-border bg-secondary p-2 font-mono text-[11px] text-muted-foreground break-all">
                {uninstallTarget?.install_path}
              </span>
              {t("skills.uninstallDesc")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction disabled={uninstallMut.isPending}
              onClick={() => uninstallTarget && uninstallMut.mutate(uninstallTarget.key)}
              className="border-destructive/30 bg-destructive/10 text-destructive hover:bg-destructive/20">
              {uninstallMut.isPending ? t("detail.deleting") : t("skills.uninstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Clean dialog */}
      <AlertDialog open={!!cleanTarget} onOpenChange={(open) => !open && setCleanTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("skills.cleanTitle", { name: cleanTarget?.name })}</AlertDialogTitle>
            <AlertDialogDescription>{t("skills.cleanDesc")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction disabled={cleanMut.isPending}
              onClick={() => cleanTarget && cleanMut.mutate(cleanTarget.key)}
              className="border-destructive/30 bg-destructive/10 text-destructive hover:bg-destructive/20">
              {cleanMut.isPending ? "..." : t("skills.clean")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Reinstall dialog */}
      <AlertDialog open={!!reinstallTarget}
        onOpenChange={(open) => { if (!open) { setReinstallTarget(null); setReinstallError(null); } }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("skills.reinstallTitle", { name: reinstallTarget?.name })}</AlertDialogTitle>
            <AlertDialogDescription>{t("skills.reinstallDesc")}</AlertDialogDescription>
          </AlertDialogHeader>
          {reinstallError && (
            <div className="rounded-sm border border-destructive/30 bg-destructive/10 p-2 text-[12px] text-destructive">
              {t("skills.reinstallError", { error: reinstallError })}
            </div>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction disabled={reinstallMut.isPending}
              onClick={() => reinstallTarget && reinstallMut.mutate(reinstallTarget.key)}
              className="border-chart-3/30 bg-chart-3/10 text-chart-3 hover:bg-chart-3/20">
              {reinstallMut.isPending ? "..." : t("skills.reinstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function HealthBadge({ health }: { health: string }) {
  const hc = HEALTH_COLORS[health] ?? HEALTH_COLORS.hook;
  return (
    <span className={`flex shrink-0 items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-[0.05em] ${hc.text} ${hc.bg} ${hc.border}`}>
      <span className={`size-1 rounded-full ${hc.dot}`} />
      {HEALTH_LABELS[health] ?? health.toUpperCase()}
    </span>
  );
}

function SkillRow({ skill }: { skill: SkillInfo }) {
  return (
    <div className="flex items-center gap-1.5 border-b border-border px-2.5 py-1 transition-colors hover:bg-element-hover">
      <Badge variant="outline"
        className={`grid size-4 shrink-0 place-items-center px-0 py-0 text-[9px] font-medium ${
          skill.skill_type === "agent" ? "text-chart-3 border-chart-3/30" : "text-foreground border-foreground/30"
        }`}>
        {skill.skill_type === "agent" ? "A" : "S"}
      </Badge>
      <span className="truncate text-[12px] text-foreground">{skill.name}</span>
      <span className="min-w-0 flex-1 truncate text-[11px] text-muted-foreground">{skill.description}</span>
      {skill.tools && <span className="zed-chip font-mono text-[10px]">{skill.tools}</span>}
      <span className={`size-1.5 shrink-0 rounded-full ${skill.health === "ok" ? "bg-chart-2" : "bg-chart-3"}`} />
    </div>
  );
}

/** Marketplace card with expandable plugin list */
function MarketplaceCard({
  marketplace,
  installedCount,
  isUpdating,
  expanded,
  onToggleExpand,
  onUpdate,
  onRemove,
  pluginByKey,
  onToggle,
  onUninstall,
  onClean,
  onReinstall,
}: {
  marketplace: MarketplaceEntry;
  installedCount: number;
  isUpdating: boolean;
  expanded: boolean;
  onToggleExpand: () => void;
  onUpdate: () => void;
  onRemove: () => void;
  pluginByKey: Map<string, PluginInfo>;
  onToggle: (key: string) => void;
  onUninstall: (p: PluginInfo) => void;
  onClean: (p: PluginInfo) => void;
  onReinstall: (p: PluginInfo) => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data: plugins, isLoading: pluginsLoading } = useQuery({
    queryKey: ["marketplace-plugins", marketplace.name],
    queryFn: () => listMarketplacePlugins(marketplace.name),
    enabled: expanded,
  });

  const installMut = useMutation({
    mutationFn: (pluginName: string) => installMarketplacePlugin(marketplace.name, pluginName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["marketplace-plugins", marketplace.name] });
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
    },
  });

  const resolved = useMemo(() => {
    if (!plugins) return [];
    return plugins.map((mp): { mp: MarketplacePlugin; info: PluginInfo | null } => {
      const key = `${mp.name}@${marketplace.name}`;
      return { mp, info: pluginByKey.get(key) ?? null };
    });
  }, [plugins, marketplace.name, pluginByKey]);

  const sorted = useMemo(() =>
    [...resolved].sort((a, b) => {
      if (a.info && !b.info) return -1;
      if (!a.info && b.info) return 1;
      return a.mp.name.localeCompare(b.mp.name);
    }),
  [resolved]);

  return (
    <article className={`toml-card ${expanded ? "is-active" : ""}`}>
      <div className="toml-card-head cursor-pointer" onClick={onToggleExpand}>
        <div className="flex min-w-0 items-center gap-2">
          <span className="toml-card-title">{marketplace.name}</span>
          <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground">{marketplace.repo}</span>
          {marketplace.last_updated && <span className="shrink-0 text-[10px] text-muted-foreground">{marketplace.last_updated.split("T")[0]}</span>}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <span className="text-[10px] text-muted-foreground">{t("marketplace.installedCount", { installed: installedCount, total: marketplace.plugin_count })}</span>
          <Button
            type="button"
            variant="outline"
            size="xs"
            disabled={isUpdating}
            onClick={(e) => { e.stopPropagation(); onUpdate(); }}
          >
            <RefreshCcw size={16} className={isUpdating ? "animate-spin" : ""} />
            {t("marketplace.update")}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="xs"
            className="text-destructive hover:border-destructive/30 hover:bg-destructive/10 hover:text-destructive"
            onClick={(e) => { e.stopPropagation(); onRemove(); }}
          >
            <Trash2 size={16} />
            {t("marketplace.remove")}
          </Button>
        </div>
      </div>
      {expanded && (
        <div className="toml-card-body">
          {pluginsLoading ? (
            <div className="space-y-1">
              {Array.from({ length: 3 }).map((_, i) => (<Skeleton key={i} className="h-7 w-full rounded-sm" />))}
            </div>
          ) : sorted.length > 0 ? (
            <div className="grid grid-cols-1 gap-2 xl:grid-cols-2">
              {sorted.map(({ mp, info }) => {
                if (info) {
                  return (
                    <PluginSubCard
                      key={mp.name}
                      plugin={info}
                      onToggle={() => onToggle(info.key)}
                      onUninstall={() => onUninstall(info)}
                      onClean={() => onClean(info)}
                      onReinstall={() => onReinstall(info)}
                    />
                  );
                }
                return (
                  <div key={mp.name} className="flex flex-wrap items-center gap-1.5 rounded-md border border-border px-2 py-1.5 text-[11px]">
                    <span className="truncate text-foreground">{mp.name}</span>
                    <span className="min-w-0 flex-1 truncate text-muted-foreground">{mp.description}</span>
                    <Button variant="outline" size="sm" className="h-5 shrink-0 rounded-md px-1.5 text-[10px]" disabled={installMut.isPending} onClick={() => installMut.mutate(mp.name)}>
                      {installMut.isPending ? t("marketplace.installing") : t("marketplace.install")}
                    </Button>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="text-[12px] text-muted-foreground">{t("marketplace.noPlugins")}</div>
          )}
        </div>
      )}
      {isUpdating && <div className="indeterminate-bar" />}
    </article>
  );
}

/** Plugin sub-card inside marketplace card body */
function PluginSubCard({ plugin, onToggle, onUninstall, onClean, onReinstall }: {
  plugin: PluginInfo;
  onToggle: () => void;
  onUninstall: () => void;
  onClean: () => void;
  onReinstall: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation();
  const isEnabled = plugin.enabled;
  const isBroken = plugin.health === "broken";

  return (
    <div className="overflow-hidden rounded-md border border-border">
      <div className="flex items-center gap-1.5 bg-card px-2.5 py-1.5 transition-colors hover:bg-element-hover">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="size-5 shrink-0"
          onClick={() => setExpanded(!expanded)}
          aria-label={expanded ? t("marketplace.collapseDetails") : t("marketplace.expandDetails")}
        >
          <ChevronRight size={14} className={`transition-transform ${expanded ? "rotate-90" : ""}`} />
        </Button>
        <span className={`truncate text-[11px] font-medium text-foreground ${!isEnabled ? "opacity-50" : ""}`}>{plugin.name}</span>
        <span className="font-mono text-[10px] text-muted-foreground">v{plugin.version}</span>
        <span className="min-w-0 flex-1" />
        <HealthBadge health={plugin.health} />
        <div onClick={(e) => e.stopPropagation()}>
          <Switch size="sm" checked={isEnabled} onCheckedChange={onToggle} />
        </div>
        {isBroken ? (
          <>
            <Button variant="outline" size="sm" className="h-5 rounded-md px-1.5 text-[10px] text-muted-foreground hover:border-destructive hover:text-destructive hover:bg-destructive/10" onClick={(e) => { e.stopPropagation(); onClean(); }}>{t("skills.clean")}</Button>
            <Button variant="outline" size="sm" className="h-5 rounded-md px-1.5 text-[10px] text-muted-foreground hover:border-chart-3 hover:text-chart-3 hover:bg-chart-3/10" onClick={(e) => { e.stopPropagation(); onReinstall(); }}>{t("skills.reinstall")}</Button>
          </>
        ) : (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            className="text-destructive hover:bg-destructive/10 hover:text-destructive"
            onClick={(e) => { e.stopPropagation(); onUninstall(); }}
            aria-label={t("skills.uninstall")}
          >
            <Trash2 size={16} />
          </Button>
        )}
      </div>
      {expanded && (
        <div className="border-t border-border bg-card">
          {plugin.health_issues.length > 0 && (
            <div className="border-b border-border px-2.5 py-1.5">
              {plugin.health_issues.map((issue, i) => (
                <div key={i} className={`flex items-center gap-1 text-[11px] ${plugin.health === "broken" ? "text-destructive" : "text-chart-3"}`}>
                  <span className={`size-1 rounded-full ${plugin.health === "broken" ? "bg-destructive" : "bg-chart-3"}`} />
                  {issue}
                </div>
              ))}
            </div>
          )}
          {plugin.skills.map((s) => <SkillRow key={s.name} skill={s} />)}
          {plugin.agents.map((a) => <SkillRow key={a.name} skill={a} />)}
        </div>
      )}
    </div>
  );
}
