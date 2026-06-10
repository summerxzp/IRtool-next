import { useState, useEffect } from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";
import { setupAlertListener } from "@/stores/alert-store";
import { useSysmonEventListener, usePcapEventListener } from "@/features/log-collector/hooks";
import { ToolsCheckDialog } from "@/components/ToolsCheckDialog";
import { invoke } from "@tauri-apps/api/core";
import type { ToolStatus } from "@/lib/bindings";

setupAlertListener();

export function AppShell({ children }: { children: React.ReactNode }) {
  // Keep sysmon event listener alive across page navigation
  useSysmonEventListener();
  usePcapEventListener();

  const [toolsDialogOpen, setToolsDialogOpen] = useState(false);

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
    </div>
  );
}
