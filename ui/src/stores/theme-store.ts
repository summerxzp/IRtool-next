import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

export type Theme = "dark" | "light" | "system";

interface ThemeState {
  theme: Theme;
  resolvedTheme: "dark" | "light";
  setTheme: (theme: Theme) => void;
  applyResolvedTheme: () => void;
}

function resolveSystem(): "dark" | "light" {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: "dark",
      resolvedTheme: "dark",
      setTheme: (theme) => {
        const resolved = theme === "system" ? resolveSystem() : theme;
        set({ theme, resolvedTheme: resolved });
        document.documentElement.setAttribute("data-theme", resolved);
        document.documentElement.classList.toggle("dark", resolved === "dark");
      },
      applyResolvedTheme: () => {
        const { theme } = get();
        const resolved = theme === "system" ? resolveSystem() : theme;
        set({ resolvedTheme: resolved });
        document.documentElement.setAttribute("data-theme", resolved);
        document.documentElement.classList.toggle("dark", resolved === "dark");
      },
    }),
    {
      name: "irtool-theme",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ theme: state.theme }),
    },
  ),
);
