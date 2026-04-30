import { useState, useEffect, useCallback, useRef } from "react";

function scrollDetailPane(edge: "top" | "bottom") {
  const vps = document.querySelectorAll<HTMLElement>(
    '[data-slot="scroll-area-viewport"]',
  );
  // Detail pane viewport is the last one in the DOM (after session list)
  const vp = vps[vps.length - 1];
  if (!vp) return;
  vp.scrollTo({
    top: edge === "top" ? 0 : vp.scrollHeight,
    behavior: "smooth",
  });
}

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

      const mod = e.metaKey || e.ctrlKey;

      switch (e.key) {
        case "?":
          setShowHelp((v) => !v);
          break;
        case "/":
          e.preventDefault();
          searchRef.current?.focus();
          break;
        case "ArrowDown":
          if (mod && selectedId) {
            e.preventDefault();
            scrollDetailPane("bottom");
          } else {
            navigateList("down");
          }
          break;
        case "ArrowUp":
          if (mod && selectedId) {
            e.preventDefault();
            scrollDetailPane("top");
          } else {
            navigateList("up");
          }
          break;
        case "j":
          if (!mod) navigateList("down");
          break;
        case "k":
          if (!mod) navigateList("up");
          break;
        case "Escape":
          if (showHelp) setShowHelp(false);
          else onSelect(null);
          break;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigateList, showHelp, onSelect, selectedId]);

  return { showHelp, setShowHelp, searchRef };
}
