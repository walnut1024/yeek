import { useEffect } from "react";
import { useLocalStorage } from "@/lib/hooks";

const BASE = 14;
const DEFAULT = 16;
const MIN = 10;
const MAX = 24;
const STEP = 2;

export function useZoom() {
  const [fontSize, setFontSize] = useLocalStorage("font-size", DEFAULT);

  useEffect(() => {
    document.documentElement.style.zoom = String(fontSize / BASE);
  }, [fontSize]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!e.metaKey && !e.ctrlKey) return;

      switch (e.key) {
        case "=":
        case "+":
          e.preventDefault();
          setFontSize((v: number) => Math.min(v + STEP, MAX));
          break;
        case "-":
          e.preventDefault();
          setFontSize((v: number) => Math.max(v - STEP, MIN));
          break;
        case "0":
          e.preventDefault();
          setFontSize(DEFAULT);
          break;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [setFontSize]);

  return { fontSize };
}
