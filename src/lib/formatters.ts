import i18n from "@/i18n";

export function getCurrentLocale(): string {
  return i18n.language === "zh-CN" ? "zh-CN" : "en-US";
}

export function formatProjectLabel(path: string | null): string {
  if (!path) return i18n.t("format.noProject");
  const clean = path.replace(/\/+/g, "/");
  const parts = clean.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? i18n.t("format.unknown");
}

export function formatRelativeTime(iso: string | null): string {
  if (!iso) return "";
  try {
    const date = new Date(iso);
    const diffMs = Date.now() - date.getTime();
    const mins = Math.floor(diffMs / 60_000);
    if (mins < 1) return i18n.t("format.now");
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d`;
    return date.toLocaleDateString(getCurrentLocale());
  } catch {
    return "";
  }
}

export function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  return `${m}m ${s}s`;
}

export function formatTime(iso: string | null): string {
  if (!iso) return i18n.t("format.notAvailable");
  try {
    return new Date(iso).toLocaleString(getCurrentLocale());
  } catch {
    return i18n.t("format.notAvailable");
  }
}
