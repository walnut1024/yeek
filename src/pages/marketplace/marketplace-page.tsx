import { useState, useCallback, useEffect, useMemo } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getEventTransport } from "@/lib/events";
import { Trash2, RefreshCcw, SquarePlus, ChevronRight, CircleCheck, CircleAlert, CircleDot, AlertTriangle } from "lucide-react";
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
  type MarketplaceEntry,
  type MarketplacePlugin,
  type PluginInfo,
  type SkillInfo,
} from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

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
const HEALTH_ICONS: Record<string, { Icon: typeof CircleCheck; className: string }> = {
  ok: { Icon: CircleCheck, className: "text-chart-2" },
  partial: { Icon: CircleAlert, className: "text-chart-3" },
  hook: { Icon: CircleDot, className: "text-chart-5" },
  broken: { Icon: AlertTriangle, className: "text-destructive" },
};

function HealthBadge({ health }: { health: string }) {
  const { Icon, className } = HEALTH_ICONS[health] ?? HEALTH_ICONS.hook;
  return <Icon size={14} className={`shrink-0 ${className}`} />;
}

export default function MarketplacePage({ agentFilter }: { agentFilter: string }) {
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

  const { t } = useTranslation();
  const queryClient = useQueryClient();

  // Fetch marketplaces
  const { data: mktData, isLoading: mktLoading } = useQuery({
    queryKey: ["marketplaces"],
    queryFn: listMarketplaces,
  });

  // Fetch installed plugins for cross-reference
  const { data: pluginsData } = useQuery({
    queryKey: ["plugins", "global"],
    queryFn: () => listPlugins("global"),
  });

  // Map tab value to plugin.agent field
  const agentMatch = useMemo(() => {
    const map: Record<string, string> = { "claude-code": "claude_code", codex: "codex", opencode: "opencode" };
    return map[agentFilter] ?? agentFilter;
  }, [agentFilter]);

  const filteredPlugins = useMemo(
    () => (pluginsData?.plugins ?? []).filter((p) => p.agent === agentMatch),
    [pluginsData, agentMatch],
  );

  // Build lookup: pluginName@marketplaceName -> PluginInfo
  const pluginByKey = useMemo(() => {
    const map = new Map<string, PluginInfo>();
    for (const p of filteredPlugins) {
      map.set(p.key, p);
    }
    return map;
  }, [filteredPlugins]);

  // Count installed plugins per marketplace
  const installedPerMarketplace = useMemo(() => {
    const counts = new Map<string, number>();
    for (const p of filteredPlugins) {
      if (p.marketplace?.name) {
        counts.set(p.marketplace.name, (counts.get(p.marketplace.name) ?? 0) + 1);
      }
    }
    return counts;
  }, [filteredPlugins]);

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
      {/* Content */}
      {mktLoading ? (
        <div className="space-y-1 p-2">
          {Array.from({ length: 4 }).map((_, i) => (<Skeleton key={i} className="h-16 w-full rounded-md" />))}
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <div className="proxy-config-cards">
            <div className="proxy-panel-head">
              <span className="zed-kicker">{t("marketplace.sectionMarketplaces")}</span>
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="font-mono text-[12px] text-muted-foreground">{t("marketplace.registries", { count: marketplaces.length })}</span>
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
                      agentFilter={agentMatch}
                      pluginByKey={pluginByKey}
                      onToggle={toggleMut.mutate}
                      onUninstall={setUninstallTarget}
                      onClean={setCleanTarget}
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
          <label className="flex items-center gap-2 text-[14px] text-foreground">
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
              <span className="mb-2 block rounded-sm border border-border bg-secondary p-2 font-mono text-[12px] text-muted-foreground break-all">
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

    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

const AGENT_TARGET_LABELS: Record<string, string> = {
  claude_code: "Claude",
  codex: "Codex",
  opencode: "OpenCode",
};

const MARKETPLACE_STATUS_CLASSES: Record<string, string> = {
  current: "border-chart-2/30 bg-chart-2/10 text-chart-2",
  update_available: "border-chart-3/30 bg-chart-3/10 text-chart-3",
  stale: "border-chart-5/30 bg-chart-5/10 text-chart-5",
  check_failed: "border-destructive/30 bg-destructive/10 text-destructive",
  clone_failed: "border-destructive/30 bg-destructive/10 text-destructive",
  clone_missing: "border-chart-5/30 bg-chart-5/10 text-chart-5",
  fetch_failed: "border-destructive/30 bg-destructive/10 text-destructive",
  never_checked: "border-border bg-secondary text-muted-foreground",
  unknown: "border-border bg-secondary text-muted-foreground",
};

function MarketplaceStatusBadge({ marketplace }: { marketplace: MarketplaceEntry }) {
  const status = marketplace.sync_status || "unknown";
  const className = MARKETPLACE_STATUS_CLASSES[status] ?? MARKETPLACE_STATUS_CLASSES.unknown;
  const showUpdateCount = status === "update_available" && marketplace.updates_available > 0;
  return (
    <span className={`rounded-sm border px-1.5 py-0.5 text-[10px] font-medium ${className}`} title={marketplace.check_error}>
      {status.replaceAll("_", " ")}
      {showUpdateCount ? ` · ${marketplace.updates_available}` : ""}
    </span>
  );
}

function AgentTargetsBadge({ targets }: { targets: string[] }) {
  if (!targets.length) return null;
  return (
    <span className="flex shrink-0 items-center gap-0.5">
      {targets.map((t) => (
        <span key={t} className="rounded-sm bg-secondary px-1 py-0.5 text-[10px] font-medium text-muted-foreground">
          {AGENT_TARGET_LABELS[t] ?? t}
        </span>
      ))}
    </span>
  );
}

function SkillRow({ skill }: { skill: SkillInfo }) {
  return (
    <div className="flex items-start gap-2 border-b border-border px-2.5 py-1.5 transition-colors hover:bg-element-hover">
      <Badge variant="outline"
        className={`mt-0.5 grid size-4 shrink-0 place-items-center px-0 py-0 text-[10px] font-medium ${
          skill.skill_type === "agent" ? "text-chart-3 border-chart-3/30" : "text-foreground border-foreground/30"
        }`}>
        {skill.skill_type === "agent" ? "A" : "S"}
      </Badge>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-[12px] font-medium text-foreground">{skill.name}</span>
          {skill.tools && <span className="zed-chip shrink-0 font-mono text-[10px]">{skill.tools}</span>}
        </div>
        {skill.description && <p className="mt-0.5 text-[12px] leading-[1.4] text-muted-foreground line-clamp-2">{skill.description}</p>}
      </div>
      <span className={`mt-1 size-1.5 shrink-0 rounded-full ${skill.health === "ok" ? "bg-chart-2" : "bg-chart-3"}`} />
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
  agentFilter,
  pluginByKey,
  onToggle,
  onUninstall,
  onClean,
}: {
  marketplace: MarketplaceEntry;
  installedCount: number;
  isUpdating: boolean;
  expanded: boolean;
  onToggleExpand: () => void;
  onUpdate: () => void;
  onRemove: () => void;
  agentFilter: string;
  pluginByKey: Map<string, PluginInfo>;
  onToggle: (key: string) => void;
  onUninstall: (p: PluginInfo) => void;
  onClean: (p: PluginInfo) => void;
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
    return plugins
      .filter((mp) => mp.agent_targets.includes(agentFilter))
      .map((mp): { mp: MarketplacePlugin; info: PluginInfo | null } => {
        const key = `${mp.name}@${marketplace.name}`;
        return { mp, info: pluginByKey.get(key) ?? null };
      });
  }, [plugins, marketplace.name, pluginByKey, agentFilter]);

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
          <MarketplaceStatusBadge marketplace={marketplace} />
          <span className="toml-card-title">{marketplace.name}</span>
          <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground">{marketplace.repo}</span>
          {marketplace.remote_head && <span className="shrink-0 font-mono text-[10px] text-muted-foreground">remote {marketplace.remote_head}</span>}
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
            <div className="grid grid-cols-1 gap-2 lg:grid-cols-3">
              {sorted.map(({ mp, info }) => {
                if (info) {
                  return (
                    <PluginSubCard
                      key={mp.name}
                      plugin={info}
                      onToggle={() => onToggle(info.key)}
                      onUninstall={() => onUninstall(info)}
                      onClean={() => onClean(info)}
                    />
                  );
                }
                return (
                  <div key={mp.name} className="rounded-md border border-border px-2 py-1.5 text-[12px]">
                    <div className="flex items-center gap-1.5">
                      <span className="shrink-0 font-semibold text-foreground">{mp.name}</span>
                      <AgentTargetsBadge targets={mp.agent_targets} />
                      <span className="min-w-0 flex-1" />
                      <Button variant="outline" size="sm" className="h-5 shrink-0 rounded-md px-1.5 text-[10px]" disabled={installMut.isPending} onClick={() => installMut.mutate(mp.name)}>
                        {installMut.isPending ? t("marketplace.installing") : t("marketplace.install")}
                      </Button>
                    </div>
                    <p className="mt-1 text-muted-foreground leading-snug">{mp.description}</p>
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
function PluginSubCard({ plugin, onToggle, onUninstall, onClean }: {
  plugin: PluginInfo;
  onToggle: () => void;
  onUninstall: () => void;
  onClean: () => void;
}) {
  const [expanded, setExpanded] = useState(true);
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
        <HealthBadge health={plugin.health} />
        <span className={`truncate text-[12px] font-medium text-foreground ${!isEnabled ? "opacity-50" : ""}`}>{plugin.name}</span>
        <span className="font-mono text-[10px] text-muted-foreground">v{plugin.version}</span>
        <span className="min-w-0 flex-1" />
        <div onClick={(e) => e.stopPropagation()}>
          <Switch size="sm" checked={isEnabled} onCheckedChange={onToggle} />
        </div>
        {isBroken ? (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            className="text-destructive hover:bg-destructive/10 hover:text-destructive"
            onClick={(e) => { e.stopPropagation(); onClean(); }}
            aria-label={t("skills.clean")}
          >
            <Trash2 size={16} />
          </Button>
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
                <div key={i} className={`flex items-center gap-1 text-[12px] ${plugin.health === "broken" ? "text-destructive" : "text-chart-3"}`}>
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
