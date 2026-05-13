import { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getEventTransport } from "@/lib/events";
import { useTranslation } from "react-i18next";
import { getActionLog, releaseAndResync } from "@/lib/api";
import { useLocalStorage } from "@/lib/hooks";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { PageHeader } from "@/components/ui/page-header";
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

  // Show only sync errors from the current session (last hour)
  const errorActions =
    actionLog?.actions.filter(
      (a: { detail: string | null }) =>
        a.detail?.includes("errors=") && !a.detail.includes("errors=0")
    ) ?? [];

  return (
    <div className="grid h-full min-h-0 xl:grid-cols-[minmax(0,1.2fr)_320px]">
      <section data-ai-region="settings-content" className="surface-panel overflow-auto p-3">
        <PageHeader kicker={t("settings.operations")} title={t("settings.title")} description={t("settings.description")} region="settings-header" />

        {/* Settings */}
        <div className="mt-4">
          <p className="zed-kicker">{t("settings.settings")}</p>
          <h3 className="mt-1 text-[14px] font-medium leading-none text-foreground">{t("settings.defaultTerminal")}</h3>
          <div className="mt-2 max-w-xs">
            <select
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
          </div>

          <h3 className="mt-3 text-[14px] font-medium leading-none text-foreground">{t("settings.language")}</h3>
          <div className="mt-2 flex items-center gap-2">
            <Button
              size="sm"
              variant={i18n.language === "en" ? "default" : "outline"}
              className="h-7 rounded-md px-2.5 text-[13px]"
              onClick={() => i18n.changeLanguage("en")}
            >
              English
            </Button>
            <Button
              size="sm"
              variant={i18n.language === "zh-CN" ? "default" : "outline"}
              className="h-7 rounded-md px-2.5 text-[13px]"
              onClick={() => i18n.changeLanguage("zh-CN")}
            >
              中文
            </Button>
          </div>
        </div>

        {/* Operations */}
        <div className="mt-4">
          <p className="zed-kicker">{t("settings.operations")}</p>
          <div className="flex items-center justify-between gap-3">
            <div>
              <h3 className="text-[14px] font-medium leading-none text-foreground">{t("settings.rebuild")}</h3>
              <p className="mt-1 text-[13px] text-muted-foreground">{t("settings.hintRebuildBody")}</p>
            </div>
            <div className="flex items-center gap-2">
              {confirmRebuild ? (
                <div className="flex items-center gap-2">
                  <span className="text-[13px] text-destructive">{t("settings.confirmRebuild")}</span>
                  <Button
                    size="sm"
                    className="h-7 rounded-md px-2.5 text-[13px]"
                    variant="outline"
                    onClick={() => setConfirmRebuild(false)}
                    disabled={isScanning}
                  >
                    {t("manage.cancel")}
                  </Button>
                  <Button
                    size="sm"
                    className="h-7 rounded-md px-2.5 text-[13px] font-medium"
                    variant="destructive"
                    onClick={() => rebuild.mutate()}
                    disabled={isScanning}
                  >
                    {t("settings.confirmRebuildBtn")}
                  </Button>
                </div>
              ) : (
                <Button
                  size="sm"
                  className="h-7 rounded-md px-2.5 text-[13px] font-medium disabled:opacity-60"
                  variant="outline"
                  onClick={() => setConfirmRebuild(true)}
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
              )}
            </div>
          </div>

          {isScanning && scanProgress!.total > 0 && (
            <div className="mt-3 h-1 overflow-hidden rounded-full bg-secondary">
              <div
                className="h-full rounded-full bg-primary transition-all duration-300 ease-out"
                style={{ width: `${Math.round((scanProgress!.processed / scanProgress!.total) * 100)}%` }}
              />
            </div>
          )}
        </div>

        {errorActions.length > 0 && (
          <div className="mt-4">
            <p className="zed-kicker text-destructive">{t("settings.attention")}</p>
            <h3 className="mt-1 text-[14px] font-medium leading-none text-foreground">{t("settings.syncIssues")}</h3>
            <div className="mt-2 space-y-2">
              {errorActions.map(
                (a: {
                  id: number;
                  action: string;
                  detail: string | null;
                  created_at: string;
                }) => (
                  <div
                    key={a.id}
                    className="border border-destructive/30 bg-destructive/10 px-2.5 py-2"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <Badge className="border-destructive/30 bg-destructive/10 px-1.5 py-0.5 text-[12px] text-destructive">
                        {a.action}
                      </Badge>
                      <span className="font-mono text-[12px] text-muted-foreground">
                        {formatRelativeTime(a.created_at)}
                      </span>
                    </div>
                    <p className="mt-2 text-[14px] leading-[1.5] text-destructive/80">
                      {a.detail}
                    </p>
                  </div>
                )
              )}
            </div>
          </div>
        )}
      </section>

      <aside data-ai-region="settings-ops-notes" className="surface-panel overflow-hidden border-l border-border p-3">
        <p className="zed-kicker">{t("settings.opsNotes")}</p>
        <h3 className="mt-2 text-[14px] font-semibold leading-none text-foreground">
          {t("settings.opsHeading")}
        </h3>
        <p className="mt-2 text-[14px] leading-[1.5] text-muted-foreground">
          {t("settings.opsDescription")}
        </p>
        <div className="mt-3 space-y-2">
          <SettingsHint title={t("settings.hintRebuildTitle")} body={t("settings.hintRebuildBody")} />
          <SettingsHint title={t("settings.hintErrorTitle")} body={t("settings.hintErrorBody")} />
          <SettingsHint title={t("settings.hintAuditTitle")} body={t("settings.hintAuditBody")} />
        </div>
      </aside>
    </div>
  );
}

function SettingsHint({ title, body }: { title: string; body: string }) {
  return (
    <div className="border border-border bg-secondary/50 p-2.5">
      <p className="text-[14px] font-medium text-foreground">
        {title}
      </p>
      <p className="mt-1.5 text-[14px] leading-[1.5] text-muted-foreground">{body}</p>
    </div>
  );
}
