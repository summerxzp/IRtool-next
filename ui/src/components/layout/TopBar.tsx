import { Search, Sun, Moon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { useThemeStore } from "@/stores/theme-store";

export function TopBar() {
  const { t } = useTranslation();
  const { resolvedTheme, setTheme } = useThemeStore();

  return (
    <header className="h-10 bg-bg-elev-1 border-b border-border flex items-center px-3 gap-3">
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold text-fg-primary">
          {t("app.name")}
        </span>
        <span className="text-xs text-fg-tertiary">v2.0.0-alpha.1</span>
      </div>
      <div className="flex-1 max-w-xl mx-auto">
        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-fg-tertiary" />
          <input
            type="text"
            placeholder="Ctrl+P"
            className="w-full h-7 bg-bg-base border border-border rounded pl-7 pr-3 text-xs placeholder:text-fg-tertiary focus:outline-none focus:border-accent"
            disabled
          />
        </div>
      </div>
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          onClick={() =>
            setTheme(resolvedTheme === "dark" ? "light" : "dark")
          }
          title={resolvedTheme === "dark" ? "Light" : "Dark"}
        >
          {resolvedTheme === "dark" ? (
            <Sun className="h-4 w-4" />
          ) : (
            <Moon className="h-4 w-4" />
          )}
        </Button>
      </div>
    </header>
  );
}
