import { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { RefreshCcw } from "lucide-react";
import { cn } from "@/lib/utils";
import { checkForUpdate, downloadAndInstall } from "@/lib/updater";
import type { Update } from "@/lib/updater";

type Phase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; downloaded: number; total: number | null }
  | { kind: "installing" }
  | { kind: "error"; message: string };

export function UpdateBanner() {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const { t } = useTranslation();

  // Defer check to avoid blocking first paint
  useEffect(() => {
    const t = setTimeout(async () => {
      setPhase({ kind: "checking" });
      const update = await checkForUpdate();
      if (update) {
        setPhase({ kind: "available", update });
      } else {
        setPhase({ kind: "idle" });
      }
    }, 3000);
    return () => clearTimeout(t);
  }, []);

  const handleUpgrade = useCallback(async (update: Update) => {
    try {
      await downloadAndInstall(update, (downloaded, total) => {
        setPhase({ kind: "downloading", downloaded, total });
      });
      setPhase({ kind: "installing" });
    } catch (e) {
      setPhase({ kind: "error", message: String(e) });
    }
  }, []);

  // Nothing to show
  if (phase.kind === "idle" || phase.kind === "checking") return null;

  return (
    <div
      className={cn(
        "flex items-center gap-3 px-4 py-2.5 rounded-md border",
        "bg-card border-border text-[14px]",
        phase.kind === "error" && "border-destructive/30 bg-destructive/10",
      )}
    >
      {/* Available */}
      {phase.kind === "available" && (
        <>
          <span className="flex-1 text-muted-foreground">
            <span className="text-foreground font-medium">
              v{phase.update.version}
            </span>
            {" — "}
            {phase.update.body ?? t("update.newVersion")}
          </span>
          <RefreshCcw size={16} className="cursor-pointer text-muted-foreground hover:text-primary" onClick={() => handleUpgrade(phase.update)} />
        </>
      )}

      {/* Downloading */}
      {phase.kind === "downloading" && (
        <div className="flex flex-1 items-center gap-3">
          <span className="text-muted-foreground text-[13px]">
            {t("update.downloading")}
          </span>
          <div className="flex-1 h-1.5 rounded-full bg-secondary overflow-hidden">
            <div
              className="h-full rounded-full bg-primary transition-all duration-300"
              style={{
                width: phase.total
                  ? `${Math.round((phase.downloaded / phase.total) * 100)}%`
                  : "20%",
              }}
            />
          </div>
          <span className="text-muted-foreground text-[12px] tabular-nums min-w-[48px] text-right">
            {phase.total
              ? `${Math.round((phase.downloaded / phase.total) * 100)}%`
              : `${(phase.downloaded / 1024 / 1024).toFixed(1)} MB`}
          </span>
        </div>
      )}

      {/* Installing */}
      {phase.kind === "installing" && (
        <span className="text-muted-foreground">
          {t("update.installing")}
        </span>
      )}

      {/* Error */}
      {phase.kind === "error" && (
        <>
          <span className="flex-1 text-destructive text-[13px]">
            {t("update.failed", { message: phase.message })}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setPhase({ kind: "idle" })}
          >
            {t("update.dismiss")}
          </Button>
        </>
      )}
    </div>
  );
}
