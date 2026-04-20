import { useState, useEffect, useCallback, useRef } from "react";

export function useKeyboardNavigation(
  flatSessionIds: string[],
  selectedId: string | null,
  onSelect: (id: string | null) => void,
) {
  const [showHelp, setShowHelp] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  const navigateList = useCallback(
    (direction: "up" | "down") => {
      if (flatSessionIds.length === 0) return;
      const idx = selectedId ? flatSessionIds.indexOf(selectedId) : -1;
      const next =
        direction === "down"
          ? Math.min(idx + 1, flatSessionIds.length - 1)
          : Math.max(idx - 1, 0);
      onSelect(flatSessionIds[next]);
    },
    [flatSessionIds, selectedId, onSelect]
  );

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      switch (e.key) {
        case "?":
          setShowHelp((v) => !v);
          break;
        case "/":
          e.preventDefault();
          searchRef.current?.focus();
          break;
        case "j":
        case "ArrowDown":
          navigateList("down");
          break;
        case "k":
        case "ArrowUp":
          navigateList("up");
          break;
        case "Escape":
          if (showHelp) setShowHelp(false);
          else onSelect(null);
          break;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigateList, showHelp, onSelect]);

  return { showHelp, setShowHelp, searchRef };
}
