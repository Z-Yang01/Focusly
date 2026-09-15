/** 主题：浅色/深色/跟随系统。class 策略（html.dark）。
 *  每个窗口（manager / note-*）独立 bootstrap，都挂这个 Provider。 */
import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { getAllSettings, setSetting } from "@/lib/api";
import { onSettingsChanged } from "@/lib/tauri";
import { SETTINGS_KEYS, type ThemeMode } from "@/types";
import { useUiStore } from "@/stores/ui";

interface ThemeCtx {
  theme: ThemeMode;
  setTheme: (t: ThemeMode) => void;
}

const ThemeContext = createContext<ThemeCtx>({
  theme: "system",
  setTheme: () => {},
});

function applyTheme(mode: ThemeMode) {
  const prefersDark =
    window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
  const dark = mode === "dark" || (mode === "system" && prefersDark);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeMode>("system");
  const setUiTheme = useUiStore((s) => s.setTheme);

  useEffect(() => {
    let alive = true;
    getAllSettings().then((s) => {
      if (!alive) return;
      const t = (s[SETTINGS_KEYS.theme] as ThemeMode) || "system";
      setThemeState(t);
      setUiTheme(t);
    });
    const off = onSettingsChanged(({ key, value }) => {
      if (key !== SETTINGS_KEYS.theme) return;
      const t = (value as ThemeMode) || "system";
      setThemeState(t);
      setUiTheme(t);
    });
    return () => {
      alive = false;
      off.then((f) => f());
    };
  }, [setUiTheme]);

  useEffect(() => {
    applyTheme(theme);
    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme(theme);
    mq?.addEventListener("change", onChange);
    return () => mq?.removeEventListener("change", onChange);
  }, [theme]);

  const setTheme = (t: ThemeMode) => {
    setThemeState(t);
    setUiTheme(t);
    void setSetting(SETTINGS_KEYS.theme, t);
  };

  return <ThemeContext.Provider value={{ theme, setTheme }}>{children}</ThemeContext.Provider>;
}

export const useTheme = () => useContext(ThemeContext);
