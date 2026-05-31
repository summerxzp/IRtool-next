import { useEffect } from "react";
import { useThemeStore } from "@/stores/theme-store";

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const applyResolvedTheme = useThemeStore((s) => s.applyResolvedTheme);
  const theme = useThemeStore((s) => s.theme);

  useEffect(() => {
    applyResolvedTheme();
  }, [theme, applyResolvedTheme]);

  useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyResolvedTheme();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme, applyResolvedTheme]);

  return <>{children}</>;
}
