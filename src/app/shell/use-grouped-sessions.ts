import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { formatProjectLabel } from "@/lib/formatters";
import type { SessionRecord } from "@/lib/api";

export interface SessionGroup {
  key: string;
  label: string;
  sessions: SessionRecord[];
}

export function useGroupedSessions(
  sessions: SessionRecord[],
  isSearching: boolean,
) {
  const { t } = useTranslation();

  return useMemo(() => {
    const map = new Map<string, { label: string; sessions: SessionRecord[] }>();
    for (const s of sessions) {
      const key = isSearching ? "Results" : (s.project_path ?? t("sessions.noProject"));
      if (!map.has(key)) {
        map.set(key, {
          label: isSearching ? t("sessions.searchResults") : formatProjectLabel(s.project_path),
          sessions: [],
        });
      }
      map.get(key)?.sessions.push(s);
    }

    for (const group of map.values()) {
      group.sessions.sort((a, b) =>
        (b.updated_at ?? "").localeCompare(a.updated_at ?? "")
      );
    }

    return Array.from(map.entries())
      .sort((a, b) => {
        const aTime = a[1].sessions[0]?.updated_at ?? "";
        const bTime = b[1].sessions[0]?.updated_at ?? "";
        return bTime.localeCompare(aTime);
      })
      .map(([key, group]) => ({
        key,
        label: group.label,
        sessions: group.sessions,
      }));
  }, [sessions, isSearching, t]);
}
