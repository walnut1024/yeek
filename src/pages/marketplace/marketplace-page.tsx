import { useState, useCallback, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getEventTransport } from "@/lib/events";
import {
  listMarketplaces,
  addMarketplace,
  updateMarketplace,
  removeMarketplace,
  listMarketplacePlugins,
  installMarketplacePlugin,
  type MarketplaceEntry,
} from "@/lib/api";
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

export default function MarketplacePage() {
  const [addOpen, setAddOpen] = useState(false);
  const [addName, setAddName] = useState("");
  const [addRepo, setAddRepo] = useState("");
  const [removeTarget, setRemoveTarget] = useState<MarketplaceEntry | null>(null);
  const [removePlugins, setRemovePlugins] = useState(false);
  const [updatingNames, setUpdatingNames] = useState<Set<string>>(new Set());
  const [updateAllActive, setUpdateAllActive] = useState(false);
  const [expandedName, setExpandedName] = useState<string | null>(null);
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ["marketplaces"],
    queryFn: listMarketplaces,
  });

  useEffect(() => {
    const transport = getEventTransport();
    const unlisten = transport.on("plugin-config-changed", () => {
      queryClient.invalidateQueries({ queryKey: ["marketplace-plugins"] });
      queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
    });
    return () => { unlisten.then(f => f()); };
  }, [queryClient]);

  const marketplaces = data?.marketplaces ?? [];
  const isUpdatingAny = updatingNames.size > 0;

  const handleUpdateOne = useCallback(async (name: string) => {
    setUpdatingNames((prev) => new Set(prev).add(name));
    try {
      await updateMarketplace(name);
      queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
    } catch (err: any) {
      console.error("Update failed:", name, err);
    } finally {
      setUpdatingNames((prev) => {
        const next = new Set(prev);
        next.delete(name);
        return next;
      });
    }
  }, [queryClient]);

  const handleUpdateAll = useCallback(async () => {
    setUpdateAllActive(true);
    for (const m of marketplaces) {
      await handleUpdateOne(m.name);
    }
    setUpdateAllActive(false);
  }, [marketplaces, handleUpdateOne]);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Toolbar */}
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <span className="text-[13px] font-medium text-foreground">
          {t("marketplace.title")}
          <span className="ml-2 text-[11px] text-muted-foreground">
            {marketplaces.length}
          </span>
        </span>
        <div className="flex items-center gap-1.5">
          {marketplaces.length > 0 && (
            <Button
              variant="outline"
              size="sm"
              className="h-7 rounded-md px-2.5 text-[13px]"
              onClick={handleUpdateAll}
              disabled={isUpdatingAny}
            >
              {updateAllActive ? t("marketplace.updating") : t("marketplace.updateAll")}
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            className="h-7 rounded-md px-2.5 text-[13px]"
            onClick={() => setAddOpen(true)}
          >
            {t("marketplace.add")}
          </Button>
        </div>
      </div>

      {/* Content */}
      <ScrollArea className="min-h-0 flex-1">
        {isLoading ? (
          <div className="space-y-1 p-2">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-16 w-full rounded-md" />
            ))}
          </div>
        ) : marketplaces.length === 0 ? (
          <div className="flex h-72 items-center justify-center px-6">
            <p className="text-[14px] text-muted-foreground">{t("marketplace.empty")}</p>
          </div>
        ) : (
          <div className="space-y-1 p-2">
            {marketplaces.map((m) => (
              <MarketplaceRow
                key={m.name}
                marketplace={m}
                isUpdating={updatingNames.has(m.name)}
                expanded={expandedName === m.name}
                onToggleExpand={() => setExpandedName(expandedName === m.name ? null : m.name)}
                onUpdate={() => handleUpdateOne(m.name)}
                onRemove={() => {
                  setRemovePlugins(false);
                  setRemoveTarget(m);
                }}
              />
            ))}
          </div>
        )}
      </ScrollArea>

      {/* Add dialog */}
      <AlertDialog open={addOpen} onOpenChange={setAddOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("marketplace.addTitle")}</AlertDialogTitle>
            <AlertDialogDescription>{t("marketplace.addDesc")}</AlertDialogDescription>
          </AlertDialogHeader>
          <div className="space-y-2">
            <input
              className="zed-input w-full"
              placeholder={t("marketplace.addNamePlaceholder")}
              value={addName}
              onChange={(e) => setAddName(e.target.value)}
            />
            <input
              className="zed-input w-full"
              placeholder={t("marketplace.addRepoPlaceholder")}
              value={addRepo}
              onChange={(e) => setAddRepo(e.target.value)}
            />
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction
              disabled={!addName.trim() || !addRepo.trim()}
              onClick={async () => {
                await addMarketplace(addName.trim(), addRepo.trim());
                queryClient.invalidateQueries({ queryKey: ["marketplaces"] });
                setAddOpen(false);
                setAddName("");
                setAddRepo("");
              }}
            >
              {t("marketplace.add")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Remove dialog */}
      <AlertDialog
        open={!!removeTarget}
        onOpenChange={(open) => !open && setRemoveTarget(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("marketplace.removeTitle", { name: removeTarget?.name })}
            </AlertDialogTitle>
            <AlertDialogDescription>{t("marketplace.removeDesc")}</AlertDialogDescription>
          </AlertDialogHeader>
          <label className="flex items-center gap-2 text-[13px] text-foreground">
            <input
              type="checkbox"
              checked={removePlugins}
              onChange={(e) => setRemovePlugins(e.target.checked)}
              className="rounded border-border"
            />
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
              className="border-destructive/30 bg-destructive/10 text-destructive hover:bg-destructive/20"
            >
              {t("marketplace.remove")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function MarketplaceRow({
  marketplace,
  isUpdating,
  expanded,
  onToggleExpand,
  onUpdate,
  onRemove,
}: {
  marketplace: MarketplaceEntry;
  isUpdating: boolean;
  expanded: boolean;
  onToggleExpand: () => void;
  onUpdate: () => void;
  onRemove: () => void;
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

  return (
    <div className="surface-card overflow-hidden">
      <div
        className="relative flex cursor-pointer items-center gap-3 overflow-hidden px-3 py-2.5 transition-colors hover:bg-accent/50"
        onClick={onToggleExpand}
      >
        <span className={`grid size-4 shrink-0 place-items-center rounded-sm bg-secondary text-[10px] text-foreground transition ${expanded ? "rotate-90" : ""}`}>
          ▶
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-foreground">{marketplace.name}</p>
          <div className="mt-0.5 flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
            <span className="font-mono">{marketplace.repo}</span>
            {marketplace.last_updated && (
              <span>· {marketplace.last_updated.split("T")[0]}</span>
            )}
            <span>· {t("marketplace.pluginCount", { count: marketplace.plugin_count })}</span>
          </div>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-6 rounded-md px-2 text-[11px]"
          onClick={(e) => { e.stopPropagation(); onUpdate(); }}
          disabled={isUpdating}
        >
          {isUpdating ? t("marketplace.updating") : t("marketplace.update")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="h-6 rounded-md px-2 text-[11px] text-muted-foreground hover:border-destructive hover:text-destructive hover:bg-destructive/10"
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
          disabled={isUpdating}
        >
          {t("marketplace.remove")}
        </Button>
        {isUpdating && <div className="indeterminate-bar" />}
      </div>

      {expanded && (
        <div className="border-t border-border bg-card">
          {pluginsLoading ? (
            <div className="space-y-1 p-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-8 w-full rounded-sm" />
              ))}
            </div>
          ) : plugins && plugins.length > 0 ? (
            [...plugins].sort((a, b) => {
              if (a.installed !== b.installed) return a.installed ? -1 : 1;
              return a.name.localeCompare(b.name);
            }).map((p) => (
              <div
                key={p.name}
                className="flex items-center gap-2 border-b border-border px-3 py-1.5 text-[12px] transition-colors hover:bg-accent/50"
              >
                <span className="truncate text-foreground">{p.name}</span>
                <span className="min-w-0 flex-1 truncate text-muted-foreground">{p.description}</span>
                {p.installed ? (
                  <span className="shrink-0 text-[10px] text-chart-2">✓ {t("marketplace.installed")}</span>
                ) : (
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-5 shrink-0 rounded-md px-1.5 text-[10px]"
                    disabled={installMut.isPending}
                    onClick={() => installMut.mutate(p.name)}
                  >
                    {installMut.isPending ? t("marketplace.installing") : t("marketplace.install")}
                  </Button>
                )}
              </div>
            ))
          ) : (
            <div className="px-3 py-3 text-[12px] text-muted-foreground">{t("marketplace.noPlugins")}</div>
          )}
        </div>
      )}
    </div>
  );
}
