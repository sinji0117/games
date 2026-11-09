import { useEffect, useState } from "react";
import { load, save } from "../lib/storage.ts";

type Theme = "system" | "light" | "dark";
const NEXT: Record<Theme, Theme> = { system: "light", light: "dark", dark: "system" };
const LABEL: Record<Theme, string> = { system: "화면: 시스템", light: "화면: 모눈종이", dark: "화면: 칠판" };

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(() => (load("theme") as Theme | null) ?? "system");
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
    save("theme", theme);
  }, [theme]);
  return (
    <button type="button" className="btn btn--ghost btn--sm" onClick={() => setTheme(NEXT[theme])}>
      {LABEL[theme]}
    </button>
  );
}
