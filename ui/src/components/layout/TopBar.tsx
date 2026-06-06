import { Sun, Moon, Minus, Square, X, Bell } from "lucide-react";
import { useThemeStore } from "@/stores/theme-store";
import { Button } from "@/components/ui/button";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useState, useEffect } from "react";
import { useAlertStore } from "@/stores/alert-store";
import { AlertPanel } from "./AlertPanel";

export function TopBar() {
  const { resolvedTheme, setTheme } = useThemeStore();
  const [_isMaximized, setIsMaximized] = useState(false);
  const [alertPanelOpen, setAlertPanelOpen] = useState(false);
  const unreadCount = useAlertStore((s) => s.unreadCount);

  useEffect(() => {
    const window = getCurrentWebviewWindow();
    window.isMaximized().then(setIsMaximized);
  }, []);

  const handleMinimize = () => {
    getCurrentWebviewWindow().minimize();
  };

  const handleMaximize = () => {
    const window = getCurrentWebviewWindow();
    window.toggleMaximize();
    setIsMaximized((prev) => !prev);
  };

  const handleClose = () => {
    getCurrentWebviewWindow().close();
  };

  return (
    <header
      className="h-10 bg-bg-elev-1 border-b border-border flex items-center px-3 select-none"
      data-tauri-drag-region
    >
      {/* 左侧：标题 */}
      <div className="flex items-center gap-2" data-tauri-drag-region>
        <span className="text-sm font-semibold text-fg-primary">
          IRtool
        </span>
        <span className="text-xs text-fg-tertiary">v2.0.0-alpha.1</span>
      </div>

      {/* 中间：拖拽区域 */}
      <div className="flex-1 h-full" data-tauri-drag-region />

      {/* 右侧：告警 + 主题切换 + 窗口控制 */}
      <div className="flex items-center gap-0.5 relative">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 relative"
          onClick={() => setAlertPanelOpen(!alertPanelOpen)}
        >
          <Bell className="h-4 w-4" />
          {unreadCount > 0 && (
            <span className="absolute -top-0.5 -right-0.5 h-3.5 min-w-[14px] flex items-center justify-center rounded-full bg-red-500 text-[8px] text-white font-bold px-0.5">
              {unreadCount > 99 ? "99+" : unreadCount}
            </span>
          )}
        </Button>

        {alertPanelOpen && (
          <AlertPanel onClose={() => setAlertPanelOpen(false)} />
        )}

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
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

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-bg-elev-2"
          onClick={handleMinimize}
        >
          <Minus className="h-4 w-4" />
        </Button>

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-bg-elev-2"
          onClick={handleMaximize}
        >
          <Square className="h-3.5 w-3.5" />
        </Button>

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-danger hover:text-white"
          onClick={handleClose}
        >
          <X className="h-4 w-4" />
        </Button>
      </div>
    </header>
  );
}
