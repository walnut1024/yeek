import type { MessageRecord } from "@/lib/api";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { X, Clock3, Boxes } from "lucide-react";

interface NodeDetailPanelProps {
  nodeId: string;
  messages: MessageRecord[];
  onClose: () => void;
}

export default function NodeDetailPanel({
  nodeId,
  messages,
  onClose,
}: NodeDetailPanelProps) {
  const { t } = useTranslation();
  const msg = messages.find((m) => m.id === nodeId);
  if (!msg) return null;

  const roleLabel =
    msg.role === "human"
      ? t("graph.nodeUser")
      : msg.kind === "tool_use"
        ? t("graph.nodeTool", { name: msg.tool_name || t("graph.nodeUnknown") })
        : msg.kind === "tool_result"
          ? t("graph.nodeResult")
          : t("graph.nodeAssistant");
  const tone =
    msg.role === "human"
      ? "border-primary/20 bg-primary/10 text-primary"
      : msg.kind === "tool_use"
        ? "border-border bg-secondary text-foreground"
        : msg.kind === "tool_result"
          ? "border-border bg-secondary/50 text-muted-foreground"
          : "border-border bg-card text-muted-foreground";
  const toolTone = getToolTone(msg.tool_name ?? undefined);

  return (
    <aside className="absolute inset-y-0 right-0 z-10 flex w-[330px] max-w-[90vw] flex-col border-l border-border bg-card backdrop-blur-sm">
      <div className="flex items-start justify-between gap-2 border-b border-border px-3 py-3">
        <div className="min-w-0">
          <p className="zed-kicker">{t("graph.panelTitle")}</p>
          <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
            <Badge variant="outline" className={`px-1.5 py-0.5 text-[10px] ${tone}`}>
              {roleLabel}
            </Badge>
            {msg.tool_name && (
              <span
                className={`rounded-full border px-1.5 py-0.5 font-mono text-[10px] ${toolTone.badge}`}
              >
                {msg.tool_name}
              </span>
            )}
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onClose}
          className="text-muted-foreground"
          aria-label={t("graph.closePanel")}
        >
          <X size={16} />
        </Button>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-3 p-3">
          <section className="rounded-xl border border-border bg-secondary p-2.5">
            <p className="zed-kicker">{t("graph.messageMeta")}</p>
            <div className="mt-1.5 space-y-1.5 text-[12px] text-muted-foreground">
              <MetaRow label={t("graph.messageId")} value={msg.id} mono />
              {msg.model && <MetaRow label={t("graph.model")} value={msg.model} mono />}
              {msg.timestamp && (
                <MetaRow
                  label={t("graph.timestamp")}
                  value={new Date(msg.timestamp).toLocaleString()}
                  icon={<Clock3 size={13} />}
                />
              )}
              {msg.parent_id && <MetaRow label={t("graph.parentId")} value={msg.parent_id} mono />}
            </div>
          </section>

          <section className="rounded-xl border border-border bg-card p-2.5">
            <p className="zed-kicker">{t("graph.previewTitle")}</p>
            <p className="mt-1.5 whitespace-pre-wrap break-words text-[12px] leading-[1.55] text-foreground">
              {msg.content_preview || t("graph.nodeEmpty")}
            </p>
          </section>

          {msg.tool_name && (
            <section className="rounded-xl border border-border bg-secondary p-2.5">
              <div className="flex items-center gap-2">
                <Boxes size={14} className="text-muted-foreground" />
                <p className="zed-kicker">{t("graph.toolMetadata")}</p>
              </div>
              <Separator className="my-2.5" />
              <div className="space-y-1.5 text-[12px] text-muted-foreground">
                <MetaRow label={t("graph.toolName")} value={msg.tool_name} mono accentClass={toolTone.text} />
                {msg.kind && <MetaRow label={t("graph.kind")} value={msg.kind} mono />}
                {msg.entry_type && <MetaRow label={t("graph.entryType")} value={msg.entry_type} mono />}
              </div>
            </section>
          )}
        </div>
      </ScrollArea>
    </aside>
  );
}

function MetaRow({
  label,
  value,
  mono,
  icon,
  accentClass,
}: {
  label: string;
  value: string;
  mono?: boolean;
  icon?: ReactNode;
  accentClass?: string;
}) {
  return (
    <div className="rounded-lg border border-border bg-card px-2.5 py-1.5">
      <p className="zed-kicker flex items-center gap-1.5">
        {icon}
        <span>{label}</span>
      </p>
      <p
        className={`mt-0.5 break-all text-[12px] leading-[1.45] ${mono ? "font-mono" : ""} ${accentClass || "text-foreground"}`}
      >
        {value}
      </p>
    </div>
  );
}

function getToolTone(toolName?: string) {
  switch (toolName) {
    case "Bash":
      return { badge: "border-orange-400/20 bg-orange-500/10 text-orange-300", text: "text-orange-300" };
    case "Read":
    case "Write":
    case "Edit":
      return { badge: "border-amber-300/20 bg-amber-400/10 text-amber-200", text: "text-amber-200" };
    case "Grep":
    case "Glob":
      return { badge: "border-emerald-400/20 bg-emerald-500/10 text-emerald-300", text: "text-emerald-300" };
    case "Agent":
      return { badge: "border-fuchsia-400/20 bg-fuchsia-500/10 text-fuchsia-300", text: "text-fuchsia-300" };
    case "WebSearch":
      return { badge: "border-sky-400/20 bg-sky-500/10 text-sky-300", text: "text-sky-300" };
    default:
      return { badge: "border-border bg-secondary text-muted-foreground", text: "text-foreground" };
  }
}
