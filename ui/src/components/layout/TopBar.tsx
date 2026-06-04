import { Sun, Moon, Minus, Square, X } from "lucide-react";
import { useThemeStore } from "@/stores/theme-store";
import { Button } from "@/components/ui/button";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useState, useEffect } from "react";

export function TopBar() {
  const { resolvedTheme, setTheme } = useThemeStore();
  const [isMaximized, setIsMaximized] = useState(false);

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

      {/* 右侧：主题切换 + 窗口控制 */}
      <div className="flex items-center gap-0.5">
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
          {isMaximized ? (
            <Square className="h-3.5 w-3.5" />
          ) : (
            <Square className="h-3.5 w-3.5" />
          )}
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
