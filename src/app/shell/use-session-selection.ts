import { useState, useMemo, useEffect, useRef } from "react";
import type { SessionRecord } from "@/lib/api";
import type { SessionGroup } from "./use-grouped-sessions";

export function useSessionSelection(
  sessions: SessionRecord[],
  grouped: SessionGroup[],
  collapsedProjects: Record<string, boolean>,
  selectedId: string | null,
  onSelect: (id: string | null) => void,
) {
  const [manageMode, setManageMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [confirmDelete, setConfirmDelete] = useState(false);
  const hasAutoSelectedRef = useRef(false);

  const toggleSession = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleProject = (projectSessions: SessionRecord[]) => {
    const ids = projectSessions.map((s) => s.id);
    const allSelected = ids.every((id) => selectedIds.has(id));
    setSelectedIds((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => (allSelected ? next.delete(id) : next.add(id)));
      return next;
    });
  };

  const exitManageMode = () => {
    setManageMode(false);
    setSelectedIds(new Set());
    setConfirmDelete(false);
  };

  const allSelected = sessions.length > 0 && sessions.every((s) => selectedIds.has(s.id));
  const someSelected = selectedIds.size > 0 && !allSelected;

  const toggleAll = () => {
    if (allSelected) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(sessions.map((s) => s.id)));
    }
  };

  const flatSessionIds = useMemo(() => {
    const ids: string[] = [];
    for (const g of grouped) {
      if (!collapsedProjects[g.key]) {
        for (const s of g.sessions) {
          ids.push(s.id);
        }
      }
    }
    return ids;
  }, [grouped, collapsedProjects]);

  const topSessionId = flatSessionIds[0] ?? grouped[0]?.sessions[0]?.id ?? null;

  // Auto-select first session
  useEffect(() => {
    if (!topSessionId) {
      hasAutoSelectedRef.current = false;
      return;
    }

    if (selectedId && sessions.some((session) => session.id === selectedId)) {
      hasAutoSelectedRef.current = true;
      return;
    }

    if (!hasAutoSelectedRef.current && selectedId === null) {
      hasAutoSelectedRef.current = true;
      onSelect(topSessionId);
      return;
    }

    if (selectedId && !sessions.some((session) => session.id === selectedId)) {
      onSelect(topSessionId);
    }
  }, [topSessionId, selectedId, sessions, onSelect]);

  return {
    manageMode,
    setManageMode,
    selectedIds,
    confirmDelete,
    setConfirmDelete,
    toggleSession,
    toggleProject,
    exitManageMode,
    allSelected,
    someSelected,
    toggleAll,
    flatSessionIds,
  };
}
