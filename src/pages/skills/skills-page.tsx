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

export default function SkillsPage() {
  const [scope, setScope] = useState<"global" | "project">("global");
  const [view, setView] = useState<"plugin" | "flat">("plugin");
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [uninstallTarget, setUninstallTarget] = useState<PluginInfo | null>(null);
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ["plugins", scope],
    queryFn: () => listPlugins(scope),
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
      {/* Toolbar */}
      <div className="flex items-center justify-between border-b border-border bg-surface px-3 py-2">
        <div className="flex items-center gap-3">
          <h2 className="text-[14px] font-medium text-foreground">{t("skills.title")}</h2>
          <div className="view-toggle flex overflow-hidden rounded-md border border-border">
            <button
              type="button"
              className={`px-2.5 py-1 text-[12px] font-medium transition ${scope === "global" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setScope("global")}
            >
              {t("skills.global")} <span className="text-[10px] opacity-60">{data?.total_plugins ?? "..."}</span>
            </button>
            <button
              type="button"
              className={`border-l border-border px-2.5 py-1 text-[12px] font-medium transition ${scope === "project" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setScope("project")}
            >
              {t("skills.project")} <span className="text-[10px] opacity-60">0</span>
            </button>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <div className="zed-chip px-2 py-1 font-mono text-[12px]">
            {data ? t("skills.countChip", { skills: data.total_skills, agents: data.total_agents }) : "..."}
          </div>
          <div className="view-toggle flex overflow-hidden rounded-md border border-border">
            <button
              type="button"
              className={`px-2.5 py-1 text-[12px] font-medium transition ${view === "plugin" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setView("plugin")}
            >
              {t("skills.viewPlugins")}
            </button>
            <button
              type="button"
              className={`border-l border-border px-2.5 py-1 text-[12px] font-medium transition ${view === "flat" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setView("flat")}
            >
              {t("skills.viewAllSkills")}
            </button>
          </div>
        </div>
      </div>

      {/* Health bar */}
      {hs && (
        <div className="flex items-center gap-3 border-b border-[var(--border-variant)] bg-[var(--element)] px-3 py-1.5 text-[12px] text-muted-foreground">
          <span className="text-[10px] uppercase tracking-[0.06em] text-placeholder">{t("skills.health")}</span>
          <HealthDot color="ok" count={hs.ok} />
          <HealthDot color="partial" count={hs.partial} />
          <HealthDot color="hook" count={hs.hook} />
          <HealthDot color="broken" count={hs.broken} />
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
          <div className="p-2">
            {flatSkills.map((s, i) => (
              <div key={i} className="flex items-center gap-2 rounded-md px-3 py-1.5 hover:bg-[var(--element-hover)]">
                <span className={`text-[10px] font-medium uppercase ${s.skill_type === "agent" ? "text-[var(--warning)]" : "text-[var(--accent)]"}`}>
                  {s.skill_type === "agent" ? "A" : "S"}
                </span>
                <span className="w-[160px] shrink-0 truncate text-[13px] font-medium text-foreground">{s.name}</span>
                <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">{s.description}</span>
                <span className="shrink-0 rounded-sm border border-[var(--border-variant)] bg-[var(--element)] px-1.5 py-0.5 text-[11px] text-muted-foreground">
                  {s.plugin.name} ← {s.plugin.marketplace?.name ?? ""}
                </span>
                <span className={`size-1.5 shrink-0 rounded-full ${s.health === "ok" ? "bg-[var(--success)]" : "bg-[var(--warning)]"}`} />
              </div>
            ))}
          </div>
        )}
      </ScrollArea>

      {/* Uninstall dialog */}
      <AlertDialog open={!!uninstallTarget} onOpenChange={(open) => !open && setUninstallTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("skills.uninstallTitle", { name: uninstallTarget?.name })}</AlertDialogTitle>
            <AlertDialogDescription>
              <span className="font-mono text-[11px] text-muted-foreground block mb-2 rounded-sm border border-[var(--border-variant)] bg-[var(--element)] p-2 break-all">
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

function HealthDot({ color, count }: { color: string; count: number }) {
  const colors: Record<string, string> = {
    ok: "bg-[var(--success)]",
    partial: "bg-[var(--warning)]",
    hook: "bg-[var(--text-placeholder)]",
    broken: "bg-[var(--error)]",
  };
  return (
    <span className="flex items-center gap-1">
      <span className={`size-1.5 rounded-full ${colors[color] ?? ""}`} />
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
  const borderColors: Record<string, string> = {
    ok: "border-l-[3px] border-l-[var(--success)]",
    partial: "border-l-[3px] border-l-[var(--warning)]",
    hook: "border-l-[3px] border-l-[var(--text-placeholder)]",
    broken: "border-l-[3px] border-l-[var(--error)]",
  };

  return (
    <div className={`overflow-hidden rounded-md border border-border bg-[var(--surface)] transition ${borderColors[plugin.health] ?? ""}`}>
      <div
        className="flex cursor-pointer items-center gap-2 px-3 py-2 hover:bg-[var(--element-hover)]"
        onClick={onToggleExpand}
      >
        <span className={`grid size-4 shrink-0 place-items-center rounded-sm bg-[var(--element)] text-[10px] text-[var(--accent)] transition ${expanded ? "rotate-90" : ""}`}>&#x25B6;</span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-foreground">{plugin.name}</p>
          <div className="mt-0.5 flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
            <span className="font-mono">v{plugin.version}</span>
            {(plugin.skills.length + plugin.agents.length) > 0 && (
              <span>{plugin.skills.length} skills, {plugin.agents.length} agents</span>
            )}
            {plugin.health === "hook" && <span>hook-only</span>}
            {plugin.marketplace && (
              <span className="text-[var(--accent)] opacity-70">
                ← {plugin.marketplace.name}
                <span className="font-mono text-[10px] ml-1">{plugin.marketplace.repo}</span>
              </span>
            )}
          </div>
        </div>
        <HealthBadge health={plugin.health} />
        <label className="relative inline-flex shrink-0 cursor-pointer" onClick={(e) => e.stopPropagation()}>
          <input type="checkbox" className="sr-only" checked={plugin.enabled} onChange={onToggle} />
          <span className={`block h-[18px] w-[32px] rounded-full border transition ${plugin.enabled ? "bg-[var(--accent)] border-[var(--accent)]" : "bg-[var(--element-active)] border-[var(--border)]"}`}>
            <span className={`block size-3 rounded-full bg-foreground transition ${plugin.enabled ? "translate-x-[14px]" : ""} mt-[2px] ml-[2px]`} />
          </span>
        </label>
        <button
          type="button"
          className="shrink-0 rounded-md border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground transition hover:border-[var(--error)] hover:text-[var(--error)] hover:bg-[rgba(208,114,119,0.1)]"
          onClick={(e) => { e.stopPropagation(); onUninstall(); }}
        >
          {t("skills.uninstall")}
        </button>
      </div>

      {expanded && (
        <div className="border-t border-border bg-[var(--editor)]">
          <DetailRow label={t("skills.path")} value={plugin.install_path} mono />
          {plugin.marketplace && (
            <DetailRow
              label={t("skills.market")}
              value={`${plugin.marketplace.name} · ${plugin.marketplace.repo}${plugin.marketplace.last_updated ? ` · ${plugin.marketplace.last_updated.split("T")[0]}` : ""}`}
            />
          )}
          {plugin.health_issues.length > 0 && (
            <div className="px-3 py-2 border-b border-[var(--border-variant)]">
              {plugin.health_issues.map((issue, i) => (
                <div key={i} className={`text-[11px] flex items-center gap-1 ${plugin.health === "broken" ? "text-[var(--error)]" : "text-[var(--warning)]"}`}>
                  <span className={`size-1 rounded-full ${plugin.health === "broken" ? "bg-[var(--error)]" : "bg-[var(--warning)]"}`} />
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
    <div className="flex items-center gap-2 border-b border-[var(--border-variant)] px-3 py-1">
      <span className="text-[10px] uppercase tracking-[0.06em] text-placeholder opacity-70 shrink-0 w-12">{label}</span>
      <span className={`min-w-0 flex-1 truncate text-[11px] text-placeholder ${mono ? "font-mono direction-rtl text-left" : ""}`}>{value}</span>
    </div>
  );
}

function SkillRow({ skill }: { skill: SkillInfo }) {
  return (
    <div className="flex items-center gap-2 border-b border-[var(--border-variant)] px-3 py-1 hover:bg-[var(--element-hover)]">
      <span className={`grid size-4 shrink-0 place-items-center rounded-sm bg-[var(--element)] text-[9px] ${skill.skill_type === "agent" ? "text-[var(--warning)]" : "text-[var(--accent)]"}`}>
        {skill.skill_type === "agent" ? "A" : "S"}
      </span>
      <span className="text-[13px] text-foreground truncate">{skill.name}</span>
      <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">{skill.description}</span>
      {skill.tools && (
        <span className="shrink-0 rounded-sm border border-[var(--border-variant)] bg-[var(--element)] px-1 py-0.5 font-mono text-[10px] text-muted-foreground">{skill.tools}</span>
      )}
      <span className={`size-1.5 shrink-0 rounded-full ${skill.health === "ok" ? "bg-[var(--success)]" : "bg-[var(--warning)]"}`} />
    </div>
  );
}

function HealthBadge({ health }: { health: string }) {
  const styles: Record<string, string> = {
    ok: "text-[var(--success)] bg-[rgba(161,193,129,0.15)] border-[rgba(161,193,129,0.3)]",
    partial: "text-[var(--warning)] bg-[rgba(222,193,132,0.15)] border-[rgba(222,193,132,0.3)]",
    hook: "text-[var(--text-placeholder)] bg-[rgba(135,138,152,0.15)] border-[rgba(135,138,152,0.3)]",
    broken: "text-[var(--error)] bg-[rgba(208,114,119,0.15)] border-[rgba(208,114,119,0.3)]",
  };
  const dotColors: Record<string, string> = {
    ok: "bg-[var(--success)]",
    partial: "bg-[var(--warning)]",
    hook: "bg-[var(--text-placeholder)]",
    broken: "bg-[var(--error)]",
  };
  const labels: Record<string, string> = {
    ok: "OK",
    partial: "PARTIAL",
    hook: "HOOK",
    broken: "BROKEN",
  };
  return (
    <span className={`flex shrink-0 items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.04em] ${styles[health] ?? ""}`}>
      <span className={`size-1 rounded-full ${dotColors[health] ?? ""}`} />
      {labels[health] ?? health.toUpperCase()}
    </span>
  );
}
