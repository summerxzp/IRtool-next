import { Sun, Moon, Minus, Square, X, Bell, PanelRight, PanelBottom } from "lucide-react";
import { useThemeStore } from "@/stores/theme-store";
import { Button } from "@/components/ui/button";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useState, useEffect } from "react";
import { useAlertStore } from "@/stores/alert-store";
import { AlertPanel } from "./AlertPanel";
import { useRouterState } from "@tanstack/react-router";
import { useUIStore } from "@/stores/ui-store";
import { commands, type AppInfo } from "@/lib/bindings";

export function TopBar() {
  const { resolvedTheme, setTheme } = useThemeStore();
  const [_isMaximized, setIsMaximized] = useState(false);
  const [alertPanelOpen, setAlertPanelOpen] = useState(false);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const unreadCount = useAlertStore((s) => s.unreadCount);
  const alertPanelAutoOpen = useAlertStore((s) => s.alertPanelAutoOpen);
  const setAlertPanelAutoOpen = useAlertStore((s) => s.setAlertPanelAutoOpen);

  const pathname = useRouterState({ select: (s) => s.location.pathname });

  const detailPage = pathname === "/autoruns" ? "autoruns"
    : pathname === "/log-collector" ? "log-collector"
    : pathname === "/network" ? "network"
    : pathname === "/workspace" ? "workspace"
    : null;

  const detailPosition = useUIStore((s) =>
    detailPage ? (s.detailPositions[detailPage] ?? "right") : null
  );
  const setDetailPosition = useUIStore((s) => s.setDetailPosition);

  useEffect(() => {
    const window = getCurrentWebviewWindow();
    window.isMaximized().then(setIsMaximized);
  }, []);

  useEffect(() => {
    commands.cmdAppInfo().then(setAppInfo).catch(() => null);
  }, []);

  // Auto-open alert panel when notification is clicked
  useEffect(() => {
    if (alertPanelAutoOpen) {
      setAlertPanelOpen(true);
      setAlertPanelAutoOpen(false);
    }
  }, [alertPanelAutoOpen, setAlertPanelAutoOpen]);

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
        {appInfo && (
          <span className="text-xs text-fg-tertiary">v{appInfo.version}</span>
        )}
      </div>

      {/* 中间：拖拽区域 */}
      <div className="flex-1 h-full" data-tauri-drag-region />

      {/* 右侧：告警 + 主题切换 + 窗口控制 */}
      <div className="flex items-center gap-0.5 relative">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 relative"
          data-alert-toggle
          onClick={() => setAlertPanelOpen((prev) => !prev)}
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

        {detailPage && detailPosition && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={() => setDetailPosition(detailPage, detailPosition === "bottom" ? "right" : "bottom")}
            title={detailPosition === "bottom" ? "Switch to Right" : "Switch to Bottom"}
          >
            {detailPosition === "bottom" ? <PanelRight className="h-4 w-4" /> : <PanelBottom className="h-4 w-4" />}
          </Button>
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
