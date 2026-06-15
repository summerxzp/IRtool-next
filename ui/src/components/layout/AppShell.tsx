import { useState, useEffect, useCallback } from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";
import { setupAlertListener } from "@/stores/alert-store";
import { useSysmonEventListener, usePcapEventListener } from "@/features/log-collector/hooks";
import { ToolsCheckDialog } from "@/components/ToolsCheckDialog";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands } from "@/lib/bindings";
import type { ToolStatus } from "@/lib/bindings";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogAction,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog";

setupAlertListener();

export function AppShell({ children }: { children: React.ReactNode }) {
  // Keep sysmon event listener alive across page navigation
  useSysmonEventListener();
  usePcapEventListener();

  const [toolsDialogOpen, setToolsDialogOpen] = useState(false);
  const [closeConfirmOpen, setCloseConfirmOpen] = useState(false);

  useEffect(() => {
    // Check for missing tools on startup
    invoke<ToolStatus[]>("cmd_tools_check")
      .then((tools) => {
        if (tools.some((t) => !t.installed)) {
          setToolsDialogOpen(true);
        }
      })
      .catch(() => {});
  }, []);

  // Listen for close-requested event from backend (background mode)
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen("evt_close_requested", () => {
      setCloseConfirmOpen(true);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleForceQuit = useCallback(async () => {
    try {
      const result = await commands.cmdAppForceQuit();
      if (result.status === "error") throw result.error;
    } catch (e) {
      console.error("Force quit failed:", e);
    }
  }, []);

  const handleHideToTray = useCallback(async () => {
    try {
      // Hide window via entering background mode (window is already hidden by backend)
      // Just close the dialog; the backend already prevented close
      setCloseConfirmOpen(false);
      // Use the Tauri window API to hide
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().hide();
    } catch (e) {
      console.error("Hide to tray failed:", e);
    }
  }, []);

  return (
    <div className="flex flex-col h-screen bg-bg-base text-fg-primary">
      <TopBar />
      <div className="flex flex-1 min-h-0">
        <Sidebar />
        <div className="flex-1 flex flex-col min-w-0">
          <main className="flex-1 overflow-auto bg-bg-base">{children}</main>
          <StatusBar />
        </div>
      </div>
      <ToolsCheckDialog open={toolsDialogOpen} onOpenChange={setToolsDialogOpen} />

      <AlertDialog open={closeConfirmOpen} onOpenChange={setCloseConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>正在后台监控中</AlertDialogTitle>
            <AlertDialogDescription>
              当前处于后台监控模式，数据采集仍在运行。关闭窗口将中断监控。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter className="flex gap-2">
            <AlertDialogCancel onClick={() => setCloseConfirmOpen(false)}>取消</AlertDialogCancel>
            <AlertDialogAction onClick={handleHideToTray} className="bg-blue-600 hover:bg-blue-700">
              后台运行
            </AlertDialogAction>
            <AlertDialogAction onClick={handleForceQuit} className="bg-red-600 hover:bg-red-700">
              彻底退出
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
