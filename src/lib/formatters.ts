export function formatProjectLabel(path: string | null): string {
  if (!path) return "No project";
  const clean = path.replace(/\/+/g, "/");
  const parts = clean.split("/").filter(Boolean);
  const name = parts[parts.length - 1] ?? "unknown";
  if (parts.length <= 2) return name;
  // "yeek (/Users/…/apps/yeek)"
  return `${name} (/${parts[0]}/…/${parts[parts.length - 2]}/${parts[parts.length - 1]})`;
}

export function formatRelativeTime(iso: string | null): string {
  if (!iso) return "";
  try {
    const date = new Date(iso);
    const diffMs = Date.now() - date.getTime();
    const mins = Math.floor(diffMs / 60_000);
    if (mins < 1) return "now";
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d`;
    return date.toLocaleDateString();
  } catch {
    return "";
  }
}

export function formatTime(iso: string | null): string {
  if (!iso) return "N/A";
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return "N/A";
  }
}
