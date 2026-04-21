import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  listPlugins,
  togglePlugin,
  uninstallPlugin,
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

// Health color mapping — uses design system tokens (chart-2=green, chart-3=amber, chart-5=gray, destructive=red)
const HEALTH_COLORS: Record<string, { dot: string; text: string; bg: string; border: string }> = {
  ok: { dot: "bg-chart-2", text: "text-chart-2", bg: "bg-chart-2/15", border: "border-chart-2/30" },
  partial: { dot: "bg-chart-3", text: "text-chart-3", bg: "bg-chart-3/15", border: "border-chart-3/30" },
  hook: { dot: "bg-chart-5", text: "text-chart-5", bg: "bg-chart-5/15", border: "border-chart-5/30" },
  broken: { dot: "bg-destructive", text: "text-destructive", bg: "bg-destructive/15", border: "border-destructive/30" },
};

const HEALTH_BORDER_L: Record<string, string> = {
  ok: "border-l-chart-2",
  partial: "border-l-chart-3",
  hook: "border-l-chart-5",
  broken: "border-l-destructive",
};

const HEALTH_LABELS: Record<string, string> = {
  ok: "OK",
  partial: "PARTIAL",
  hook: "HOOK",
  broken: "BROKEN",
};

export default function SkillsPage() {
  const [scope, setScope] = useState<"global" | "project">("global");
  const [view, setView] = useState<"plugin" | "flat">("plugin");
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [uninstallTarget, setUninstallTarget] = useState<PluginInfo | null>(null);
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data, isLoading, refetch } = useQuery({
    queryKey: ["plugins", scope],
    queryFn: () => listPlugins(scope),
    refetchInterval: 30_000,
  });

  const toggleMut = useMutation({
    mutationFn: (key: string) => togglePlugin(key),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["plugins"] }),
  });

  const uninstallMut = useMutation({
    mutationFn: (key: string) => uninstallPlugin(key),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      setUninstallTarget(null);
    },
  });

  const plugins = data?.plugins ?? [];
  const hs = data?.health_summary;
  const flatSkills = plugins.flatMap((p) => [
    ...p.skills.map((s) => ({ ...s, plugin: p })),
    ...p.agents.map((a) => ({ ...a, plugin: p })),
  ]);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Toolbar — matches sessions page toolbar style */}
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          {(["global", "project"] as const).map((s) => (
            <button
              type="button"
              key={s}
              onClick={() => setScope(s)}
              className={`pill-tab ${scope === s ? "pill-tab-active" : "pill-tab-idle"}`}
            >
              {t(`skills.${s}`)}
              <span className="ml-1 text-[10px] opacity-60">
                {s === "global" ? (data?.total_plugins ?? "...") : "0"}
              </span>
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <span className="zed-chip">
            {data
              ? t("skills.countChip", { skills: data.total_skills, agents: data.total_agents })
              : "..."}
          </span>
          {(["plugin", "flat"] as const).map((v) => (
            <button
              type="button"
              key={v}
              onClick={() => setView(v)}
              className={`pill-tab ${view === v ? "pill-tab-active" : "pill-tab-idle"}`}
            >
              {v === "plugin" ? t("skills.viewPlugins") : t("skills.viewAllSkills")}
            </button>
          ))}
          <Button
            variant="outline"
            size="sm"
            className="h-7 rounded-md px-2.5 text-[13px]"
            onClick={() => refetch()}
          >
            {t("skills.refresh")}
          </Button>
        </div>
      </div>

      {/* Health bar */}
      {hs && (
        <div className="flex items-center gap-3 border-b border-border bg-secondary px-3 py-1.5 text-[12px] text-muted-foreground">
          <span className="text-[10px] uppercase tracking-[0.08em]">{t("skills.health")}</span>
          <HealthDot dot="bg-chart-2" count={hs.ok} />
          <HealthDot dot="bg-chart-3" count={hs.partial} />
          <HealthDot dot="bg-chart-5" count={hs.hook} />
          <HealthDot dot="bg-destructive" count={hs.broken} />
        </div>
      )}

      {/* Content */}
      <ScrollArea className="min-h-0 flex-1">
        {isLoading ? (
          <div className="space-y-1 p-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-18 w-full rounded-md" />
            ))}
          </div>
        ) : plugins.length === 0 ? (
          <div className="flex h-72 items-center justify-center px-6">
            <div className="max-w-sm text-center">
              <div className="mx-auto mb-3 size-10 rounded-sm border border-border bg-secondary" />
              <p className="text-[16px] font-medium text-foreground">
                {scope === "project" ? t("skills.emptyProject") : t("skills.emptyStandalone")}
              </p>
            </div>
          </div>
        ) : view === "plugin" ? (
          <div className="space-y-1 p-2">
            {plugins.map((p) => (
              <PluginCard
                key={p.key}
                plugin={p}
                expanded={expandedKey === p.key}
                onToggleExpand={() => setExpandedKey(expandedKey === p.key ? null : p.key)}
                onToggle={() => toggleMut.mutate(p.key)}
                onUninstall={() => setUninstallTarget(p)}
              />
            ))}
          </div>
        ) : (
          <div className="space-y-1 p-2">
            {flatSkills.map((s, i) => (
              <div
                key={i}
                className="zed-list-row flex items-center gap-2 border border-transparent px-2.5 py-1.5 transition-colors hover:border-border hover:bg-accent/50"
              >
                <Badge
                  variant="outline"
                  className={`px-1 py-0 text-[10px] font-medium ${
                    s.skill_type === "agent"
                      ? "text-chart-3 border-chart-3/30"
                      : "text-primary border-primary/30"
                  }`}
                >
                  {s.skill_type === "agent" ? "A" : "S"}
                </Badge>
                <span className="w-[160px] shrink-0 truncate text-[13px] font-medium text-foreground">
                  {s.name}
                </span>
                <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">
                  {s.description}
                </span>
                <span className="zed-chip">
                  {s.plugin.name} ← {s.plugin.marketplace?.name ?? ""}
                </span>
                <span
                  className={`size-1.5 shrink-0 rounded-full ${
                    s.health === "ok" ? "bg-chart-2" : "bg-chart-3"
                  }`}
                />
              </div>
            ))}
          </div>
        )}
      </ScrollArea>

      {/* Uninstall dialog */}
      <AlertDialog
        open={!!uninstallTarget}
        onOpenChange={(open) => !open && setUninstallTarget(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("skills.uninstallTitle", { name: uninstallTarget?.name })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              <span className="mb-2 block rounded-sm border border-border bg-secondary p-2 font-mono text-[11px] text-muted-foreground break-all">
                {uninstallTarget?.install_path}
              </span>
              {t("skills.uninstallDesc")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction
              disabled={uninstallMut.isPending}
              onClick={() => uninstallTarget && uninstallMut.mutate(uninstallTarget.key)}
              className="border-[#4c2b2c] bg-destructive/10 text-destructive hover:bg-destructive/20"
            >
              {uninstallMut.isPending ? t("detail.deleting") : t("skills.uninstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function HealthDot({ dot, count }: { dot: string; count: number }) {
  return (
    <span className="flex items-center gap-1">
      <span className={`size-1.5 rounded-full ${dot}`} />
      <span className="font-mono text-[11px]">{count}</span>
    </span>
  );
}

function PluginCard({
  plugin,
  expanded,
  onToggleExpand,
  onToggle,
  onUninstall,
}: {
  plugin: PluginInfo;
  expanded: boolean;
  onToggleExpand: () => void;
  onToggle: () => void;
  onUninstall: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div
      className={`surface-card overflow-hidden transition border-l-[3px] ${
        HEALTH_BORDER_L[plugin.health] ?? ""
      }`}
    >
      <div
        className="flex cursor-pointer items-center gap-2 px-2.5 py-2 transition-colors hover:bg-accent/50"
        onClick={onToggleExpand}
      >
        <span
          className={`grid size-4 shrink-0 place-items-center rounded-sm bg-secondary text-[10px] text-primary transition ${
            expanded ? "rotate-90" : ""
          }`}
        >
          ▶
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-foreground">{plugin.name}</p>
          <div className="mt-0.5 flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
            <span className="zed-chip font-mono">v{plugin.version}</span>
            {plugin.skills.length + plugin.agents.length > 0 && (
              <span className="zed-chip">
                {plugin.skills.length} skills · {plugin.agents.length} agents
              </span>
            )}
            {plugin.health === "hook" && <span className="zed-chip">hook-only</span>}
            {plugin.marketplace && (
              <span className="text-primary opacity-70">
                ← {plugin.marketplace.name}
                <span className="ml-1 font-mono text-[10px]">{plugin.marketplace.repo}</span>
              </span>
            )}
          </div>
        </div>
        <HealthBadge health={plugin.health} />
        <label
          className="relative inline-flex shrink-0 cursor-pointer"
          onClick={(e) => e.stopPropagation()}
        >
          <input
            type="checkbox"
            className="sr-only"
            checked={plugin.enabled}
            onChange={onToggle}
          />
          <span
            className={`block h-[18px] w-[32px] rounded-full border transition ${
              plugin.enabled ? "bg-primary border-primary" : "bg-secondary border-border"
            }`}
          >
            <span
              className={`mt-[2px] ml-[2px] block size-3 rounded-full bg-foreground transition ${
                plugin.enabled ? "translate-x-[14px]" : ""
              }`}
            />
          </span>
        </label>
        <Button
          variant="outline"
          size="sm"
          className="h-6 rounded-md px-2 text-[11px] text-muted-foreground hover:border-destructive hover:text-destructive hover:bg-destructive/10"
          onClick={(e) => {
            e.stopPropagation();
            onUninstall();
          }}
        >
          {t("skills.uninstall")}
        </Button>
      </div>

      {expanded && (
        <div className="border-t border-border bg-card">
          <DetailRow label={t("skills.path")} value={plugin.install_path} mono />
          {plugin.marketplace && (
            <DetailRow
              label={t("skills.market")}
              value={`${plugin.marketplace.name} · ${plugin.marketplace.repo}${
                plugin.marketplace.last_updated
                  ? ` · ${plugin.marketplace.last_updated.split("T")[0]}`
                  : ""
              }`}
            />
          )}
          {plugin.health_issues.length > 0 && (
            <div className="border-b border-border px-3 py-2">
              {plugin.health_issues.map((issue, i) => (
                <div
                  key={i}
                  className={`flex items-center gap-1 text-[11px] ${
                    plugin.health === "broken" ? "text-destructive" : "text-chart-3"
                  }`}
                >
                  <span
                    className={`size-1 rounded-full ${
                      plugin.health === "broken" ? "bg-destructive" : "bg-chart-3"
                    }`}
                  />
                  {issue}
                </div>
              ))}
            </div>
          )}
          {plugin.skills.map((s) => (
            <SkillRow key={s.name} skill={s} />
          ))}
          {plugin.agents.map((a) => (
            <SkillRow key={a.name} skill={a} />
          ))}
        </div>
      )}
    </div>
  );
}

function DetailRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-center gap-2 border-b border-border px-3 py-1">
      <span className="w-12 shrink-0 text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
        {label}
      </span>
      <span
        className={`min-w-0 flex-1 truncate text-[11px] text-muted-foreground ${
          mono ? "font-mono" : ""
        }`}
      >
        {value}
      </span>
    </div>
  );
}

function SkillRow({ skill }: { skill: SkillInfo }) {
  return (
    <div className="flex items-center gap-2 border-b border-border px-3 py-1 transition-colors hover:bg-accent/50">
      <Badge
        variant="outline"
        className={`grid size-4 shrink-0 place-items-center px-0 py-0 text-[9px] font-medium ${
          skill.skill_type === "agent"
            ? "text-chart-3 border-chart-3/30"
            : "text-primary border-primary/30"
        }`}
      >
        {skill.skill_type === "agent" ? "A" : "S"}
      </Badge>
      <span className="truncate text-[13px] text-foreground">{skill.name}</span>
      <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">
        {skill.description}
      </span>
      {skill.tools && <span className="zed-chip font-mono text-[10px]">{skill.tools}</span>}
      <span
        className={`size-1.5 shrink-0 rounded-full ${
          skill.health === "ok" ? "bg-chart-2" : "bg-chart-3"
        }`}
      />
    </div>
  );
}

function HealthBadge({ health }: { health: string }) {
  const hc = HEALTH_COLORS[health] ?? HEALTH_COLORS.hook;
  return (
    <span
      className={`flex shrink-0 items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.04em] ${hc.text} ${hc.bg} ${hc.border}`}
    >
      <span className={`size-1 rounded-full ${hc.dot}`} />
      {HEALTH_LABELS[health] ?? health.toUpperCase()}
    </span>
  );
}
