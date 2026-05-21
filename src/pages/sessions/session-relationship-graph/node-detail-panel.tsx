import type { MessageRecord } from "@/lib/api";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { X, Clock3, Boxes, Copy, CircleCheck } from "lucide-react";
import { useState, useCallback } from "react";

interface NodeDetailPanelProps {
  nodeId: string;
  messages: MessageRecord[];
  onClose: () => void;
  className?: string;
}

export default function NodeDetailPanel({
  nodeId,
  messages,
  onClose,
  className = "",
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
    <aside className={`min-h-0 rounded-lg border border-border bg-card ${className}`}>
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <p className="zed-kicker">{t("graph.panelTitle")}</p>
          <div className="flex flex-wrap items-center gap-1.5">
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

      <div className="space-y-2 p-2">
        <section className="rounded-md border border-border bg-secondary/80 px-2 py-1.5">
          <div className="flex items-center justify-between gap-2">
            <p className="zed-kicker">{t("graph.messageMeta")}</p>
            <span className="rounded border border-border bg-card px-1.5 py-0.5 font-mono text-[10px] leading-none text-muted-foreground">
              {msg.role}
            </span>
          </div>
          <dl className="mt-1 divide-y divide-border/70 text-[11px]">
            <CompactMetaRow label={t("graph.messageId")} value={msg.id} mono />
            {msg.model && <CompactMetaRow label={t("graph.model")} value={msg.model} mono />}
            {msg.timestamp && (
              <CompactMetaRow
                label={t("graph.timestamp")}
                value={new Date(msg.timestamp).toLocaleString()}
                icon={<Clock3 size={13} />}
              />
            )}
            {msg.parent_id && <CompactMetaRow label={t("graph.parentId")} value={msg.parent_id} mono />}
          </dl>
        </section>

        <section className="rounded-md border border-border bg-card p-2">
          <div className="flex items-center justify-between gap-2">
            <p className="zed-kicker">{t("graph.previewTitle")}</p>
            <CopyPreviewButton text={msg.content_preview || ""} />
          </div>
          <p className="mt-1.5 whitespace-pre-wrap break-words text-[12px] leading-[1.5] text-foreground">
            {msg.content_preview || t("graph.nodeEmpty")}
          </p>
        </section>

        {msg.tool_name && (
          <section className="rounded-md border border-border bg-secondary p-2">
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
    </aside>
  );
}

function CopyPreviewButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [text]);
  if (!text) return null;
  return (
    <button
      type="button"
      onClick={handleCopy}
      className="text-muted-foreground transition-colors hover:text-foreground"
      aria-label="Copy preview"
    >
      {copied ? <CircleCheck size={13} className="text-primary" /> : <Copy size={13} />}
    </button>
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

function CompactMetaRow({
  label,
  value,
  mono,
  icon,
}: {
  label: string;
  value: string;
  mono?: boolean;
  icon?: ReactNode;
}) {
  return (
    <div className="grid grid-cols-[76px_minmax(0,1fr)] items-start gap-2 py-1 first:pt-0 last:pb-0">
      <dt className="flex min-w-0 items-center gap-1 text-[10px] font-medium uppercase tracking-normal text-muted-foreground">
        {icon}
        <span className="truncate">{label}</span>
      </dt>
      <dd
        title={value}
        className={`min-w-0 truncate text-right leading-[1.35] text-foreground ${mono ? "font-mono" : ""}`}
      >
        {value}
      </dd>
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
