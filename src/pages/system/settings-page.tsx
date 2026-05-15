import { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getEventTransport } from "@/lib/events";
import { useTranslation } from "react-i18next";
import { getActionLog, releaseAndResync } from "@/lib/api";
import { useLocalStorage } from "@/lib/hooks";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { formatRelativeTime } from "@/lib/formatters";

const TERMINAL_OPTIONS = [
  "Ghostty",
  "iTerm",
  "Warp",
  "WezTerm",
  "kitty",
  "Alacritty",
  "Terminal.app",
  "cmux",
  "ghostty",
  "gnome-terminal",
  "konsole",
  "xfce4-terminal",
  "pwsh.exe",
  "powershell.exe",
  "wt.exe",
];

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const { t, i18n } = useTranslation();
  const [scanProgress, setScanProgress] = useState<{ processed: number; total: number } | null>(null);
  const [defaultTerminal, setDefaultTerminal] = useLocalStorage<string>("default-terminal", "");

  const { data: actionLog } = useQuery({
    queryKey: ["action-log"],
    queryFn: () => getActionLog(50),
  });

  const [confirmRebuild, setConfirmRebuild] = useState(false);
  const rebuild = useMutation({
    mutationFn: releaseAndResync,
    onSuccess: () => {
      setConfirmRebuild(false);
    },
  });

  // Listen for scan progress and completion events
  useEffect(() => {
    const transport = getEventTransport();
    const unlistenStarted = transport.on<{ source_count: number }>("sync-started", (payload) => {
      setScanProgress({ processed: 0, total: payload.source_count });
    });
    const unlistenProgress = transport.on<{ processed: number; total: number }>("sync-progress", (payload) => {
      setScanProgress(payload);
    });
    const unlistenCompleted = transport.on("sync-completed", () => {
      setScanProgress(null);
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["system-status"] });
      queryClient.invalidateQueries({ queryKey: ["action-log"] });
    });

    return () => {
      unlistenStarted.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenCompleted.then((f) => f());
    };
  }, [queryClient]);

  const isScanning = scanProgress !== null;
  const progressPercent =
    scanProgress && scanProgress.total > 0
      ? Math.round((scanProgress.processed / scanProgress.total) * 100)
      : 0;

  // Show only sync errors from the current session (last hour)
  const errorActions =
    actionLog?.actions.filter(
      (a: { detail: string | null }) =>
        a.detail?.includes("errors=") && !a.detail.includes("errors=0")
    ) ?? [];

  return (
    <div className="grid h-full min-h-0 xl:grid-cols-[minmax(0,1.2fr)_320px]">
      <section data-ai-region="settings-content" className="overflow-auto p-3">

        <div className="mt-3 space-y-3">
          <section className="surface-card p-3">
            <p className="zed-kicker">{t("settings.preferences")}</p>
            <div className="mt-2.5 grid gap-2.5 xl:grid-cols-2">
              <SettingsCard title={t("settings.defaultTerminal")} description={t("settings.terminalDescription")}>
                <label className="block">
                  <span className="sr-only">{t("settings.defaultTerminal")}</span>
                  <select
                    title={t("settings.defaultTerminal")}
                    value={defaultTerminal}
                    onChange={(e) => setDefaultTerminal(e.target.value)}
                    className="zed-input"
                  >
                    <option value="">{t("settings.terminalAuto")}</option>
                    {TERMINAL_OPTIONS.map((name) => (
                      <option key={name} value={name}>
                        {name}
                      </option>
                    ))}
                  </select>
                </label>
              </SettingsCard>
              <SettingsCard title={t("settings.language")} description={t("settings.languageDescription")}>
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    size="sm"
                    variant={i18n.language === "en" ? "default" : "outline"}
                    className="h-8 rounded-md px-3 text-[14px]"
                    onClick={() => i18n.changeLanguage("en")}
                  >
                    {t("settings.languageEnglish")}
                  </Button>
                  <Button
                    size="sm"
                    variant={i18n.language === "zh-CN" ? "default" : "outline"}
                    className="h-8 rounded-md px-3 text-[14px]"
                    onClick={() => i18n.changeLanguage("zh-CN")}
                  >
                    {t("settings.languageChinese")}
                  </Button>
                </div>
              </SettingsCard>
            </div>
          </section>

          <section className="surface-card p-3">
            <p className="zed-kicker">{t("settings.maintenance")}</p>
            <div className="mt-2.5 flex flex-wrap items-start justify-between gap-2.5">
              <div className="max-w-xl">
                <h3 className="text-[16px] font-medium text-foreground">{t("settings.rebuild")}</h3>
                <p className="mt-0.5 text-[12px] leading-[1.5] text-muted-foreground">{t("settings.hintRebuildBody")}</p>
              </div>
              <Button
                size="sm"
                className="h-8 rounded-md px-3 text-[14px] font-medium disabled:opacity-60"
                variant={isScanning ? "default" : "destructive"}
                onClick={() => !isScanning && setConfirmRebuild(true)}
                disabled={isScanning}
              >
                {isScanning ? (
                  <span className="flex items-center gap-2">
                    <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-primary-foreground/30 border-t-primary-foreground" />
                    {scanProgress!.total > 0
                      ? `${scanProgress!.processed}/${scanProgress!.total}`
                      : t("settings.starting")}
                  </span>
                ) : (
                  t("settings.rebuild")
                )}
              </Button>
            </div>

            {isScanning && scanProgress!.total > 0 && (
              <div className="mt-3 rounded-lg border border-border bg-secondary p-2.5">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-[14px] font-medium text-foreground">{t("settings.rebuilding")}</span>
                  <span className="font-mono text-[12px] text-muted-foreground">{progressPercent}%</span>
                </div>
                <progress className="session-progress mt-2 h-1.5 w-full overflow-hidden rounded-full bg-secondary" value={progressPercent} max={100} />
              </div>
            )}
          </section>

          {errorActions.length > 0 && (
            <section className="surface-card p-3">
              <p className="zed-kicker text-destructive">{t("settings.attention")}</p>
              <h3 className="mt-1 text-[16px] font-medium text-foreground">{t("settings.syncIssues")}</h3>
              <div className="mt-2.5 space-y-2">
              {errorActions.map(
                (a: {
                  id: number;
                  action: string;
                  detail: string | null;
                  created_at: string;
                }) => (
                  <div
                    key={a.id}
                    className="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <Badge className="border-destructive/30 bg-destructive/10 px-1.5 py-0.5 text-[12px] text-destructive">
                        {a.action}
                      </Badge>
                      <span className="font-mono text-[12px] text-muted-foreground">
                        {formatRelativeTime(a.created_at)}
                      </span>
                    </div>
                    <p className="mt-1.5 text-[14px] leading-[1.45] text-destructive/80">
                      {a.detail}
                    </p>
                  </div>
                )
              )}
              </div>
            </section>
          )}
        </div>
      </section>

      <aside data-ai-region="settings-ops-notes" className="surface-panel overflow-auto p-3">
        <p className="zed-kicker">{t("settings.opsNotes")}</p>
        <h3 className="mt-1.5 text-[14px] font-semibold leading-none text-foreground">
          {t("settings.opsHeading")}
        </h3>
        <p className="mt-1.5 text-[14px] leading-[1.45] text-muted-foreground">
          {t("settings.opsDescription")}
        </p>
        <div className="mt-2.5 space-y-2">
          <SettingsHint title={t("settings.hintRebuildTitle")} body={t("settings.hintRebuildBody")} />
          <SettingsHint title={t("settings.hintErrorTitle")} body={t("settings.hintErrorBody")} />
          <SettingsHint title={t("settings.hintAuditTitle")} body={t("settings.hintAuditBody")} />
        </div>
      </aside>

      <AlertDialog open={confirmRebuild} onOpenChange={setConfirmRebuild}>
        <AlertDialogContent size="sm">
          <AlertDialogHeader>
            <AlertDialogTitle>{t("settings.confirmRebuild")}</AlertDialogTitle>
            <AlertDialogDescription>{t("settings.rebuildImpact")}</AlertDialogDescription>
          </AlertDialogHeader>
          <div className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2.5">
            <p className="text-[14px] font-medium text-destructive">{t("settings.rebuild")}</p>
            <p className="mt-1 text-[12px] leading-[1.5] text-muted-foreground">{t("settings.hintRebuildBody")}</p>
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("manage.cancel")}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => rebuild.mutate()} disabled={rebuild.isPending || isScanning}>
              {rebuild.isPending ? t("settings.starting") : t("settings.confirmRebuildBtn")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function SettingsHint({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-lg border border-border bg-secondary p-2.5">
      <p className="text-[14px] font-medium text-foreground">
        {title}
      </p>
      <p className="mt-1 text-[12px] leading-[1.45] text-muted-foreground">{body}</p>
    </div>
  );
}

function SettingsCard({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-lg border border-border bg-secondary p-2.5">
      <h3 className="text-[14px] font-medium text-foreground">{title}</h3>
      <p className="mt-0.5 text-[12px] leading-[1.45] text-muted-foreground">{description}</p>
      <div className="mt-2.5">{children}</div>
    </div>
  );
}
